//! Sessions opaques (contrat §5.2) : le cookie `sig_session` porte l'uuid d'une ligne
//! `sessions`. Pas de JWT — révoquer, c'est `DELETE`, pas une liste noire à maintenir.

use axum::http::{header::COOKIE, HeaderMap};
use chrono::{DateTime, Duration, Utc};
use serde_json::json;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::{
    auth::middleware::client_ip,
    config::Config,
    error::{AppError, Result},
    util, AppState,
};

pub const SESSION_COOKIE: &str = "sig_session";
/// Cookie court portant `state` + `code_verifier` PKCE pendant l'aller-retour OAuth.
pub const OAUTH_COOKIE: &str = "sig_oauth";
pub const SESSION_MAX_AGE: i64 = 30 * 24 * 3600;
pub const OAUTH_MAX_AGE: i64 = 600;

/// `__Host-` interdit au navigateur d'accepter le cookie sans `Secure`, sans `Path=/`
/// ou avec un `Domain` : un sous-domaine compromis ne peut plus écraser la session.
/// Impossible en http, donc réservé aux déploiements https.
pub fn cookie_name(cfg: &Config, base: &str) -> String {
    if secure(cfg) {
        format!("__Host-{base}")
    } else {
        base.to_string()
    }
}

pub fn secure(cfg: &Config) -> bool {
    cfg.app_url.starts_with("https://")
}

pub fn read_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all(COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|s| s.split(';'))
        .filter_map(|kv| kv.split_once('='))
        .find(|(k, _)| k.trim() == name)
        .map(|(_, v)| v.trim().to_string())
}

/// `same_site` : "Lax" partout sauf le retour Apple, qui arrive en POST cross-site et exige
/// "None". Un `SameSite=None` sans `Secure` est rejeté par les navigateurs : en http local
/// on retombe sur "Lax" (Apple impose de toute façon une redirection https).
pub fn set_cookie(name: &str, value: &str, max_age: i64, same_site: &str, secure: bool) -> String {
    let same_site = if same_site == "None" && !secure {
        "Lax"
    } else {
        same_site
    };
    let mut c =
        format!("{name}={value}; Path=/; HttpOnly; SameSite={same_site}; Max-Age={max_age}");
    if secure {
        c.push_str("; Secure");
    }
    c
}

pub fn clear_cookie(name: &str, secure: bool) -> String {
    set_cookie(name, "", 0, "Lax", secure)
}

#[derive(Debug, Clone)]
pub struct Session {
    pub id: Uuid,
    pub user_id: Uuid,
    pub email: String,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
}

pub async fn create_session(
    db: &PgPool,
    user_id: Uuid,
    headers: &HeaderMap,
    ip_salt: &str,
) -> Result<Uuid> {
    let id = Uuid::new_v4();
    let ua: Option<String> = headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.chars().take(300).collect());
    let ip_hash = client_ip(headers).map(|ip| util::hash_ip(&ip, ip_salt));

    sqlx::query(
        "INSERT INTO sessions (id, user_id, expires_at, user_agent, ip_hash)
         VALUES ($1, $2, now() + interval '30 days', $3, $4)",
    )
    .bind(id)
    .bind(user_id)
    .bind(ua)
    .bind(ip_hash)
    .execute(db)
    .await?;
    Ok(id)
}

pub async fn destroy_session(db: &PgPool, id: Uuid) -> Result<()> {
    sqlx::query("DELETE FROM sessions WHERE id = $1")
        .bind(id)
        .execute(db)
        .await?;
    Ok(())
}

/// `None` si la session est inconnue ou expirée.
pub async fn lookup_session(db: &PgPool, id: Uuid) -> Result<Option<Session>> {
    let row = sqlx::query(
        // `::text` : `citext` est un type d'extension, sqlx ne le décode pas en String.
        "SELECT s.user_id, s.last_seen_at, u.email::text AS email, u.name, u.avatar_url
         FROM sessions s JOIN users u ON u.id = s.user_id
         WHERE s.id = $1 AND s.expires_at > now()",
    )
    .bind(id)
    .fetch_optional(db)
    .await?;

    let Some(row) = row else { return Ok(None) };
    let user_id: Uuid = row.try_get("user_id")?;
    let last_seen: DateTime<Utc> = row.try_get("last_seen_at")?;

    // Rafraîchir `last_seen_at` à chaque requête ajouterait une écriture à chaque appel API
    // pour une donnée dont la précision utile se compte en heures.
    if Utc::now() - last_seen > Duration::hours(1) {
        sqlx::query("UPDATE sessions SET last_seen_at = now() WHERE id = $1")
            .bind(id)
            .execute(db)
            .await?;
        sqlx::query("UPDATE users SET last_seen_at = now() WHERE id = $1")
            .bind(user_id)
            .execute(db)
            .await?;
    }

    Ok(Some(Session {
        id,
        user_id,
        email: row.try_get("email")?,
        name: row.try_get("name")?,
        avatar_url: row.try_get("avatar_url")?,
    }))
}

/// Retrouve l'utilisateur d'une identité externe, ou le crée avec son organisation
/// personnelle. Renvoie son id.
///
/// `provider` : `Some((provider, provider_uid))` pour Google/Apple, `None` pour le magic
/// link (qui prouve la possession de la boîte, pas une identité tierce).
///
/// **Le rattachement par e-mail n'a lieu que si l'e-mail est vérifié par le fournisseur.**
/// Sinon n'importe qui déclarant `email: victime@exemple.fr` chez un fournisseur laxiste
/// prendrait le contrôle d'un compte existant.
/// Longueur maximale d'un code de parrainage accepté. Un slug en fait 12
/// (`util::gen_slug`) ; au-delà de 64 c'est du bruit, et ça n'a rien à faire dans une
/// requête SQL ni dans une colonne.
const MAX_REFERRAL_LEN: usize = 64;

/// Même chose, avec l'attribution du contrat §11.3.
///
/// `referral` est le code lu **dans l'URL** — `/r/{slug}` → `/?ref={code}` → `/login?ref=`,
/// puis le corps de `POST /api/auth/magic/request` ou le paramètre de départ OAuth. Aucun
/// cookie n'est posé sur le destinataire de l'e-mail : il n'est pas notre utilisateur, il
/// n'a rien accepté, et un cookie d'attribution imposerait un bandeau de consentement qui
/// ferait chuter la conversion qu'on mesure.
///
/// Le code n'est inscrit qu'à la **création** du compte. Une connexion ultérieure avec un
/// autre `?ref=` ne réécrit rien : l'origine d'un compte ne change pas, et laisser un lien
/// de parrainage réattribuer un compte existant est exactement ce qui rend une prime de
/// parrainage fraudable.
pub async fn find_or_create_user_referred(
    st: &AppState,
    email: &str,
    email_verified: bool,
    name: Option<&str>,
    avatar: Option<&str>,
    provider: Option<(&str, &str)>,
    referral: Option<&str>,
) -> Result<Uuid> {
    // `AppState` plutôt que le pool seul : la création d'un compte doit aussi joindre le
    // contact à la liste marketing Brevo, et ce chemin est le seul par lequel un
    // utilisateur naît. Le brancher chez les trois appelants OAuth/magic, c'est l'oublier.
    let db = &st.db;
    let email = email.trim().to_lowercase();
    let referral = referral
        .map(str::trim)
        .filter(|c| !c.is_empty() && c.len() <= MAX_REFERRAL_LEN);

    if let Some((p, uid)) = provider {
        let existing: Option<Uuid> = sqlx::query_scalar(
            "SELECT user_id FROM identities WHERE provider = $1 AND provider_uid = $2",
        )
        .bind(p)
        .bind(uid)
        .fetch_optional(db)
        .await?;
        if let Some(user_id) = existing {
            // Apple ne renvoie le nom qu'à la première autorisation : ne jamais l'écraser
            // par un NULL des connexions suivantes.
            sqlx::query(
                "UPDATE users SET name = coalesce($2, name), avatar_url = coalesce($3, avatar_url)
                 WHERE id = $1",
            )
            .bind(user_id)
            .bind(name)
            .bind(avatar)
            .execute(db)
            .await?;
            return Ok(user_id);
        }
    }

    let by_email: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM users WHERE email = $1::citext")
            .bind(&email)
            .fetch_optional(db)
            .await?;

    if let Some(user_id) = by_email {
        if !email_verified {
            return Err(AppError::conflict(
                "Un compte existe déjà avec cette adresse. Connectez-vous par e-mail pour \
                 la rattacher à ce fournisseur.",
            ));
        }
        if let Some((p, uid)) = provider {
            sqlx::query(
                "INSERT INTO identities (id, user_id, provider, provider_uid) VALUES ($1, $2, $3, $4)
                 ON CONFLICT (provider, provider_uid) DO NOTHING",
            )
            .bind(Uuid::new_v4())
            .bind(user_id)
            .bind(p)
            .bind(uid)
            .execute(db)
            .await?;
        }
        sqlx::query(
            "UPDATE users SET email_verified = true, name = coalesce(name, $2),
                              avatar_url = coalesce(avatar_url, $3)
             WHERE id = $1",
        )
        .bind(user_id)
        .bind(name)
        .bind(avatar)
        .execute(db)
        .await?;
        return Ok(user_id);
    }

    create_user(st, &email, email_verified, name, avatar, provider, referral).await
}

/// Utilisateur + organisation personnelle + appartenance : une seule transaction. Un
/// utilisateur sans org serait invisible de toutes les routes applicatives.
async fn create_user(
    st: &AppState,
    email: &str,
    email_verified: bool,
    name: Option<&str>,
    avatar: Option<&str>,
    provider: Option<(&str, &str)>,
    referral: Option<&str>,
) -> Result<Uuid> {
    let db = &st.db;
    let user_id = Uuid::new_v4();
    let display = name
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| local_part(email));

    let mut tx = db.begin().await?;

    // §11.3 : le code brut est conservé tel quel, et la signature d'origine est résolue en
    // même temps par sous-requête. Un code inconnu (signature supprimée, lien recopié de
    // travers) laisse simplement `referred_by_signature_id` à NULL — une inscription ne
    // rate jamais parce qu'une attribution n'a pas abouti.
    //
    // Pas de filtre sur `deleted_at`, comme dans `/r/{code}` : un e-mail déjà parti
    // continue de créditer son auteur même s'il a supprimé le brouillon depuis.
    sqlx::query(
        "INSERT INTO users (id, email, email_verified, name, avatar_url,
                            referral_code, referred_by_signature_id)
         VALUES ($1, $2::citext, $3, $4, $5, $6,
                 (SELECT id FROM signatures WHERE public_slug = $6::citext))",
    )
    .bind(user_id)
    .bind(email)
    .bind(email_verified)
    .bind(name)
    .bind(avatar)
    .bind(referral)
    .execute(&mut *tx)
    .await?;

    if let Some((p, uid)) = provider {
        sqlx::query(
            "INSERT INTO identities (id, user_id, provider, provider_uid) VALUES ($1, $2, $3, $4)",
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(p)
        .bind(uid)
        .execute(&mut *tx)
        .await?;
    }

    let org_name = format!("Espace de {display}");
    let base = {
        let s = util::slugify(display);
        if s.is_empty() {
            util::gen_slug()
        } else {
            s
        }
    };
    let mut slug = base.clone();
    let mut org_id = None;
    for _ in 0..6 {
        // ON CONFLICT DO NOTHING plutôt qu'une erreur rattrapée : dans une transaction,
        // une violation d'unicité invalide tout le reste.
        let inserted: Option<Uuid> = sqlx::query_scalar(
            "INSERT INTO orgs (id, name, slug) VALUES ($1, $2, $3::citext)
             ON CONFLICT (slug) DO NOTHING RETURNING id",
        )
        .bind(Uuid::new_v4())
        .bind(&org_name)
        .bind(&slug)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(id) = inserted {
            org_id = Some(id);
            break;
        }
        slug = format!("{base}-{}", &util::gen_slug()[..5]);
    }
    let org_id = org_id.ok_or_else(|| {
        AppError::Internal(anyhow::anyhow!(
            "aucun slug d'organisation libre pour « {base} »"
        ))
    })?;

    // Profil pré-rempli : {{name}} et {{email}} résolvent dès la première signature.
    sqlx::query(
        "INSERT INTO org_members (org_id, user_id, role, profile) VALUES ($1, $2, 'owner', $3)",
    )
    .bind(org_id)
    .bind(user_id)
    .bind(json!({ "name": display, "email": email }))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    // Après le commit, et détaché : un aller-retour vers Brevo n'a pas à tenir la
    // transaction ouverte, ni à faire attendre — encore moins échouer — une inscription.
    let (st, email, display) = (st.clone(), email.to_string(), display.to_string());
    tokio::spawn(async move {
        crate::email::sync_contact(&st, &email, Some(&display)).await;
    });

    Ok(user_id)
}

fn local_part(email: &str) -> &str {
    email.split('@').next().unwrap_or(email)
}
