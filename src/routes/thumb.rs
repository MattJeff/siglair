//! `GET /api/signatures/{id}/thumb.png` — la vignette du tableau de bord.
//!
//! Elle n'existe que pour une raison, et c'est une raison de correction, pas de confort :
//! `/s/{slug}.png` **enregistre une ouverture** (contrat §5.1). Afficher la vignette du
//! tableau de bord depuis l'URL publique faisait donc compter une ouverture chaque fois
//! qu'un client regardait ses propres signatures. Les statistiques sont une fonctionnalité
//! vendue ; fausses, elles valent moins que pas de statistiques du tout.
//!
//! Même fichier PNG, même `Storage`, contrôle d'accès par `OrgAccess`, **aucun événement**.

use axum::{
    extract::{Path, State},
    http::header,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use uuid::Uuid;

use crate::{
    auth::OrgAccess,
    error::{AppError, Result},
    AppState,
};

/// Cache **privé** : la réponse est derrière une session, un cache partagé n'a rien à y
/// garder. 60 s suffisent à absorber un rechargement de page sans figer une republication.
const CACHE: &str = "private, max-age=60";

pub fn router() -> Router<AppState> {
    Router::new().route("/signatures/{id}/thumb.png", get(thumb))
}

async fn thumb(
    State(st): State<AppState>,
    access: OrgAccess,
    Path(id): Path<Uuid>,
) -> Result<Response> {
    // Le filtre `org_id` est le contrôle d'accès : une signature d'une autre organisation
    // n'existe pas (404, pas 403 — ne pas confirmer son existence). Même règle que
    // `signatures::fetch`.
    let key: Option<String> = sqlx::query_scalar(
        "SELECT r.png_key FROM signatures s JOIN renders r ON r.id = s.published_render_id \
         WHERE s.id = $1 AND s.org_id = $2 AND s.deleted_at IS NULL",
    )
    .bind(id)
    .bind(access.org_id)
    .fetch_optional(&st.db)
    .await?;

    // Pas encore publiée : 404. Le tableau de bord n'affiche l'`<img>` que si
    // `published_render_id` est non nul, il n'y a pas de pixel de repli à inventer ici.
    let bytes = st.storage.get(&key.ok_or(AppError::NotFound)?).await?;

    Ok((
        [
            (header::CONTENT_TYPE, "image/png"),
            (header::CACHE_CONTROL, CACHE),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        bytes,
    )
        .into_response())
}
