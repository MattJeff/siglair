//! Routes applicatives — contrat §5.3. Montées sous `/api` par `src/bin/api.rs`.
//!
//! L'authentification passe par l'extracteur [`crate::auth::OrgAccess`] plutôt que par une
//! couche globale : un extracteur ne peut pas être oublié sur une route, le compilateur
//! refuse un handler qui n'en prend pas. `OrgAccess` porte l'utilisateur, l'organisation
//! courante et son rôle ; tout ce que renvoient ces routes en est filtré.

pub mod analytics;
pub mod assets;
pub mod campaigns;
pub mod events;
pub mod growth;
pub mod me;
pub mod meta;
pub mod onboarding;
pub mod orgs;
pub mod public;
// `/r/{code}` est publique (§5.1) : déclarée ici, mais montée à la racine par `bin/api.rs`,
// pas sous `/api` — un destinataire d'e-mail n'a pas de session.
pub mod referral;
pub mod signatures;
pub mod thumb;

use axum::{extract::State, routing::post, Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    auth::OrgAccess,
    doc::{Doc, Profile},
    error::{AppError, Result},
    render::{html::render_document, RenderMode, RenderOpts},
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .merge(signatures::router())
        .merge(assets::router())
        .merge(orgs::router())
        .merge(campaigns::router())
        .merge(me::router())
        .merge(events::router())
        .merge(growth::router())
        // `analyze` est publique (§6bis.6) : elle ne prend pas d'extracteur `CurrentUser`,
        // c'est tout ce qu'il faut ici — l'authentification n'est pas une couche, c'est un
        // extracteur. `generate` et `pick` en prennent un, donc exigent la session.
        .merge(onboarding::router())
        .merge(thumb::router())
        // `billing::router()` était écrit et documenté (« à monter ainsi »), mais monté
        // nulle part : les trois routes de l'écran Facturation répondaient 404.
        .nest("/billing", crate::billing::router())
        .route("/preview", post(preview))
        // sans ça, un `/api/inconnu` tomberait sur le fallback SPA et renverrait du HTML
        .fallback(|| async { AppError::NotFound })
}

// ------------------------------------------------------------------ aperçu éditeur

#[derive(Deserialize)]
struct PreviewReq {
    #[serde(default)]
    doc: Doc,
    #[serde(default)]
    profile: Profile,
    #[serde(default)]
    mode: RenderMode,
}

/// `POST /api/preview` — contrat §2 : l'éditeur n'a pas son propre moteur de rendu.
async fn preview(
    State(st): State<AppState>,
    access: OrgAccess,
    Json(req): Json<PreviewReq>,
) -> Result<Json<Value>> {
    // ponytail : pas de `Doc::validate` ici. La validation fait une résolution DNS (garde
    // SSRF) et l'aperçu part à chaque frappe ; `render_document` échappe et borne déjà tout
    // ce qu'il écrit. La validation reste obligatoire à l'écriture et à la publication.
    let plan = access.plan;
    let opts = RenderOpts {
        public_url: st.cfg.public_url.clone(),
        slug: None,
        assets: assets::urls_for_doc(&st, access.org_id, &req.doc).await?,
        for_capture: false,
        branding: plan.limits.branding,
        // Sans slug, `verify_row` ne produit rien de toute façon : l'aperçu de l'éditeur
        // porte sur un brouillon, qui n'a pas de page de vérification à montrer.
        verify_link: false,
    };
    Ok(Json(
        json!({ "html": render_document(&req.doc, &req.profile, req.mode, &opts) }),
    ))
}

#[cfg(test)]
mod tests {
    /// Axum ne signale un chemin monté deux fois qu'au montage — c'est-à-dire au démarrage,
    /// en production. Ce test assemble exactement ce qu'assemble `src/bin/api.rs`.
    /// Il est tombé une fois : `/api/me` était monté par `auth::router()` **et** par
    /// `routes::me`, ce qui compile parfaitement.
    #[test]
    fn le_routeur_complet_se_monte() {
        let _ = Router::<AppState>::new()
            .merge(super::public::router())
            .merge(super::referral::router())
            .merge(super::meta::router())
            .merge(crate::auth::router())
            .merge(crate::billing::webhook_router())
            .nest("/api", super::router());
    }

    use super::*;
}
