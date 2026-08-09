//! Google — OpenID Connect, code flow + PKCE (contrat §5.2).
//!
//! Les endpoints sont écrits en dur : le document de découverte de Google n'a pas changé
//! depuis dix ans et le relire à chaque connexion ajoute un aller-retour réseau, une
//! dépendance de disponibilité et un cache de plus à invalider.

use std::sync::Mutex;

use axum::{
    extract::{Query, State},
    http::HeaderMap,
    response::Response,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    auth::{self, session, JwksCache},
    error::{AppError, Result},
    util, AppState,
};

const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const JWKS_URL: &str = "https://www.googleapis.com/oauth2/v3/certs";
const ISSUERS: [&str; 2] = ["https://accounts.google.com", "accounts.google.com"];

static JWKS: JwksCache = Mutex::new(None);

pub async fn start(State(st): State<AppState>) -> Result<Response> {
    let g = st.cfg.google.as_ref().ok_or_else(|| {
        AppError::validation("La connexion avec Google n'est pas disponible sur cette instance.")
    })?;

    let s = auth::OauthState {
        state: util::random_token(),
        verifier: util::random_token(),
        nonce: util::random_token(),
        exp: chrono::Utc::now().timestamp() + session::OAUTH_MAX_AGE,
    };
    let redirect_uri = format!("{}/api/auth/google/callback", st.cfg.app_url);
    let challenge = auth::pkce_challenge(&s.verifier);

    let url = format!(
        "{AUTH_URL}?{}",
        auth::qs(&[
            ("client_id", g.client_id.as_str()),
            ("redirect_uri", redirect_uri.as_str()),
            ("response_type", "code"),
            ("scope", "openid email profile"),
            ("state", s.state.as_str()),
            ("nonce", s.nonce.as_str()),
            ("code_challenge", challenge.as_str()),
            ("code_challenge_method", "S256"),
            ("prompt", "select_account"),
        ])
    );

    let cookie = session::set_cookie(
        &session::cookie_name(&st.cfg, session::OAUTH_COOKIE),
        &auth::seal(&st.cfg.secret_key, &s)?,
        session::OAUTH_MAX_AGE,
        "Lax",
        session::secure(&st.cfg),
    );
    Ok(auth::redirect(&url, [cookie]))
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

pub async fn callback(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<CallbackQuery>,
) -> Response {
    match exchange(&st, &headers, q).await {
        Ok(user_id) => match auth::finish_login(&st, &headers, user_id).await {
            Ok(r) => r,
            Err(e) => auth::oauth_failure(&st, "google", e),
        },
        Err(e) => auth::oauth_failure(&st, "google", e),
    }
}

#[derive(Deserialize)]
struct TokenResponse {
    id_token: String,
}

async fn exchange(st: &AppState, headers: &HeaderMap, q: CallbackQuery) -> Result<Uuid> {
    let g = st
        .cfg
        .google
        .as_ref()
        .ok_or_else(|| AppError::validation("Google non configuré."))?;

    if let Some(err) = q.error {
        return Err(AppError::validation(format!(
            "Google a refusé la connexion ({err})."
        )));
    }
    let code = q
        .code
        .ok_or_else(|| AppError::validation("Réponse Google incomplète."))?;

    let raw = session::read_cookie(
        headers,
        &session::cookie_name(&st.cfg, session::OAUTH_COOKIE),
    )
    .ok_or_else(|| AppError::validation("Connexion expirée, recommencez."))?;
    let saved = auth::open(&st.cfg.secret_key, &raw)
        .ok_or_else(|| AppError::validation("Connexion expirée, recommencez."))?;

    if q.state.as_deref() != Some(saved.state.as_str()) {
        return Err(AppError::validation("Requête de connexion invalide."));
    }

    let redirect_uri = format!("{}/api/auth/google/callback", st.cfg.app_url);
    let token: TokenResponse = st
        .http
        .post(TOKEN_URL)
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code.as_str()),
            ("client_id", g.client_id.as_str()),
            ("client_secret", g.client_secret.as_str()),
            ("redirect_uri", redirect_uri.as_str()),
            ("code_verifier", saved.verifier.as_str()),
        ])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let claims =
        auth::verify_id_token(st, &token.id_token, JWKS_URL, &JWKS, &ISSUERS, &g.client_id).await?;

    if claims.nonce.as_deref() != Some(saved.nonce.as_str()) {
        return Err(AppError::validation("Requête de connexion invalide."));
    }

    // Sans e-mail vérifié on ne peut ni rattacher un compte existant ni faire confiance à
    // l'adresse : Google ne devrait pas arriver là, on refuse plutôt que de deviner.
    let verified = claims.email_verified.as_ref().is_some_and(|v| v.is_true());
    let email = claims.email.filter(|_| verified).ok_or_else(|| {
        AppError::validation("Votre adresse Google n'est pas vérifiée : impossible de continuer.")
    })?;

    session::find_or_create_user(
        &st.db,
        &email,
        true,
        claims.name.as_deref(),
        claims.picture.as_deref(),
        Some(("google", claims.sub.as_str())),
    )
    .await
}
