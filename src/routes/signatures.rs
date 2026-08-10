//! CRUD des signatures, publication et export — contrat §5.3 et §7.
//!
//! Tout est filtré par `access.org_id` : une requête ne peut pas nommer une signature
//! d'une autre organisation, elle reçoit 404 (pas 403 — ne pas confirmer l'existence).

use std::collections::HashSet;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    ai::generate::{self, EditorAsset},
    auth::OrgAccess,
    doc::{Doc, Profile},
    error::{AppError, Result},
    plans::{check_signature_quota, Plan},
    render::{html::render_document, RenderMode, RenderOpts},
    routes::{analytics, assets},
    util::{doc_hash, gen_slug},
    AppState,
};

/// Fenêtre des plafonds de débit du contrat §8.
const HOUR: std::time::Duration = std::time::Duration::from_secs(3600);

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/signatures", get(list).post(create))
        .route(
            "/signatures/{id}",
            get(get_one).patch(update).delete(remove),
        )
        .route("/signatures/{id}/publish", post(publish))
        .route("/signatures/{id}/ai", post(ai_edit))
        .route("/signatures/{id}/status", get(status))
        .route("/signatures/{id}/export", get(export))
        .route("/signatures/{id}/analytics", get(stats))
        .route("/signatures/{id}/duplicate", post(duplicate))
}

/// `public_slug` est un `citext` : sans `::text` sqlx ne sait pas le décoder.
const COLS: &str = "id, org_id, owner_user_id, name, kind, doc, profile, \
                    public_slug::text AS public_slug, published_render_id, created_at, updated_at";

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct SignatureRow {
    pub id: Uuid,
    pub org_id: Uuid,
    pub owner_user_id: Option<Uuid>,
    pub name: String,
    pub kind: String,
    pub doc: Value,
    pub profile: Value,
    pub public_slug: Option<String>,
    pub published_render_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

async fn fetch(db: &PgPool, org_id: Uuid, id: Uuid) -> Result<SignatureRow> {
    sqlx::query_as::<_, SignatureRow>(&format!(
        "SELECT {COLS} FROM signatures WHERE id = $1 AND org_id = $2 AND deleted_at IS NULL"
    ))
    .bind(id)
    .bind(org_id)
    .fetch_optional(db)
    .await?
    .ok_or(AppError::NotFound)
}

/// Une signature personnelle appartient à son auteur. L'administration transversale est
/// une capacité Team : owner/admin peut alors gérer les signatures et modèles de l'org.
fn can_mutate(access: &OrgAccess, row: &SignatureRow) -> bool {
    let team_admin = access.plan.id == "team" && matches!(access.role.as_str(), "owner" | "admin");
    if row.kind == "org_template" {
        return team_admin;
    }
    row.owner_user_id == Some(access.user_id) || team_admin
}

fn require_mutation(access: &OrgAccess, row: &SignatureRow) -> Result<()> {
    if can_mutate(access, row) {
        Ok(())
    } else {
        Err(AppError::Forbidden)
    }
}

fn doc_of(row: &SignatureRow) -> Result<Doc> {
    Ok(serde_json::from_value(row.doc.clone())?)
}

fn profile_of(row: &SignatureRow) -> Profile {
    serde_json::from_value(row.profile.clone()).unwrap_or_default()
}

async fn count_live(db: &PgPool, org_id: Uuid) -> Result<u32> {
    let n: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM signatures WHERE org_id = $1 AND deleted_at IS NULL",
    )
    .bind(org_id)
    .fetch_one(db)
    .await?;
    Ok(n.max(0) as u32)
}

fn clean_name(raw: &str) -> Result<String> {
    let n = raw.trim();
    if n.is_empty() {
        return Err(AppError::validation("Donnez un nom à votre signature."));
    }
    if n.chars().count() > 120 {
        return Err(AppError::validation(
            "Le nom d'une signature est limité à 120 caractères.",
        ));
    }
    Ok(n.to_string())
}

fn clean_kind(raw: Option<&str>, plan: &Plan) -> Result<&'static str> {
    match raw.unwrap_or("personal") {
        "personal" => Ok("personal"),
        "org_template" if plan.limits.org_templates => Ok("org_template"),
        "org_template" => Err(AppError::QuotaExceeded(
            "Les modèles d'organisation sont inclus dans le plan Team. Passez à Team pour \
             déployer une même signature sur toute votre équipe."
                .into(),
        )),
        _ => Err(AppError::validation(
            "Type de signature inconnu : attendu personal ou org_template.",
        )),
    }
}

// ------------------------------------------------------------------ lecture

async fn list(State(st): State<AppState>, access: OrgAccess) -> Result<Json<Vec<SignatureRow>>> {
    let rows = sqlx::query_as::<_, SignatureRow>(&format!(
        "SELECT {COLS} FROM signatures WHERE org_id = $1 AND deleted_at IS NULL \
         ORDER BY updated_at DESC"
    ))
    .bind(access.org_id)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(rows))
}

async fn get_one(
    State(st): State<AppState>,
    access: OrgAccess,
    Path(id): Path<Uuid>,
) -> Result<Json<SignatureRow>> {
    Ok(Json(fetch(&st.db, access.org_id, id).await?))
}

// ------------------------------------------------------------------ écriture

#[derive(Deserialize)]
struct CreateReq {
    #[serde(default)]
    name: String,
    doc: Option<Doc>,
    kind: Option<String>,
    profile: Option<Profile>,
}

async fn create(
    State(st): State<AppState>,
    access: OrgAccess,
    Json(req): Json<CreateReq>,
) -> Result<(StatusCode, Json<SignatureRow>)> {
    let plan = access.plan;
    let name = clean_name(&req.name)?;
    let kind = clean_kind(req.kind.as_deref(), &plan)?;
    if kind == "org_template" {
        access.require_admin()?;
    }
    let doc = req.doc.unwrap_or_default();
    doc.validate()?;
    check_signature_quota(&plan, count_live(&st.db, access.org_id).await?)?;

    let row = sqlx::query_as::<_, SignatureRow>(&format!(
        "INSERT INTO signatures (id, org_id, owner_user_id, name, kind, doc, profile) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING {COLS}"
    ))
    .bind(Uuid::new_v4())
    .bind(access.org_id)
    .bind(access.user_id)
    .bind(&name)
    .bind(kind)
    .bind(serde_json::to_value(&doc)?)
    .bind(serde_json::to_value(req.profile.unwrap_or_default())?)
    .fetch_one(&st.db)
    .await?;
    Ok((StatusCode::CREATED, Json(row)))
}

#[derive(Deserialize)]
struct UpdateReq {
    name: Option<String>,
    doc: Option<Doc>,
    profile: Option<Profile>,
}

async fn update(
    State(st): State<AppState>,
    access: OrgAccess,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateReq>,
) -> Result<Json<SignatureRow>> {
    // ponytail : lecture puis écriture complète, sans verrou. Un éditeur a un seul auteur ;
    // le jour où deux onglets se battent, il faudra un numéro de version dans la table.
    let cur = fetch(&st.db, access.org_id, id).await?;
    require_mutation(&access, &cur)?;
    let name = match req.name {
        Some(n) => clean_name(&n)?,
        None => cur.name,
    };
    let doc = match req.doc {
        Some(d) => {
            d.validate()?;
            serde_json::to_value(&d)?
        }
        None => cur.doc,
    };
    let profile = match req.profile {
        Some(p) => serde_json::to_value(p)?,
        None => cur.profile,
    };

    let row = sqlx::query_as::<_, SignatureRow>(&format!(
        "UPDATE signatures SET name = $1, doc = $2, profile = $3, updated_at = now() \
         WHERE id = $4 AND org_id = $5 AND deleted_at IS NULL RETURNING {COLS}"
    ))
    .bind(&name)
    .bind(doc)
    .bind(profile)
    .bind(id)
    .bind(access.org_id)
    .fetch_optional(&st.db)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(Json(row))
}

/// Soft delete : le slug reste réservé pour qu'une URL déjà collée dans un client mail ne
/// se retrouve jamais réattribuée à la signature de quelqu'un d'autre.
async fn remove(
    State(st): State<AppState>,
    access: OrgAccess,
    Path(id): Path<Uuid>,
) -> Result<StatusCode> {
    let row = fetch(&st.db, access.org_id, id).await?;
    require_mutation(&access, &row)?;
    let n = sqlx::query(
        "UPDATE signatures SET deleted_at = now(), updated_at = now() \
         WHERE id = $1 AND org_id = $2 AND deleted_at IS NULL",
    )
    .bind(id)
    .bind(access.org_id)
    .execute(&st.db)
    .await?
    .rows_affected();
    if n == 0 {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn duplicate(
    State(st): State<AppState>,
    access: OrgAccess,
    Path(id): Path<Uuid>,
) -> Result<(StatusCode, Json<SignatureRow>)> {
    let plan = access.plan;
    let src = fetch(&st.db, access.org_id, id).await?;
    require_mutation(&access, &src)?;
    check_signature_quota(&plan, count_live(&st.db, access.org_id).await?)?;

    let name: String = format!("{} (copie)", src.name).chars().take(120).collect();
    // un modèle dupliqué reste un modèle tant que le plan le permet, sinon il redevient perso
    let kind = if src.kind == "org_template" && plan.limits.org_templates {
        "org_template"
    } else {
        "personal"
    };

    let row = sqlx::query_as::<_, SignatureRow>(&format!(
        "INSERT INTO signatures (id, org_id, owner_user_id, name, kind, doc, profile) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING {COLS}"
    ))
    .bind(Uuid::new_v4())
    .bind(access.org_id)
    .bind(access.user_id)
    .bind(&name)
    .bind(kind)
    .bind(src.doc)
    .bind(src.profile)
    .fetch_one(&st.db)
    .await?;
    Ok((StatusCode::CREATED, Json(row)))
}

// ------------------------------------------------------------------ copilote IA

#[derive(Deserialize)]
struct AiEditReq {
    #[serde(default)]
    message: String,
    selected_id: Option<String>,
}

/// Le modèle renvoie un document complet, mais il ne devient jamais une écriture aveugle :
/// même garde d'autorisation que PATCH, validation du contrat, contrôle des assets, puis UPDATE.
async fn ai_edit(
    State(st): State<AppState>,
    access: OrgAccess,
    Path(id): Path<Uuid>,
    Json(req): Json<AiEditReq>,
) -> Result<Json<Value>> {
    if access.plan.id == "free" {
        return Err(AppError::QuotaExceeded(format!(
            "Le copilote IA est inclus dans Pro ({}/mois) et Team.",
            crate::plans::PRO.price_label()
        )));
    }
    let message = req.message.trim();
    if message.is_empty() || message.chars().count() > 2_000 {
        return Err(AppError::validation(
            "Décrivez la modification souhaitée en 2 000 caractères maximum.",
        ));
    }

    let row = fetch(&st.db, access.org_id, id).await?;
    require_mutation(&access, &row)?;
    let current = doc_of(&row)?;
    if req.selected_id.as_ref().is_some_and(|selected| {
        !current
            .elements
            .iter()
            .any(|element| &element.id == selected)
    }) {
        return Err(AppError::validation("L'élément sélectionné n'existe plus."));
    }

    let assets: Vec<(Uuid, String, String)> = sqlx::query_as(
        "SELECT id, filename, kind FROM assets WHERE org_id = $1 ORDER BY created_at DESC LIMIT 100",
    )
    .bind(access.org_id)
    .fetch_all(&st.db)
    .await?;
    let allowed: HashSet<Uuid> = assets.iter().map(|asset| asset.0).collect();
    let assets: Vec<EditorAsset> = assets
        .into_iter()
        .map(|(id, filename, kind)| EditorAsset { id, filename, kind })
        .collect();
    let profile_keys: Vec<String> = profile_of(&row).keys().cloned().collect();

    crate::routes::onboarding::consume_ai_generation(&st, &access).await?;
    let generated = generate::edit_document(
        &st.http,
        message,
        &current,
        req.selected_id.as_deref(),
        &profile_keys,
        &assets,
    )
    .await;
    let output = match generated {
        Ok(output) => output,
        Err(reason) => {
            let _ = crate::routes::onboarding::refund_ai_generation(&st, &access).await;
            return Err(AppError::NotImplemented(format!(
                "Le copilote n'est pas disponible pour l'instant. {reason}"
            )));
        }
    };

    if let Err(error) = output.doc.validate() {
        let _ = crate::routes::onboarding::refund_ai_generation(&st, &access).await;
        tracing::warn!(?error, %id, "document du copilote refusé par le validateur");
        return Err(AppError::validation(
            "L'IA a proposé une composition invalide. Reformulez la demande plus simplement.",
        ));
    }
    if output
        .doc
        .elements
        .iter()
        .filter_map(|element| element.asset_id)
        .any(|asset_id| !allowed.contains(&asset_id))
    {
        let _ = crate::routes::onboarding::refund_ai_generation(&st, &access).await;
        return Err(AppError::validation(
            "L'IA a référencé un média qui n'appartient pas à votre bibliothèque.",
        ));
    }

    sqlx::query(
        "UPDATE signatures SET doc = $1, updated_at = now() \
         WHERE id = $2 AND org_id = $3 AND deleted_at IS NULL",
    )
    .bind(serde_json::to_value(&output.doc)?)
    .bind(id)
    .bind(access.org_id)
    .execute(&st.db)
    .await?;

    Ok(Json(json!({ "reply": output.reply, "doc": output.doc })))
}

// ------------------------------------------------------------------ publication

pub(crate) struct Published {
    pub slug: String,
    pub job_id: Option<Uuid>,
    pub render_id: Option<Uuid>,
}

/// Contrat §7.1. Partagé avec le rollout d'organisation.
pub(crate) async fn publish_doc(
    db: &PgPool,
    sig_id: Uuid,
    current_slug: Option<String>,
    doc: &Doc,
    profile: &Profile,
) -> Result<Published> {
    publish_snapshot(db, sig_id, current_slug, doc, profile, None).await
}

/// Met en file l'instantané exact à rasteriser. `campaign_id` ne modifie jamais le document
/// source : il sert seulement à savoir quand la campagne doit être retirée automatiquement.
pub(crate) async fn publish_snapshot(
    db: &PgPool,
    sig_id: Uuid,
    current_slug: Option<String>,
    doc: &Doc,
    profile: &Profile,
    campaign_id: Option<Uuid>,
) -> Result<Published> {
    // Le hash porte sur (doc, profil) : c'est la même empreinte que celle que le renderer
    // recalcule et écrit dans `renders.doc_hash`, sinon le cache §7.1 ne se croiserait jamais.
    let hash = doc_hash(doc, profile);
    let slug = match current_slug {
        Some(s) => s,
        None => assign_slug(db, sig_id).await?,
    };

    // « republier sans avoir rien changé » est le cas le plus fréquent : pas de nouveau job.
    let existing: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM renders WHERE signature_id = $1 AND doc_hash = $2 \
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(sig_id)
    .bind(&hash)
    .fetch_optional(db)
    .await?;
    if let Some(render_id) = existing {
        // Un rendu plus ancien peut encore être en cours. Le marqueur `done`, plus récent,
        // l'empêche de republier son état après ce retour immédiat au cache.
        let pending_other: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM render_jobs WHERE signature_id = $1 \
             AND status IN ('queued', 'running') \
             AND (doc_hash <> $2 OR campaign_id IS DISTINCT FROM $3))",
        )
        .bind(sig_id)
        .bind(&hash)
        .bind(campaign_id)
        .fetch_one(db)
        .await?;
        if pending_other {
            sqlx::query(
                "INSERT INTO render_jobs \
                   (id, signature_id, status, doc_hash, doc, profile, campaign_id, finished_at) \
                 VALUES ($1, $2, 'done', $3, $4, $5, $6, now())",
            )
            .bind(Uuid::new_v4())
            .bind(sig_id)
            .bind(&hash)
            .bind(serde_json::to_value(doc)?)
            .bind(serde_json::to_value(profile)?)
            .bind(campaign_id)
            .execute(db)
            .await?;
        }
        sqlx::query(
            "UPDATE signatures SET published_render_id = $1, published_campaign_id = $3, \
               campaign_base_doc = CASE WHEN $3::uuid IS NULL THEN NULL ELSE campaign_base_doc END, \
               campaign_base_profile = CASE WHEN $3::uuid IS NULL THEN NULL ELSE campaign_base_profile END, \
               updated_at = now() WHERE id = $2",
        )
        .bind(render_id)
        .bind(sig_id)
        .bind(campaign_id)
        .execute(db)
        .await?;
        return Ok(Published {
            slug,
            job_id: None,
            render_id: Some(render_id),
        });
    }

    // un double-clic sur « Publier » ne doit pas empiler deux fois le même travail
    let queued: Option<(Uuid, String, Option<Uuid>)> = sqlx::query_as(
        "SELECT id, status, campaign_id FROM render_jobs \
         WHERE signature_id = $1 AND doc_hash = $2 \
         AND status IN ('queued', 'running') ORDER BY created_at DESC LIMIT 1",
    )
    .bind(sig_id)
    .bind(&hash)
    .fetch_optional(db)
    .await?;
    let job_id = match queued {
        Some((id, status, queued_campaign))
            if status == "queued" || queued_campaign == campaign_id =>
        {
            sqlx::query(
                "UPDATE render_jobs SET doc = $2, profile = $3, campaign_id = $4 WHERE id = $1",
            )
            .bind(id)
            .bind(serde_json::to_value(doc)?)
            .bind(serde_json::to_value(profile)?)
            .bind(campaign_id)
            .execute(db)
            .await?;
            id
        }
        Some(_) | None => {
            let id = Uuid::new_v4();
            sqlx::query(
                "INSERT INTO render_jobs (id, signature_id, doc_hash, doc, profile, campaign_id) \
                 VALUES ($1, $2, $3, $4, $5, $6)",
            )
            .bind(id)
            .bind(sig_id)
            .bind(&hash)
            .bind(serde_json::to_value(doc)?)
            .bind(serde_json::to_value(profile)?)
            .bind(campaign_id)
            .execute(db)
            .await?;
            id
        }
    };
    Ok(Published {
        slug,
        job_id: Some(job_id),
        render_id: None,
    })
}

async fn assign_slug(db: &PgPool, sig_id: Uuid) -> Result<String> {
    for _ in 0..5 {
        let candidate = gen_slug();
        let res = sqlx::query(
            "UPDATE signatures SET public_slug = $1 WHERE id = $2 AND public_slug IS NULL",
        )
        .bind(&candidate)
        .bind(sig_id)
        .execute(db)
        .await;
        match res {
            Ok(r) if r.rows_affected() == 1 => return Ok(candidate),
            // publication concurrente : le slug déjà posé fait foi
            Ok(_) => {
                let cur: Option<String> =
                    sqlx::query_scalar("SELECT public_slug::text FROM signatures WHERE id = $1")
                        .bind(sig_id)
                        .fetch_one(db)
                        .await?;
                if let Some(slug) = cur {
                    return Ok(slug);
                }
            }
            Err(sqlx::Error::Database(e)) if e.is_unique_violation() => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Err(AppError::Internal(anyhow::anyhow!(
        "aucun slug libre après 5 tirages"
    )))
}

async fn publish(
    State(st): State<AppState>,
    access: OrgAccess,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>> {
    // §8 : une publication met un Chromium en file. Plafonné par organisation.
    if !crate::util::rate_limit(&format!("publish:{}", access.org_id), 30, HOUR) {
        return Err(AppError::RateLimited);
    }
    let plan = access.plan;
    if !plan.limits.hosted_gif {
        return Err(AppError::QuotaExceeded(
            "Le GIF hébergé est inclus à partir du plan Pro. Passez à Pro pour publier votre \
             signature à une URL et suivre ses ouvertures."
                .into(),
        ));
    }
    let row = fetch(&st.db, access.org_id, id).await?;
    require_mutation(&access, &row)?;
    let mut doc = doc_of(&row)?;
    // revalidation avant rendu : Chromium chargera ces URL depuis notre réseau (contrat §8),
    // et un DNS peut avoir changé de réponse depuis l'enregistrement.
    doc.validate()?;

    let profile = profile_of(&row);
    let campaign = crate::routes::campaigns::active_for_signature(
        &st.db,
        &access,
        row.owner_user_id,
        Utc::now(),
    )
    .await?;
    if let Some(c) = campaign.as_ref() {
        crate::routes::campaigns::remember_base(&st.db, row.id, &doc, &profile, true).await?;
        crate::routes::campaigns::apply_to_doc(&mut doc, c);
        doc.validate()?;
    }
    let p = publish_snapshot(
        &st.db,
        row.id,
        row.public_slug,
        &doc,
        &profile,
        campaign.as_ref().map(|c| c.id),
    )
    .await?;
    Ok(Json(json!({
        "slug": p.slug,
        "job_id": p.job_id,
        "render_id": p.render_id,
        "gif_url": format!("{}/s/{}.gif", st.cfg.public_url, p.slug),
        "png_url": format!("{}/s/{}.png", st.cfg.public_url, p.slug),
    })))
}

#[derive(Serialize, sqlx::FromRow)]
struct JobRow {
    id: Uuid,
    status: String,
    error: Option<String>,
    attempts: i32,
    created_at: DateTime<Utc>,
    finished_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, sqlx::FromRow)]
struct RenderInfo {
    id: Uuid,
    width: i32,
    height: i32,
    frames: i32,
    fps: i32,
    bytes: i64,
    created_at: DateTime<Utc>,
}

async fn status(
    State(st): State<AppState>,
    access: OrgAccess,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>> {
    let row = fetch(&st.db, access.org_id, id).await?;
    let job: Option<JobRow> = sqlx::query_as(
        "SELECT id, status, error, attempts, created_at, finished_at FROM render_jobs \
         WHERE signature_id = $1 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(row.id)
    .fetch_optional(&st.db)
    .await?;
    let render: Option<RenderInfo> = match row.published_render_id {
        Some(rid) => sqlx::query_as(
            "SELECT id, width, height, frames, fps, bytes, created_at FROM renders WHERE id = $1",
        )
        .bind(rid)
        .fetch_optional(&st.db)
        .await?,
        None => None,
    };
    let urls = row.public_slug.as_ref().map(|s| {
        json!({
            "gif": format!("{}/s/{}.gif", st.cfg.public_url, s),
            "png": format!("{}/s/{}.png", st.cfg.public_url, s),
        })
    });
    Ok(Json(
        json!({ "slug": row.public_slug, "job": job, "render": render, "urls": urls }),
    ))
}

// ------------------------------------------------------------------ export

#[derive(Deserialize)]
struct ExportQuery {
    #[serde(default)]
    mode: RenderMode,
}

async fn export(
    State(st): State<AppState>,
    access: OrgAccess,
    Path(id): Path<Uuid>,
    Query(q): Query<ExportQuery>,
) -> Result<Json<Value>> {
    let plan = access.plan;
    if q.mode == RenderMode::Hosted && !plan.limits.hosted_gif {
        return Err(AppError::QuotaExceeded(
            "L'export vers le GIF hébergé est inclus à partir du plan Pro. En Free, exportez \
             votre signature en mode statique."
                .into(),
        ));
    }
    let row = fetch(&st.db, access.org_id, id).await?;
    let doc = doc_of(&row)?;
    let opts = RenderOpts {
        public_url: st.cfg.public_url.clone(),
        // sans GIF hébergé il n'y a ni suivi de clic ni URL publique : liens directs
        slug: plan
            .limits
            .hosted_gif
            .then(|| row.public_slug.clone())
            .flatten(),
        assets: assets::urls_for_doc(&st, access.org_id, &doc).await?,
        for_capture: false,
        branding: plan.limits.branding,
    };
    let html = render_document(&doc, &profile_of(&row), q.mode, &opts);
    Ok(Json(json!({ "html": html, "mode": q.mode })))
}

async fn stats(
    State(st): State<AppState>,
    access: OrgAccess,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>> {
    analytics::for_signature(&st, &access, id).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plans::{PRO, TEAM};

    fn access(user_id: Uuid, role: &str, plan: Plan) -> OrgAccess {
        OrgAccess {
            org_id: Uuid::nil(),
            user_id,
            name: "Test".into(),
            slug: "test".into(),
            plan,
            seats: 1,
            analytics_enabled: true,
            role: role.into(),
        }
    }

    fn row(owner_user_id: Option<Uuid>, kind: &str) -> SignatureRow {
        let now = Utc::now();
        SignatureRow {
            id: Uuid::new_v4(),
            org_id: Uuid::nil(),
            owner_user_id,
            name: "Signature".into(),
            kind: kind.into(),
            doc: serde_json::to_value(Doc::default()).unwrap(),
            profile: json!({}),
            public_slug: None,
            published_render_id: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn personal_signatures_are_owned_unless_a_team_admin_manages_them() {
        let alice = Uuid::new_v4();
        let bob = Uuid::new_v4();
        let personal = row(Some(alice), "personal");

        assert!(can_mutate(&access(alice, "member", PRO), &personal));
        assert!(!can_mutate(&access(bob, "admin", PRO), &personal));
        assert!(!can_mutate(&access(bob, "member", TEAM), &personal));
        assert!(can_mutate(&access(bob, "admin", TEAM), &personal));
        assert!(can_mutate(&access(bob, "owner", TEAM), &personal));
    }

    #[test]
    fn organisation_templates_require_a_team_admin_even_when_created_by_the_user() {
        let alice = Uuid::new_v4();
        let template = row(Some(alice), "org_template");

        assert!(!can_mutate(&access(alice, "owner", PRO), &template));
        assert!(!can_mutate(&access(alice, "member", TEAM), &template));
        assert!(can_mutate(&access(alice, "admin", TEAM), &template));
    }
}
