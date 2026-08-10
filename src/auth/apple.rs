//! Sign in with Apple — code flow + `response_mode=form_post` (contrat §5.2).
//!
//! Deux pièges spécifiques à Apple, tous deux traités ici :
//! 1. le callback est un **POST cross-site**, donc le cookie de `state` doit être
//!    `SameSite=None; Secure`, sinon le navigateur ne le renvoie pas et toute connexion
//!    échoue en « state invalide » ;
//! 2. le nom de l'utilisateur n'est transmis qu'à la **toute première** autorisation, dans
//!    le champ de formulaire `user`. Non capturé là, il est définitivement perdu.

use std::sync::Mutex;

use anyhow::anyhow;
use axum::{
    extract::{Query, State},
    http::HeaderMap,
    response::Response,
    Form,
};
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    auth::{self, session, JwksCache},
    config::AppleConfig,
    error::{AppError, Result},
    util, AppState,
};

const AUTH_URL: &str = "https://appleid.apple.com/auth/authorize";
const TOKEN_URL: &str = "https://appleid.apple.com/auth/token";
const JWKS_URL: &str = "https://appleid.apple.com/auth/keys";
const ISSUER: &str = "https://appleid.apple.com";
/// Apple autorise jusqu'à 6 mois. On reste largement en dessous : le secret est regénéré
/// à chaque connexion, un jeton de longue durée n'aurait aucun intérêt.
const SECRET_TTL: i64 = 30 * 24 * 3600;

static JWKS: JwksCache = Mutex::new(None);

pub async fn start(
    State(st): State<AppState>,
    Query(q): Query<auth::StartQuery>,
) -> Result<Response> {
    let a = st.cfg.apple.as_ref().ok_or_else(|| {
        AppError::validation("La connexion avec Apple n'est pas disponible sur cette instance.")
    })?;

    let s = auth::OauthState {
        state: util::random_token(),
        verifier: util::random_token(),
        nonce: util::random_token(),
        exp: chrono::Utc::now().timestamp() + session::OAUTH_MAX_AGE,
        // §11.3 : l'attribution voyage dans le cookie d'état, jamais dans un cookie à elle.
        referral: q.referral,
    };
    let redirect_uri = format!("{}/api/auth/apple/callback", st.cfg.app_url);
    let challenge = auth::pkce_challenge(&s.verifier);

    let url = format!(
        "{AUTH_URL}?{}",
        auth::qs(&[
            ("client_id", a.client_id.as_str()),
            ("redirect_uri", redirect_uri.as_str()),
            ("response_type", "code"),
            ("response_mode", "form_post"),
            ("scope", "name email"),
            ("state", s.state.as_str()),
            ("nonce", s.nonce.as_str()),
            ("code_challenge", challenge.as_str()),
            ("code_challenge_method", "S256"),
        ])
    );

    let cookie = session::set_cookie(
        &session::cookie_name(&st.cfg, session::OAUTH_COOKIE),
        &auth::seal(&st.cfg.secret_key, &s)?,
        session::OAUTH_MAX_AGE,
        // Le retour est un POST depuis appleid.apple.com : "Lax" ne survivrait pas.
        "None",
        session::secure(&st.cfg),
    );
    Ok(auth::redirect(&url, [cookie]))
}

#[derive(Deserialize)]
pub struct AppleCallback {
    code: Option<String>,
    state: Option<String>,
    /// JSON `{"name":{"firstName":..,"lastName":..},"email":..}`, première autorisation seulement.
    user: Option<String>,
    error: Option<String>,
}

pub async fn callback(
    State(st): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<AppleCallback>,
) -> Response {
    match exchange(&st, &headers, form).await {
        Ok(user_id) => match auth::finish_login(&st, &headers, user_id).await {
            Ok(r) => r,
            Err(e) => auth::oauth_failure(&st, "apple", e),
        },
        Err(e) => auth::oauth_failure(&st, "apple", e),
    }
}

#[derive(Deserialize)]
struct TokenResponse {
    id_token: String,
}

#[derive(Deserialize)]
struct AppleUser {
    name: Option<AppleName>,
}

#[derive(Deserialize)]
struct AppleName {
    #[serde(rename = "firstName")]
    first: Option<String>,
    #[serde(rename = "lastName")]
    last: Option<String>,
}

async fn exchange(st: &AppState, headers: &HeaderMap, form: AppleCallback) -> Result<Uuid> {
    let a = st
        .cfg
        .apple
        .as_ref()
        .ok_or_else(|| AppError::validation("Apple non configuré."))?;

    if let Some(err) = form.error {
        return Err(AppError::validation(format!(
            "Apple a refusé la connexion ({err})."
        )));
    }
    let code = form
        .code
        .ok_or_else(|| AppError::validation("Réponse Apple incomplète."))?;

    let raw = session::read_cookie(
        headers,
        &session::cookie_name(&st.cfg, session::OAUTH_COOKIE),
    )
    .ok_or_else(|| AppError::validation("Connexion expirée, recommencez."))?;
    let saved = auth::open(&st.cfg.secret_key, &raw)
        .ok_or_else(|| AppError::validation("Connexion expirée, recommencez."))?;

    if form.state.as_deref() != Some(saved.state.as_str()) {
        return Err(AppError::validation("Requête de connexion invalide."));
    }

    let redirect_uri = format!("{}/api/auth/apple/callback", st.cfg.app_url);
    let secret = client_secret(a)?;
    let token: TokenResponse = st
        .http
        .post(TOKEN_URL)
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code.as_str()),
            ("client_id", a.client_id.as_str()),
            ("client_secret", secret.as_str()),
            ("redirect_uri", redirect_uri.as_str()),
            ("code_verifier", saved.verifier.as_str()),
        ])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let claims = auth::verify_id_token(
        st,
        &token.id_token,
        JWKS_URL,
        &JWKS,
        &[ISSUER],
        &a.client_id,
    )
    .await?;

    if claims.nonce.as_deref() != Some(saved.nonce.as_str()) {
        return Err(AppError::validation("Requête de connexion invalide."));
    }

    let email = claims
        .email
        .ok_or_else(|| AppError::validation("Apple n'a pas transmis d'adresse e-mail."))?;
    // Apple ne renvoie `email_verified` que sous forme de chaîne, et l'omet parfois pour un
    // compte relais. L'adresse relais est émise par Apple : elle est vérifiée par construction.
    let verified = claims.email_verified.as_ref().is_none_or(|v| v.is_true());

    let name = form
        .user
        .as_deref()
        .and_then(|raw| serde_json::from_str::<AppleUser>(raw).ok())
        .and_then(|u| u.name)
        .map(|n| {
            [n.first.unwrap_or_default(), n.last.unwrap_or_default()]
                .join(" ")
                .trim()
                .to_string()
        })
        .filter(|n| !n.is_empty());

    session::find_or_create_user_referred(
        &st.db,
        &email,
        verified,
        name.as_deref(),
        None,
        Some(("apple", claims.sub.as_str())),
        saved.referral.as_deref(),
    )
    .await
}

/// `client_secret` Apple = JWT ES256 signé avec la clé .p8. Regénéré à chaque échange :
/// c'est quelques microsecondes de signature, contre un secret de longue durée à stocker.
fn client_secret(a: &AppleConfig) -> Result<String> {
    #[derive(Serialize)]
    struct Claims<'a> {
        iss: &'a str,
        iat: i64,
        exp: i64,
        aud: &'a str,
        sub: &'a str,
    }

    let now = chrono::Utc::now().timestamp();
    let mut header = Header::new(Algorithm::ES256);
    header.kid = Some(a.key_id.clone());

    let key = EncodingKey::from_ec_pem(a.private_key.as_bytes())
        .map_err(|e| AppError::Internal(anyhow!("APPLE_PRIVATE_KEY illisible : {e}")))?;

    jsonwebtoken::encode(
        &header,
        &Claims {
            iss: &a.team_id,
            iat: now,
            exp: now + SECRET_TTL,
            aud: ISSUER,
            sub: &a.client_id,
        },
        &key,
    )
    .map_err(|e| AppError::Internal(anyhow!("signature du client_secret Apple : {e}")))
}
