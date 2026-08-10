//! `GET /api/config` — contrat §9. Public, sans session : le front l'appelle avant même
//! d'avoir un utilisateur, pour ne pas afficher un bouton « Continuer avec Apple » qui
//! renverrait une 500 faute de clé configurée.
//!
//! Les plans partent dans la même réponse : contrat §6, le front ne code jamais un quota
//! en dur, sinon un changement de grille demande deux déploiements et finit par diverger.

use axum::{extract::State, routing::get, Json, Router};
use serde::Serialize;

use crate::{
    ai::generate,
    config::Features,
    plans::{Plan, ALL},
    AppState,
};

#[derive(Serialize)]
struct ConfigOut {
    #[serde(flatten)]
    features: Features,
    plans: [Plan; 3],
    /// §6bis.5 : toujours vrai. Sans `AI_API_KEY` la fonctionnalité ne se désactive pas,
    /// le repli déterministe rend trois propositions — masquer le champ « collez votre
    /// site » couperait l'entonnoir d'acquisition pour une clé manquante.
    ai: bool,
    /// Vrai uniquement quand un fournisseur OpenAI-compatible est réellement branché.
    ai_provider: bool,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/api/config", get(config))
}

async fn config(State(st): State<AppState>) -> Json<ConfigOut> {
    Json(ConfigOut {
        features: st.cfg.features(),
        plans: ALL,
        ai: true,
        ai_provider: generate::provider_configured(),
    })
}
