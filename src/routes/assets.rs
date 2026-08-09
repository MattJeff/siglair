//! Médias de l'organisation — contrat §5.3 et §8.
//!
//! Le type d'un fichier est déduit de ses octets, jamais du `Content-Type` annoncé par le
//! client : c'est la seule façon de refuser un SVG déguisé en PNG, et un SVG dans une
//! signature, c'est du XSS distribué par e-mail.

use std::collections::HashMap;

use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    auth::OrgAccess,
    doc::Doc,
    error::{AppError, Result},
    AppState,
};

/// Contrat §5.3. La limite globale du serveur est à 12 Mo pour laisser passer l'enveloppe
/// multipart d'un fichier de 10 Mo.
const MAX_BYTES: usize = 10 * 1024 * 1024;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/assets", get(list).post(upload))
        .route("/assets/{id}", axum::routing::delete(remove))
}

const COLS: &str = "id, org_id, kind, filename, content_type, bytes, storage_key, \
                    width, height, created_at";

#[derive(sqlx::FromRow)]
struct AssetRow {
    id: Uuid,
    org_id: Uuid,
    kind: String,
    filename: String,
    content_type: String,
    bytes: i64,
    storage_key: String,
    width: Option<i32>,
    height: Option<i32>,
    created_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct AssetOut {
    id: Uuid,
    url: String,
    kind: String,
    filename: String,
    content_type: String,
    bytes: i64,
    width: Option<i32>,
    height: Option<i32>,
    created_at: DateTime<Utc>,
}

fn out(st: &AppState, a: AssetRow) -> AssetOut {
    AssetOut {
        url: st.storage.url(&a.storage_key),
        id: a.id,
        kind: a.kind,
        filename: a.filename,
        content_type: a.content_type,
        bytes: a.bytes,
        width: a.width,
        height: a.height,
        created_at: a.created_at,
    }
}

/// Types acceptés, liste fermée. Tout le reste (SVG, PDF, HTML, archives) est refusé.
fn kind_for(mime: &str) -> Option<&'static str> {
    match mime {
        "image/png" | "image/jpeg" | "image/gif" | "image/webp" => Some("image"),
        "video/mp4" | "video/webm" => Some("video"),
        _ => None,
    }
}

async fn list(State(st): State<AppState>, access: OrgAccess) -> Result<Json<Vec<AssetOut>>> {
    let rows = sqlx::query_as::<_, AssetRow>(&format!(
        "SELECT {COLS} FROM assets WHERE org_id = $1 ORDER BY created_at DESC LIMIT 500"
    ))
    .bind(access.org_id)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(rows.into_iter().map(|a| out(&st, a)).collect()))
}

async fn by_sha(db: &PgPool, org_id: Uuid, sha: &[u8]) -> Result<Option<AssetRow>> {
    Ok(sqlx::query_as::<_, AssetRow>(&format!(
        "SELECT {COLS} FROM assets WHERE org_id = $1 AND sha256 = $2"
    ))
    .bind(org_id)
    .bind(sha)
    .fetch_optional(db)
    .await?)
}

async fn upload(
    State(st): State<AppState>,
    access: OrgAccess,
    mut form: Multipart,
) -> Result<(StatusCode, Json<AssetOut>)> {
    // §8 : jusqu'à 10 Mo écrits sur disque par appel. Plafonné par organisation.
    if !crate::util::rate_limit(
        &format!("upload:{}", access.org_id),
        60,
        std::time::Duration::from_secs(3600),
    ) {
        return Err(AppError::RateLimited);
    }
    let plan = access.plan;

    let mut filename = String::new();
    let mut body: Option<Vec<u8>> = None;
    while let Some(field) = form
        .next_field()
        .await
        .map_err(|_| AppError::validation("Envoi illisible : utilisez un formulaire multipart."))?
    {
        if field.name() != Some("file") {
            continue;
        }
        filename = clean_filename(field.file_name().unwrap_or_default());
        body = Some(
            field
                .bytes()
                .await
                .map_err(|_| {
                    AppError::validation("Le fichier n'a pas pu être lu en entier (10 Mo maximum).")
                })?
                .to_vec(),
        );
        break;
    }
    let body = body.ok_or_else(|| {
        AppError::validation(
            "Aucun fichier reçu : le champ « file » du formulaire est obligatoire.",
        )
    })?;
    if body.is_empty() {
        return Err(AppError::validation("Le fichier envoyé est vide."));
    }
    if body.len() > MAX_BYTES {
        return Err(AppError::validation(
            "Un fichier ne peut pas dépasser 10 Mo.",
        ));
    }

    let detected = infer::get(&body).ok_or_else(|| AppError::validation(FORMAT_MSG))?;
    let kind = kind_for(detected.mime_type()).ok_or_else(|| AppError::validation(FORMAT_MSG))?;

    let used: i64 =
        sqlx::query_scalar("SELECT coalesce(sum(bytes), 0)::bigint FROM assets WHERE org_id = $1")
            .bind(access.org_id)
            .fetch_one(&st.db)
            .await?;
    if used.max(0) as u64 + body.len() as u64 > plan.limits.assets_bytes {
        return Err(AppError::QuotaExceeded(format!(
            "Votre espace média est plein ({} Mo sur ce plan). Supprimez des fichiers ou passez \
             à un plan supérieur.",
            plan.limits.assets_bytes / (1024 * 1024)
        )));
    }

    let sha = Sha256::digest(&body).to_vec();
    if let Some(existing) = by_sha(&st.db, access.org_id, &sha).await? {
        // même contenu déjà présent : on renvoie l'asset existant, ce n'est pas une erreur
        return Ok((StatusCode::OK, Json(out(&st, existing))));
    }

    let (width, height) = dimensions(&body).map_or((None, None), |(w, h)| (Some(w), Some(h)));
    let len = body.len() as i64;
    let key = format!(
        "assets/{}/{}.{}",
        access.org_id,
        hex(&sha),
        detected.extension()
    );
    st.storage.put(&key, body, detected.mime_type()).await?;

    let inserted = sqlx::query_as::<_, AssetRow>(&format!(
        "INSERT INTO assets (id, org_id, kind, filename, content_type, bytes, sha256, \
                             storage_key, width, height) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) \
         ON CONFLICT (org_id, sha256) DO NOTHING RETURNING {COLS}"
    ))
    .bind(Uuid::new_v4())
    .bind(access.org_id)
    .bind(kind)
    .bind(&filename)
    .bind(detected.mime_type())
    .bind(len)
    .bind(&sha)
    .bind(&key)
    .bind(width)
    .bind(height)
    .fetch_optional(&st.db)
    .await?;

    // conflit gagné par un envoi concurrent du même fichier : la clé de stockage étant
    // dérivée du sha256, l'objet écrit est le même — on renvoie simplement la ligne gagnante.
    let row = match inserted {
        Some(r) => r,
        None => by_sha(&st.db, access.org_id, &sha)
            .await?
            .ok_or(AppError::NotFound)?,
    };
    Ok((StatusCode::CREATED, Json(out(&st, row))))
}

const FORMAT_MSG: &str = "Format non accepté. Utilisez PNG, JPEG, GIF, WebP, MP4 ou WebM. \
                          Le SVG est refusé pour des raisons de sécurité.";

async fn remove(
    State(st): State<AppState>,
    access: OrgAccess,
    Path(id): Path<Uuid>,
) -> Result<StatusCode> {
    let row = sqlx::query_as::<_, AssetRow>(&format!(
        "SELECT {COLS} FROM assets WHERE id = $1 AND org_id = $2"
    ))
    .bind(id)
    .bind(access.org_id)
    .fetch_optional(&st.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let used: bool = sqlx::query_scalar(
        "SELECT exists( \
           SELECT 1 FROM signatures \
           WHERE org_id = $1 AND deleted_at IS NULL \
             AND doc->'elements' @> jsonb_build_array(jsonb_build_object('assetId', $2::text)))",
    )
    .bind(access.org_id)
    .bind(row.id.to_string())
    .fetch_one(&st.db)
    .await?;
    if used {
        return Err(AppError::conflict(
            "Ce média est utilisé par une signature. Retirez-le de la signature avant de le supprimer.",
        ));
    }

    sqlx::query("DELETE FROM assets WHERE id = $1 AND org_id = $2")
        .bind(row.id)
        .bind(row.org_id)
        .execute(&st.db)
        .await?;
    // un objet orphelin sur le disque est moins grave qu'une ligne pointant dans le vide
    if let Err(e) = st.storage.delete(&row.storage_key).await {
        tracing::warn!(error = ?e, key = %row.storage_key, "média non supprimé du stockage");
    }
    Ok(StatusCode::NO_CONTENT)
}

/// `asset_id` → URL publique, pour `RenderOpts::assets`. Une seule requête, même si le
/// document référence vingt fois le même média.
pub(crate) async fn urls_for_doc(
    st: &AppState,
    org_id: Uuid,
    doc: &Doc,
) -> Result<HashMap<Uuid, String>> {
    let ids: Vec<Uuid> = doc.elements.iter().filter_map(|e| e.asset_id).collect();
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows: Vec<(Uuid, String)> =
        sqlx::query_as("SELECT id, storage_key FROM assets WHERE org_id = $1 AND id = ANY($2)")
            .bind(org_id)
            .bind(&ids)
            .fetch_all(&st.db)
            .await?;
    Ok(rows
        .into_iter()
        .map(|(id, key)| (id, st.storage.url(&key)))
        .collect())
}

// ------------------------------------------------------------------ octets

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
            use std::fmt::Write as _;
            let _ = write!(s, "{b:02x}");
            s
        })
}

/// Le nom d'origine est purement informatif : il ne sert jamais à construire une clé de
/// stockage. On le raccourcit et on retire les séparateurs de chemin, c'est tout.
fn clean_filename(raw: &str) -> String {
    let base = raw.rsplit(['/', '\\']).next().unwrap_or("").trim();
    let name: String = base.chars().filter(|c| !c.is_control()).take(120).collect();
    if name.is_empty() {
        "fichier".to_string()
    } else {
        name
    }
}

/// Dimensions lues dans l'en-tête. Pas de décodage : l'éditeur veut deux entiers, pas une
/// image en mémoire — et le crate `image` n'est compilé qu'avec le décodeur PNG.
fn dimensions(b: &[u8]) -> Option<(i32, i32)> {
    if b.len() >= 24 && b.starts_with(b"\x89PNG\r\n\x1a\n") && &b[12..16] == b"IHDR" {
        return Some((be32(b, 16), be32(b, 20)));
    }
    if b.len() >= 10 && (b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a")) {
        return Some((le16(b, 6) as i32, le16(b, 8) as i32));
    }
    if b.len() >= 30 && b.starts_with(b"RIFF") && &b[8..12] == b"WEBP" {
        return webp(b);
    }
    if b.len() >= 4 && b[0] == 0xff && b[1] == 0xd8 {
        return jpeg(b);
    }
    None
}

fn be32(b: &[u8], i: usize) -> i32 {
    (u32::from_be_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]) & 0x7fff_ffff) as i32
}

fn le16(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}

fn webp(b: &[u8]) -> Option<(i32, i32)> {
    match &b[12..16] {
        b"VP8X" => {
            let d =
                |i: usize| i32::from(b[i]) | i32::from(b[i + 1]) << 8 | i32::from(b[i + 2]) << 16;
            Some((1 + d(24), 1 + d(27)))
        }
        b"VP8 " if b[23] == 0x9d && b[24] == 0x01 && b[25] == 0x2a => {
            Some(((le16(b, 26) & 0x3fff) as i32, (le16(b, 28) & 0x3fff) as i32))
        }
        b"VP8L" if b[20] == 0x2f => {
            let bits = u32::from_le_bytes([b[21], b[22], b[23], b[24]]);
            Some((
                (1 + (bits & 0x3fff)) as i32,
                (1 + ((bits >> 14) & 0x3fff)) as i32,
            ))
        }
        _ => None,
    }
}

fn jpeg(b: &[u8]) -> Option<(i32, i32)> {
    let mut i = 2;
    while i + 9 < b.len() {
        if b[i] != 0xff {
            i += 1; // resynchronisation : un octet de bourrage traîne parfois entre segments
            continue;
        }
        let marker = b[i + 1];
        // SOF0..SOF15, sauf DHT (c4), JPG (c8) et DAC (cc) : hauteur puis largeur, big-endian
        if (0xc0..=0xcf).contains(&marker) && !matches!(marker, 0xc4 | 0xc8 | 0xcc) {
            let h = u16::from_be_bytes([b[i + 5], b[i + 6]]) as i32;
            let w = u16::from_be_bytes([b[i + 7], b[i + 8]]) as i32;
            return (w > 0 && h > 0).then_some((w, h));
        }
        if matches!(marker, 0x01 | 0xd0..=0xd9) {
            i += 2;
            continue;
        }
        let len = u16::from_be_bytes([b[i + 2], b[i + 3]]) as usize;
        if len < 2 {
            return None;
        }
        i += 2 + len;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_header_dimensions() {
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        png.extend_from_slice(&620u32.to_be_bytes());
        png.extend_from_slice(&250u32.to_be_bytes());
        assert_eq!(dimensions(&png), Some((620, 250)));

        let mut gif = b"GIF89a".to_vec();
        gif.extend_from_slice(&64u16.to_le_bytes());
        gif.extend_from_slice(&32u16.to_le_bytes());
        gif.extend_from_slice(&[0; 8]);
        assert_eq!(dimensions(&gif), Some((64, 32)));

        // SOI, un APP0 à sauter, puis SOF0 déclarant 8x4
        let jpg: &[u8] = &[
            0xff, 0xd8, // SOI
            0xff, 0xe0, 0x00, 0x08, 0, 0, 0, 0, 0, 0, // APP0, longueur 8
            0xff, 0xc0, 0x00, 0x11, 0x08, 0x00, 0x04, 0x00, 0x08, 0x03, // SOF0 : h=4, w=8
            0, 0, 0, 0,
        ];
        assert_eq!(dimensions(jpg), Some((8, 4)));

        assert_eq!(
            dimensions(b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>"),
            None
        );
    }

    #[test]
    fn only_email_safe_types_pass() {
        assert_eq!(kind_for("image/png"), Some("image"));
        assert_eq!(kind_for("video/mp4"), Some("video"));
        for bad in [
            "image/svg+xml",
            "text/html",
            "application/pdf",
            "application/zip",
            "",
        ] {
            assert!(kind_for(bad).is_none(), "{bad} aurait dû être refusé");
        }
    }

    #[test]
    fn filenames_lose_their_path() {
        assert_eq!(clean_filename("../../etc/passwd"), "passwd");
        assert_eq!(clean_filename("C:\\Users\\x\\logo.png"), "logo.png");
        assert_eq!(clean_filename("   "), "fichier");
    }
}
