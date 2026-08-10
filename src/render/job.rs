//! File d'attente de rendu — contrat §7. Une table PostgreSQL et `FOR UPDATE SKIP LOCKED`
//! suffisent : à trois jobs par minute, un Redis serait un service de plus à surveiller.

use anyhow::anyhow;
use sqlx::PgPool;
use uuid::Uuid;

use super::gif;
use crate::doc::{Doc, ElementType, Profile};
use crate::{util, AppState};

/// Contrat §7.9 : 30 s, 2 min, 10 min, puis abandon.
const BACKOFF_SECS: [f64; 3] = [30.0, 120.0, 600.0];
const MAX_ATTEMPTS: i32 = 3;

/// Échec d'un rendu. `user` part dans `render_jobs.error` et s'affiche tel quel : ce champ
/// est lu par un humain qui veut savoir quoi corriger, pas par un développeur.
#[derive(Debug, thiserror::Error)]
#[error("{user}")]
pub struct RenderError {
    pub user: String,
    /// Cause technique : journalisée, jamais stockée ni renvoyée.
    pub cause: anyhow::Error,
}

impl RenderError {
    pub fn new(user: impl Into<String>, cause: impl Into<anyhow::Error>) -> Self {
        Self {
            user: user.into(),
            cause: cause.into(),
        }
    }

    pub fn msg(user: impl Into<String>) -> Self {
        let user = user.into();
        Self {
            cause: anyhow!(user.clone()),
            user,
        }
    }
}

fn db_err(e: sqlx::Error) -> RenderError {
    RenderError::new("Le rendu a échoué. Réessayez dans un instant.", e)
}

#[derive(Debug, sqlx::FromRow)]
pub struct Job {
    pub id: Uuid,
    pub signature_id: Uuid,
    pub attempts: i32,
    pub doc: Option<serde_json::Value>,
    pub profile: Option<serde_json::Value>,
    pub campaign_id: Option<Uuid>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Réclamation atomique. Un `SELECT` suivi d'un `UPDATE` ferait traiter le même job par
/// deux workers : le verrou et la mise à jour doivent être la même instruction.
pub async fn claim(db: &PgPool) -> sqlx::Result<Option<Job>> {
    sqlx::query_as::<_, Job>(
        "UPDATE render_jobs SET status = 'running', started_at = now(), attempts = attempts + 1 \
         WHERE id = (SELECT id FROM render_jobs \
                     WHERE status = 'queued' AND run_after <= now() \
                     ORDER BY created_at LIMIT 1 FOR UPDATE SKIP LOCKED) \
         RETURNING id, signature_id, attempts, doc, profile, campaign_id, created_at",
    )
    .fetch_optional(db)
    .await
}

/// Contrat §7 : un job `running` depuis plus de 5 minutes appartient à un worker mort.
pub async fn requeue_stale(db: &PgPool) -> sqlx::Result<u64> {
    let r = sqlx::query(
        "UPDATE render_jobs SET status = 'queued', run_after = now() \
         WHERE status = 'running' AND started_at < now() - interval '5 minutes'",
    )
    .execute(db)
    .await?;
    Ok(r.rows_affected())
}

pub async fn run(state: &AppState, job: Job) {
    let start = std::time::Instant::now();
    match execute(state, &job).await {
        Ok(()) => {
            tracing::info!(job = %job.id, ms = start.elapsed().as_millis(), "rendu publié");
            let done = sqlx::query(
                "UPDATE render_jobs SET status = 'done', error = NULL, finished_at = now() WHERE id = $1",
            )
            .bind(job.id)
            .execute(&state.db)
            .await;
            if let Err(e) = done {
                // Le rendu existe ; le job repassera en 'queued' au prochain démarrage et
                // retombera sur le cache par `doc_hash`. Rien n'est perdu.
                tracing::error!(job = %job.id, error = ?e, "statut 'done' non enregistré");
            }
        }
        Err(err) => fail(state, &job, err).await,
    }
}

async fn fail(state: &AppState, job: &Job, err: RenderError) {
    let definitive = job.attempts >= MAX_ATTEMPTS;
    tracing::warn!(job = %job.id, attempt = job.attempts, definitive, cause = ?err.cause, "rendu en échec");

    let query = if definitive {
        sqlx::query(
            "UPDATE render_jobs SET status = 'failed', error = $2, finished_at = now() WHERE id = $1",
        )
        .bind(job.id)
        .bind(&err.user)
    } else {
        let secs = BACKOFF_SECS[(job.attempts.max(1) - 1) as usize % BACKOFF_SECS.len()];
        sqlx::query(
            "UPDATE render_jobs SET status = 'queued', error = $2, \
             run_after = now() + make_interval(secs => $3) WHERE id = $1",
        )
        .bind(job.id)
        .bind(&err.user)
        .bind(secs)
    };
    if let Err(e) = query.execute(&state.db).await {
        tracing::error!(job = %job.id, error = ?e, "échec non enregistré");
    }

    // Un e-mail par tentative transformerait un souci passager en harcèlement : seul
    // l'abandon définitif prévient l'utilisateur.
    if definitive {
        notify_failure(state, job, &err.user).await;
    }
}

async fn notify_failure(state: &AppState, job: &Job, message: &str) {
    let row = sqlx::query_as::<_, (String, String)>(
        "SELECT u.email::text, s.name FROM signatures s \
         JOIN org_members m ON m.org_id = s.org_id \
         JOIN users u ON u.id = m.user_id \
         WHERE s.id = $1 AND (u.id = s.owner_user_id OR m.role = 'owner') \
         ORDER BY (u.id = s.owner_user_id) DESC LIMIT 1",
    )
    .bind(job.signature_id)
    .fetch_optional(&state.db)
    .await;

    let Ok(Some((email, name))) = row else {
        tracing::warn!(job = %job.id, "aucun destinataire pour l'échec de rendu");
        return;
    };
    // `email::send_render_failed` construit et échappe le corps : pas de second gabarit ici.
    let link = format!("{}/app", state.cfg.app_url);
    if let Err(e) = crate::email::send_render_failed(state, &email, &name, message, &link).await {
        tracing::error!(job = %job.id, error = ?e, "e-mail d'échec non envoyé");
    }
}

// --------------------------------------------------------------------------- exécution

#[derive(sqlx::FromRow)]
struct SignatureRow {
    org_id: Uuid,
    doc: serde_json::Value,
    profile: serde_json::Value,
}

async fn execute(state: &AppState, job: &Job) -> Result<(), RenderError> {
    let row = sqlx::query_as::<_, SignatureRow>(
        "SELECT org_id, doc, profile FROM signatures WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(job.signature_id)
    .fetch_optional(&state.db)
    .await
    .map_err(db_err)?
    .ok_or_else(|| RenderError::msg("Cette signature a été supprimée avant d'être publiée."))?;

    // Les nouveaux jobs emportent l'instantané exact demandé. Les colonnes restent
    // optionnelles pour terminer proprement les jobs créés avant la migration 0006.
    let doc_value = job.doc.clone().unwrap_or(row.doc);
    let profile_value = job.profile.clone().unwrap_or(row.profile);
    let doc: Doc = serde_json::from_value(doc_value).map_err(|e| {
        RenderError::new(
            "Cette signature est illisible. Rouvrez-la dans l'éditeur et réenregistrez-la.",
            e,
        )
    })?;
    let profile: Profile = serde_json::from_value(profile_value).unwrap_or_default();

    // Le couple (doc, profil) figé dans le job fait foi. Son hash sert à la fois de clé de
    // cache et de nom de fichier ; le profil en fait partie car ses jetons sont rasterisés
    // dans le GIF (§3.1).
    let hash = util::doc_hash(&doc, &profile);

    // Contrat §7 : rejouer un job doit redonner le même résultat, sans recalculer.
    let existing = sqlx::query_as::<_, (Uuid,)>(
        "SELECT id FROM renders WHERE signature_id = $1 AND doc_hash = $2 \
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(job.signature_id)
    .bind(&hash)
    .fetch_optional(&state.db)
    .await
    .map_err(db_err)?;
    if let Some((render_id,)) = existing {
        if superseded(&state.db, job).await.map_err(db_err)? {
            return Ok(());
        }
        return publish(state, job.signature_id, render_id, job.campaign_id).await;
    }

    let assets = load_assets(state, row.org_id, &doc).await?;
    let out = gif::render(&state.cfg, &doc, &profile, &assets).await?;

    // Clés dérivées du hash : rejouer le job réécrit exactement les mêmes fichiers.
    let stem = hex(&hash);
    let gif_key = format!("renders/{}/{stem}.gif", job.signature_id);
    let png_key = format!("renders/{}/{stem}.png", job.signature_id);
    let bytes = out.gif.len() as i64;
    let store = |e: crate::error::AppError| {
        RenderError::new(
            "Le GIF n'a pas pu être enregistré. Réessayez dans un instant.",
            e,
        )
    };
    state
        .storage
        .put(&gif_key, out.gif, "image/gif")
        .await
        .map_err(store)?;
    state
        .storage
        .put(&png_key, out.png, "image/png")
        .await
        .map_err(store)?;

    // Instantané de ce qui vient d'être rasterisé (migration 0002) : c'est lui, et non
    // `signatures.doc` qui continue d'évoluer, que lit `/c/{slug}/{element_id}`.
    let snapshot = |e: serde_json::Error| {
        RenderError::new(
            "Le rendu n'a pas pu être enregistré. Réessayez dans un instant.",
            e,
        )
    };
    let doc_snapshot = serde_json::to_value(&doc).map_err(snapshot)?;
    let profile_snapshot = serde_json::to_value(&profile).map_err(snapshot)?;

    let render_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO renders (id, signature_id, gif_key, png_key, width, height, frames, fps, bytes, doc_hash, doc, profile) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)",
    )
    .bind(render_id)
    .bind(job.signature_id)
    .bind(&gif_key)
    .bind(&png_key)
    .bind(out.width as i32)
    .bind(out.height as i32)
    .bind(out.frames as i32)
    .bind(out.fps as i32)
    .bind(bytes)
    .bind(&hash)
    .bind(doc_snapshot)
    .bind(profile_snapshot)
    .execute(&state.db)
    .await
    .map_err(db_err)?;

    if superseded(&state.db, job).await.map_err(db_err)? {
        return Ok(());
    }
    publish(state, job.signature_id, render_id, job.campaign_id).await
}

/// Un rendu lent ne doit jamais écraser une publication demandée après lui.
async fn superseded(db: &PgPool, job: &Job) -> sqlx::Result<bool> {
    sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM render_jobs \
         WHERE signature_id = $1 AND (created_at, id) > ($2, $3))",
    )
    .bind(job.signature_id)
    .bind(job.created_at)
    .bind(job.id)
    .fetch_one(db)
    .await
}

async fn publish(
    state: &AppState,
    signature_id: Uuid,
    render_id: Uuid,
    campaign_id: Option<Uuid>,
) -> Result<(), RenderError> {
    sqlx::query(
        "UPDATE signatures SET published_render_id = $2, published_campaign_id = $3, \
           campaign_base_doc = CASE WHEN $3::uuid IS NULL THEN NULL ELSE campaign_base_doc END, \
           campaign_base_profile = CASE WHEN $3::uuid IS NULL THEN NULL ELSE campaign_base_profile END \
         WHERE id = $1",
    )
    .bind(signature_id)
    .bind(render_id)
    .bind(campaign_id)
    .execute(&state.db)
    .await
    .map_err(db_err)?;
    Ok(())
}

/// Les médias sont lus depuis le `Storage` et passés au rendu en mémoire : Chromium n'a
/// alors rien à télécharger pour eux.
async fn load_assets(
    state: &AppState,
    org_id: Uuid,
    doc: &Doc,
) -> Result<Vec<gif::Asset>, RenderError> {
    let ids: Vec<Uuid> = doc
        .elements
        .iter()
        .filter(|e| !e.hidden && matches!(e.kind, ElementType::Image | ElementType::Video))
        .filter_map(|e| e.asset_id)
        .collect();
    if ids.is_empty() {
        return Ok(Vec::new());
    }

    let rows = sqlx::query_as::<_, (Uuid, String, String)>(
        "SELECT id, content_type, storage_key FROM assets WHERE org_id = $1 AND id = ANY($2)",
    )
    .bind(org_id)
    .bind(&ids[..])
    .fetch_all(&state.db)
    .await
    .map_err(db_err)?;

    let mut out = Vec::with_capacity(rows.len());
    for (id, content_type, key) in rows {
        let bytes = state.storage.get(&key).await.map_err(|e| {
            RenderError::new(
                "Une image de la signature est introuvable. Réimportez-la puis republiez.",
                e,
            )
        })?;
        out.push(gif::Asset {
            id,
            content_type,
            bytes,
        });
    }
    Ok(out)
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
            use std::fmt::Write as _;
            let _ = write!(s, "{b:02x}");
            s
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_follows_the_contract() {
        // attempts est déjà incrémenté par `claim` quand `fail` s'exécute
        let delay =
            |attempts: i32| BACKOFF_SECS[(attempts.max(1) - 1) as usize % BACKOFF_SECS.len()];
        assert_eq!(delay(1), 30.0);
        assert_eq!(delay(2), 120.0);
        assert_eq!(delay(3), 600.0);
        assert_eq!(MAX_ATTEMPTS, 3);
    }

    #[test]
    fn keys_are_deterministic_and_storable() {
        let h = hex(&[0x00, 0x0f, 0xa0, 0xff]);
        assert_eq!(h, "000fa0ff");
        assert!(h.bytes().all(|b| b.is_ascii_hexdigit()));
    }
}
