//! Magic link — contrat §5.2. C'est aussi le chemin d'inscription : une adresse inconnue
//! crée le compte à la consommation du lien.

use std::{net::IpAddr, time::Duration};

use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::Response,
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    auth::{self, middleware::client_ip, session},
    email,
    error::{AppError, Result},
    util, AppState,
};

const TTL_MINUTES: i32 = 15;
const RATE_WINDOW: Duration = Duration::from_secs(3600);
const RATE_MAX: usize = 5;

/// Contrat §5.2 : 5 demandes par heure et par e-mail, et autant par IP.
fn allow(key: String) -> bool {
    util::rate_limit(&key, RATE_MAX, RATE_WINDOW)
}

#[derive(Deserialize)]
pub struct MagicRequest {
    email: String,
    #[serde(default)]
    handoff: Option<String>,
    /// Code de parrainage (§11.3). Il voyage dans l'URL du lien, pas dans un cookie.
    #[serde(default, rename = "ref")]
    referral: Option<String>,
}

/// Toujours 204 : une 404 sur adresse inconnue transformerait l'endpoint en oracle
/// répondant « qui a un compte chez Siglair ».
pub async fn request(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<MagicRequest>,
) -> Result<StatusCode> {
    let addr = body.email.trim().to_lowercase();
    if !valid_email(&addr) {
        return Err(AppError::validation(
            "Cette adresse e-mail n'est pas valide.",
        ));
    }

    let ip = client_ip(&headers);
    if !allow(format!("e:{addr}")) || !allow(format!("i:{}", ip.as_deref().unwrap_or("?"))) {
        return Err(AppError::RateLimited);
    }

    let token = util::random_token();
    sqlx::query(
        "INSERT INTO magic_links (id, email, token_hash, expires_at, requested_ip)
         VALUES ($1, $2::citext, $3, now() + make_interval(mins => $4), $5::text::inet)",
    )
    .bind(Uuid::new_v4())
    .bind(&addr)
    .bind(util::hash_token(&token))
    .bind(TTL_MINUTES)
    .bind(ip.filter(|s| s.parse::<IpAddr>().is_ok()))
    .execute(&st.db)
    .await?;

    // Le lien clair n'existe qu'ici et dans l'e-mail : la base ne stocke que son sha256.
    let handoff = body
        .handoff
        .as_deref()
        .filter(|value| crate::ai::handoff::valid(&st, value));
    let mut params = vec![("token", token.as_str())];
    if let Some(value) = handoff {
        params.push(("handoff", value));
    }
    // §11.3 : le code traverse la connexion dans l'URL et n'est inscrit qu'à la création
    // du compte. Sans ce relais, l'attribution s'arrête à la boîte mail.
    if let Some(value) = body.referral.as_deref().filter(|v| !v.trim().is_empty()) {
        params.push(("ref", value));
    }
    let link = format!(
        "{}/api/auth/magic/consume?{}",
        st.cfg.app_url,
        auth::qs(&params)
    );
    email::send_magic_link(&st, &addr, &link, TTL_MINUTES).await?;

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct ConsumeQuery {
    token: String,
    handoff: Option<String>,
    #[serde(rename = "ref")]
    referral: Option<String>,
}

pub async fn consume(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<ConsumeQuery>,
) -> Result<Response> {
    // Un SELECT puis un UPDATE laisserait deux requêtes simultanées consommer le même lien.
    // Le `WHERE used_at IS NULL ... RETURNING` rend l'usage unique atomique.
    let email: Option<String> = sqlx::query_scalar(
        "UPDATE magic_links SET used_at = now()
         WHERE token_hash = $1 AND used_at IS NULL AND expires_at > now()
         RETURNING email::text",
    )
    .bind(util::hash_token(&q.token))
    .fetch_optional(&st.db)
    .await?;

    let Some(email) = email else {
        let to = format!("{}/login?error=magic_expired", st.cfg.app_url);
        return Ok(auth::redirect(&to, Vec::<String>::new()));
    };

    // Recevoir le lien prouve la possession de la boîte : l'adresse est vérifiée.
    let user_id = session::find_or_create_user_referred(
        &st,
        &email,
        true,
        None,
        None,
        None,
        q.referral.as_deref(),
    )
    .await?;
    let destination = q
        .handoff
        .as_deref()
        .filter(|value| crate::ai::handoff::valid(&st, value))
        .map(|value| format!("/onboarding?{}", auth::qs(&[("handoff", value)])))
        .unwrap_or_else(|| "/app".into());
    auth::finish_login_to(&st, &headers, user_id, &destination).await
}

/// Validation volontairement laxiste : le vrai test est que l'e-mail arrive.
fn valid_email(s: &str) -> bool {
    let Some((local, domain)) = s.split_once('@') else {
        return false;
    };
    s.len() <= 254
        && !s.chars().any(char::is_whitespace)
        && !local.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !domain.contains('@')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emails() {
        assert!(valid_email("a@b.fr"));
        assert!(valid_email("prenom.nom+tag@sous.domaine.example"));
        assert!(!valid_email("a@b"));
        assert!(!valid_email("@b.fr"));
        assert!(!valid_email("a b@c.fr"));
        assert!(!valid_email("a@b.fr@c.fr"));
        assert!(!valid_email("pasdarobase.fr"));
    }

    #[test]
    fn rate_limit_stops_at_five() {
        let k = || format!("test:{}", line!());
        assert!((0..RATE_MAX).all(|_| allow(k())));
        assert!(!allow(k()));
    }
}
