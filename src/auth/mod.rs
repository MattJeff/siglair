//! Authentification — contrat §5.2.
//!
//! Trois portes d'entrée (Google OIDC, Apple, magic link) qui convergent toutes vers
//! `session::find_or_create_user_referred` puis `session::create_session`.

pub mod apple;
pub mod google;
pub mod magic;
pub mod middleware;
pub mod session;

// Surface publique du module : les autres modules importent `crate::auth::OrgAccess`,
// pas `crate::auth::middleware::OrgAccess`.
pub use middleware::{client_ip, current_org, load_org, CurrentUser, OptionalUser, OrgAccess};

use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

use anyhow::anyhow;
use axum::{
    extract::State,
    http::{header::SET_COOKIE, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
    Json, Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64URL, Engine as _};
use hmac::{Hmac, Mac};
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::{
    error::{AppError, Result},
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/auth/google/start", get(google::start))
        .route("/api/auth/google/callback", get(google::callback))
        .route("/api/auth/apple/start", get(apple::start))
        // Apple répond en `form_post` : le callback est un POST cross-site, pas un GET.
        .route("/api/auth/apple/callback", post(apple::callback))
        .route("/api/auth/magic/request", post(magic::request))
        .route("/api/auth/magic/consume", get(magic::consume))
        .route("/api/auth/logout", post(logout))
        // §5.2 : `/api/me` est monté par `routes::me`, qui y ajoute PATCH et DELETE. Une
        // route littérale `/api/me` ici gagnerait sur le `nest("/api")` et ferait répondre
        // 405 aux deux autres verbes. Le handler `me` reste ici, il est réutilisé tel quel.
        .route("/api/orgs/invites/accept", post(accept_invite))
}

/// `GET /api/me` — contrat §5.2.
pub async fn me(State(st): State<AppState>, user: CurrentUser) -> Result<Json<Value>> {
    type OrgRow = (
        uuid::Uuid,
        String,
        String,
        String,
        Option<String>,
        Option<chrono::DateTime<chrono::Utc>>,
        String,
    );
    let rows = sqlx::query_as::<_, OrgRow>(
        "SELECT o.id, o.name, o.slug::text, o.plan, o.subscription_status, o.grace_until, m.role
         FROM org_members m JOIN orgs o ON o.id = m.org_id
         WHERE m.user_id = $1 ORDER BY m.created_at, o.id",
    )
    .bind(user.id)
    .fetch_all(&st.db)
    .await?;

    // Même règle que partout ailleurs : le plan annoncé est celui qui donne accès. Le
    // sélecteur d'organisation ne doit pas afficher « Pro » sur une org déjà retombée.
    let orgs: Vec<Value> = rows
        .iter()
        .map(|(id, name, slug, plan, status, grace, role)| {
            let effective =
                crate::billing::lifecycle::effective_plan(&crate::billing::lifecycle::OrgBilling {
                    plan: plan.clone(),
                    subscription_status: status.clone(),
                    grace_until: *grace,
                });
            json!({ "id": id, "name": name, "slug": slug, "plan": effective.id, "role": role })
        })
        .collect();

    let current = middleware::current_org(&st.db, user.id).await?;

    // `members` sert au plan Team : l'écran Équipe compare les sièges utilisés aux sièges
    // facturés. Sans ce compte, il affiche « — / 3 » et l'utilisateur ne sait pas où il en est.
    let (signatures, assets_bytes, members): (i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM signatures WHERE org_id = $1 AND deleted_at IS NULL),
                (SELECT coalesce(sum(bytes), 0)::bigint FROM assets WHERE org_id = $1),
                (SELECT count(*) FROM org_members WHERE org_id = $1)",
    )
    .bind(current.org_id)
    .fetch_one(&st.db)
    .await?;

    Ok(Json(json!({
        "user": {
            "id": user.id,
            "email": user.email,
            "name": user.name,
            "avatar_url": user.avatar_url,
        },
        "orgs": orgs,
        "current_org": {
            "id": current.org_id,
            "name": current.name,
            "slug": current.slug,
            "role": current.role,
            "seats": current.seats,
            "analytics_enabled": current.analytics_enabled,
        },
        "plan": current.plan.id,
        // `plan.limits`, pas `plan` : depuis que Plan porte les prix, il CONTIENT un champ
        // `limits`. Renvoyer l'objet entier ici obligeait le front à lire
        // `limits.limits.signatures` — il lisait donc `undefined`, et le bandeau de quota
        // restait vide. Les noms suivent `interface Limits` / `interface Usage` de
        // web/src/lib/types.ts : cette frontière n'est typée nulle part, c'est à la main.
        "limits": current.plan.limits,
        "usage": {
            "signatures": signatures,
            "assets_bytes": assets_bytes,
            "members": members,
        },
    })))
}

pub async fn logout(State(st): State<AppState>, headers: HeaderMap) -> Result<Response> {
    let name = session::cookie_name(&st.cfg, session::SESSION_COOKIE);
    if let Some(id) = session::read_cookie(&headers, &name).and_then(|v| v.parse().ok()) {
        session::destroy_session(&st.db, id).await?;
    }
    let cookie = session::clear_cookie(&name, session::secure(&st.cfg));
    Ok(with_cookies(
        StatusCode::NO_CONTENT.into_response(),
        [cookie],
    ))
}

/// `POST /api/orgs/invites/accept` — l'invitation part par e-mail depuis
/// `routes::orgs::create_invite` ; sans ce point de sortie le lien reçu ne mène nulle part
/// et le plan Team n'est pas utilisable. Route absente du tableau §5.3, ajoutée au plus
/// près de ce qui existe (signalée dans le rapport d'intégration).
///
/// Le jeton est comparé par son sha256 (`util::hash_token`) : la base n'a jamais le clair.
pub async fn accept_invite(
    State(st): State<AppState>,
    user: CurrentUser,
    Json(req): Json<AcceptInvite>,
) -> Result<Json<Value>> {
    let expired = || {
        AppError::validation(
            "Cette invitation n'est plus valable. Demandez à l'organisation de vous en \
             envoyer une nouvelle.",
        )
    };

    // L'e-mail de l'invitation doit être celui du compte connecté : sinon un lien intercepté
    // suffirait à entrer dans l'organisation avec n'importe quel compte.
    let row: Option<(uuid::Uuid, uuid::Uuid, String)> = sqlx::query_as(
        "SELECT id, org_id, role FROM invites \
         WHERE token_hash = $1 AND accepted_at IS NULL AND expires_at > now() \
           AND email = $2::citext",
    )
    .bind(crate::util::hash_token(&req.token))
    .bind(&user.email)
    .fetch_optional(&st.db)
    .await?;
    let (invite_id, org_id, role) = row.ok_or_else(expired)?;

    let mut tx = st.db.begin().await?;
    // `accepted_at IS NULL` dans le UPDATE : deux clics simultanés ne consomment qu'une fois.
    let consumed =
        sqlx::query("UPDATE invites SET accepted_at = now() WHERE id = $1 AND accepted_at IS NULL")
            .bind(invite_id)
            .execute(&mut *tx)
            .await?
            .rows_affected();
    if consumed == 0 {
        return Err(expired());
    }
    // Déjà membre : on ne rétrograde ni ne promeut par invitation.
    sqlx::query(
        "INSERT INTO org_members (org_id, user_id, role) VALUES ($1, $2, $3) \
         ON CONFLICT (org_id, user_id) DO NOTHING",
    )
    .bind(org_id)
    .bind(user.id)
    .bind(&role)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    // Après le commit, jamais dedans : un aller-retour Stripe de 15 s tiendrait un verrou
    // sur `invites` et `org_members`. Best effort — un membre entre même si Stripe est
    // injoignable, le webhook `customer.subscription.updated` rattrape la vérité.
    if let Err(e) = crate::billing::sync_seats(&st, org_id).await {
        tracing::warn!(error = ?e, %org_id, "sièges non synchronisés chez Stripe");
    }

    Ok(Json(json!({ "org_id": org_id, "role": role })))
}

#[derive(Deserialize)]
pub struct AcceptInvite {
    token: String,
}

// ---------------------------------------------------------------- réponses

pub(crate) fn with_cookies(
    mut resp: Response,
    cookies: impl IntoIterator<Item = String>,
) -> Response {
    for c in cookies {
        if let Ok(v) = HeaderValue::from_str(&c) {
            resp.headers_mut().append(SET_COOKIE, v);
        }
    }
    resp
}

pub(crate) fn redirect(to: &str, cookies: impl IntoIterator<Item = String>) -> Response {
    with_cookies(Redirect::to(to).into_response(), cookies)
}

/// Session posée, cookie OAuth effacé, retour sur l'application.
pub(crate) async fn finish_login(
    st: &AppState,
    headers: &HeaderMap,
    user_id: uuid::Uuid,
) -> Result<Response> {
    finish_login_to(st, headers, user_id, "/app").await
}

/// Variante bornée à un chemin interne construit par le serveur. Elle sert au relais
/// d'onboarding : après le magic link, on réclame le brouillon au lieu d'atterrir au dashboard.
pub(crate) async fn finish_login_to(
    st: &AppState,
    headers: &HeaderMap,
    user_id: uuid::Uuid,
    path: &str,
) -> Result<Response> {
    let sid = session::create_session(&st.db, user_id, headers, &st.cfg.ip_salt).await?;
    let secure = session::secure(&st.cfg);
    let path = if path.starts_with('/') && !path.starts_with("//") {
        path
    } else {
        "/app"
    };
    Ok(redirect(
        &format!("{}{path}", st.cfg.app_url),
        [
            session::set_cookie(
                &session::cookie_name(&st.cfg, session::SESSION_COOKIE),
                &sid.to_string(),
                session::SESSION_MAX_AGE,
                "Lax",
                secure,
            ),
            session::clear_cookie(
                &session::cookie_name(&st.cfg, session::OAUTH_COOKIE),
                secure,
            ),
        ],
    ))
}

pub(crate) fn qs(params: &[(&str, &str)]) -> String {
    serde_urlencoded::to_string(params).unwrap_or_default()
}

/// Échec d'un flux OAuth : on renvoie l'utilisateur sur le front avec un code court.
/// La cause détaillée reste dans les logs (contrat §5.4).
pub(crate) fn oauth_failure(st: &AppState, reason: &str, err: impl std::fmt::Display) -> Response {
    tracing::warn!(reason, error = %err, "échec de connexion OAuth");
    let cookie = session::clear_cookie(
        &session::cookie_name(&st.cfg, session::OAUTH_COOKIE),
        session::secure(&st.cfg),
    );
    redirect(
        &format!("{}/login?error={reason}", st.cfg.app_url),
        [cookie],
    )
}

// ---------------------------------------------------------------- cookie signé state/PKCE

/// Contenu du cookie `sig_oauth`. Signé (HMAC-SHA256) plutôt que stocké en base : c'est
/// une donnée à durée de vie de 10 minutes, une table de plus n'apporterait rien.
#[derive(Serialize, Deserialize)]
pub(crate) struct OauthState {
    pub state: String,
    pub verifier: String,
    pub nonce: String,
    /// Le `Max-Age` du cookie est un souhait adressé au navigateur ; l'expiration réelle
    /// se vérifie ici.
    pub exp: i64,
    /// Code de parrainage (§11.3), le temps de l'aller-retour chez le fournisseur.
    ///
    /// Il voyage dans ce cookie-ci et pas dans un nouveau : `sig_oauth` existe déjà, il est
    /// strictement nécessaire au flux (il porte `state` et le `verifier` PKCE), il dure dix
    /// minutes et il n'est posé que sur quelqu'un qui vient de cliquer « Continuer avec… ».
    /// Ce n'est pas le cookie interdit par §11.3 — celui-là serait posé sur le destinataire
    /// de l'e-mail, qui n'a rien demandé. Aucun cookie nouveau n'est créé.
    ///
    /// `default` : les cookies déjà en vol au déploiement n'ont pas ce champ.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub referral: Option<String>,
}

/// Query string de `/api/auth/{provider}/start`. Un seul champ, et il est facultatif :
/// un départ sans `?ref=` reste un départ valide.
#[derive(Deserialize, Default)]
pub struct StartQuery {
    #[serde(default, rename = "ref")]
    pub referral: Option<String>,
}

pub(crate) fn seal(secret: &[u8], s: &OauthState) -> Result<String> {
    let payload = B64URL.encode(serde_json::to_vec(s)?);
    Ok(format!("{payload}.{}", mac(secret, &payload)))
}

pub(crate) fn open(secret: &[u8], raw: &str) -> Option<OauthState> {
    let (payload, tag) = raw.split_once('.')?;
    let mut m = Hmac::<Sha256>::new_from_slice(secret).ok()?;
    m.update(payload.as_bytes());
    // `verify_slice` compare en temps constant — pas un `==` sur des chaînes.
    m.verify_slice(&B64URL.decode(tag).ok()?).ok()?;
    let s: OauthState = serde_json::from_slice(&B64URL.decode(payload).ok()?).ok()?;
    (s.exp > chrono::Utc::now().timestamp()).then_some(s)
}

fn mac(secret: &[u8], payload: &str) -> String {
    let mut m = Hmac::<Sha256>::new_from_slice(secret).expect("HMAC accepte toute longueur de clé");
    m.update(payload.as_bytes());
    B64URL.encode(m.finalize().into_bytes())
}

/// `code_challenge` PKCE S256 du vérificateur.
pub(crate) fn pkce_challenge(verifier: &str) -> String {
    B64URL.encode(Sha256::digest(verifier.as_bytes()))
}

// ---------------------------------------------------------------- vérification des id_token

/// Certains fournisseurs (Apple) sérialisent `email_verified` en chaîne.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum FlexBool {
    Bool(bool),
    Str(String),
}

impl FlexBool {
    pub fn is_true(&self) -> bool {
        match self {
            FlexBool::Bool(b) => *b,
            FlexBool::Str(s) => s == "true",
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct IdClaims {
    pub sub: String,
    pub email: Option<String>,
    pub email_verified: Option<FlexBool>,
    pub name: Option<String>,
    pub picture: Option<String>,
    pub nonce: Option<String>,
}

#[derive(Clone, Deserialize)]
pub(crate) struct Jwk {
    kid: String,
    n: String,
    e: String,
}

#[derive(Deserialize)]
struct Jwks {
    keys: Vec<Jwk>,
}

pub(crate) type JwksCache = Mutex<Option<(Instant, Vec<Jwk>)>>;
const JWKS_TTL: Duration = Duration::from_secs(3600);

/// Vérifie signature + `iss` + `aud` + `exp`. Google et Apple signent tous deux en RS256.
pub(crate) async fn verify_id_token(
    st: &AppState,
    token: &str,
    jwks_url: &str,
    cache: &'static JwksCache,
    issuers: &[&str],
    audience: &str,
) -> Result<IdClaims> {
    let kid = decode_header(token)
        .ok()
        .and_then(|h| h.kid)
        .ok_or_else(|| AppError::Internal(anyhow!("id_token sans kid")))?;

    let jwk = jwk_for(st, jwks_url, cache, &kid).await?;
    let key = DecodingKey::from_rsa_components(&jwk.n, &jwk.e)
        .map_err(|e| AppError::Internal(anyhow!("clé JWKS illisible : {e}")))?;

    let mut v = Validation::new(Algorithm::RS256);
    v.set_issuer(issuers);
    v.set_audience(&[audience]);

    decode::<IdClaims>(token, &key, &v)
        .map(|d| d.claims)
        .map_err(|e| AppError::Internal(anyhow!("id_token invalide : {e}")))
}

async fn jwk_for(st: &AppState, url: &str, cache: &'static JwksCache, kid: &str) -> Result<Jwk> {
    if let Some(k) = cached(cache, kid) {
        return Ok(k);
    }
    // Cache froid *ou* `kid` inconnu : dans les deux cas il faut relire le JWKS, sinon une
    // rotation de clé chez le fournisseur casse les connexions pendant une heure.
    let jwks: Jwks = st
        .http
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    if let Ok(mut g) = cache.lock() {
        *g = Some((Instant::now(), jwks.keys.clone()));
    }
    jwks.keys
        .into_iter()
        .find(|k| k.kid == kid)
        .ok_or_else(|| AppError::Internal(anyhow!("clé {kid} absente du JWKS {url}")))
}

fn cached(cache: &JwksCache, kid: &str) -> Option<Jwk> {
    let g = cache.lock().ok()?;
    let (at, keys) = g.as_ref()?;
    if at.elapsed() > JWKS_TTL {
        return None;
    }
    keys.iter().find(|k| k.kid == kid).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sealed_state_survives_a_round_trip_and_rejects_tampering() {
        let secret = b"0123456789abcdef0123456789abcdef";
        let s = OauthState {
            state: "st".into(),
            verifier: "ver".into(),
            nonce: "no".into(),
            exp: chrono::Utc::now().timestamp() + 60,
            referral: Some("abcdefgh2345".into()),
        };
        let sealed = seal(secret, &s).unwrap();
        assert_eq!(open(secret, &sealed).unwrap().verifier, "ver");
        // §11.3 : le code de parrainage doit survivre à l'aller-retour chez le fournisseur.
        assert_eq!(
            open(secret, &sealed).unwrap().referral.as_deref(),
            Some("abcdefgh2345")
        );
        assert!(open(b"un autre secret de 32 octets....", &sealed).is_none());

        let (payload, tag) = sealed.split_once('.').unwrap();
        assert!(open(secret, &format!("{payload}x.{tag}")).is_none());

        let expired = OauthState {
            exp: chrono::Utc::now().timestamp() - 1,
            ..s
        };
        assert!(open(secret, &seal(secret, &expired).unwrap()).is_none());
    }

    #[test]
    fn pkce_challenge_matches_rfc7636_example() {
        assert_eq!(
            pkce_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }
}
