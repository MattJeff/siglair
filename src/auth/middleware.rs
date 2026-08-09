//! Extracteurs d'authentification et d'autorisation.
//!
//! `OrgAccess` (ou `load_org`) est le seul point où l'on décide qu'un utilisateur a le droit
//! de toucher une ressource. Une route applicative qui charge une ligne par son id sans
//! passer par ici est une fuite de données : les uuid ne sont pas un contrôle d'accès.

use std::collections::HashMap;

use axum::{
    extract::{FromRequestParts, Path},
    http::{request::Parts, HeaderMap},
};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::{
    auth::session::{self, SESSION_COOKIE},
    error::{AppError, Result},
    plans::Plan,
    AppState,
};

#[derive(Debug, Clone)]
pub struct CurrentUser {
    pub id: Uuid,
    pub session_id: Uuid,
    pub email: String,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
}

/// Même chose, mais sans 401 : pour les routes qui s'adaptent à l'état de connexion.
#[derive(Debug, Clone)]
pub struct OptionalUser(pub Option<CurrentUser>);

impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self> {
        current_user(parts, state)
            .await?
            .ok_or(AppError::Unauthorized)
    }
}

impl FromRequestParts<AppState> for OptionalUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self> {
        Ok(OptionalUser(current_user(parts, state).await?))
    }
}

async fn current_user(parts: &Parts, state: &AppState) -> Result<Option<CurrentUser>> {
    let name = session::cookie_name(&state.cfg, SESSION_COOKIE);
    let Some(raw) = session::read_cookie(&parts.headers, &name) else {
        return Ok(None);
    };
    let Ok(id) = raw.parse::<Uuid>() else {
        return Ok(None);
    };

    Ok(session::lookup_session(&state.db, id)
        .await?
        .map(|s| CurrentUser {
            id: s.user_id,
            session_id: s.id,
            email: s.email,
            name: s.name,
            avatar_url: s.avatar_url,
        }))
}

/// Organisation + rôle du membre. Obtenu uniquement via `load_org` / `current_org`, qui
/// refusent 403 quand l'utilisateur n'est pas membre.
#[derive(Debug, Clone)]
pub struct OrgAccess {
    pub org_id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub slug: String,
    pub plan: Plan,
    pub seats: i32,
    pub analytics_enabled: bool,
    /// owner | admin | member
    pub role: String,
}

impl OrgAccess {
    /// Inviter, retirer un membre, changer le plan : réservé aux owners et admins.
    pub fn require_admin(&self) -> Result<()> {
        if self.role == "owner" || self.role == "admin" {
            Ok(())
        } else {
            Err(AppError::Forbidden)
        }
    }
}

pub async fn load_org(db: &PgPool, user_id: Uuid, org_id: Uuid) -> Result<OrgAccess> {
    let row = sqlx::query(
        "SELECT o.name, o.slug::text AS slug, o.plan, o.seats, o.analytics_enabled, m.role
         FROM org_members m JOIN orgs o ON o.id = m.org_id
         WHERE m.org_id = $1 AND m.user_id = $2",
    )
    .bind(org_id)
    .bind(user_id)
    .fetch_optional(db)
    .await?;

    // Non-membre et org inexistante donnent la même réponse : sinon l'API confirme
    // l'existence d'organisations qui ne regardent pas l'appelant.
    let row = row.ok_or(AppError::Forbidden)?;
    let plan: String = row.try_get("plan")?;

    Ok(OrgAccess {
        org_id,
        user_id,
        name: row.try_get("name")?,
        slug: row.try_get("slug")?,
        plan: Plan::get(&plan),
        seats: row.try_get("seats")?,
        analytics_enabled: row.try_get("analytics_enabled")?,
        role: row.try_get("role")?,
    })
}

/// Organisation courante : celle choisie via `PATCH /api/me`, sinon la plus ancienne
/// appartenance — l'org personnelle créée à l'inscription.
///
/// Le `JOIN org_members` sur le choix explicite n'est pas décoratif : `users.current_org_id`
/// survit à un retrait de l'équipe (`ON DELETE SET NULL` ne vise que la suppression de
/// l'org), et sans lui un membre exclu resterait « dans » l'org jusqu'au 403 suivant.
pub async fn current_org(db: &PgPool, user_id: Uuid) -> Result<OrgAccess> {
    let org_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT m.org_id FROM org_members m \
         LEFT JOIN users u ON u.id = m.user_id \
         WHERE m.user_id = $1 \
         ORDER BY (m.org_id = u.current_org_id) DESC, m.created_at, m.org_id LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(db)
    .await?;

    load_org(db, user_id, org_id.ok_or(AppError::Forbidden)?).await
}

/// Deux usages, distingués par le seul paramètre de chemin `org_id` :
///
/// - `/api/orgs/{org_id}/...` : l'organisation est nommée dans le chemin, appartenance vérifiée.
/// - partout ailleurs (`/api/signatures`, `/api/assets`, `/api/preview`) : l'organisation
///   courante de l'utilisateur.
///
/// Le paramètre `{id}` n'est JAMAIS lu comme un org_id : sur `/api/signatures/{id}` il
/// désigne une signature, et le confondre avec une organisation rend toute la route
/// inutilisable (403 systématique) tout en compilant parfaitement.
///
/// Ailleurs, appeler `load_org` avec l'`org_id` de la ressource chargée (signature,
/// asset...) — jamais un id venu de la requête.
impl FromRequestParts<AppState> for OrgAccess {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self> {
        let user = CurrentUser::from_request_parts(parts, state).await?;
        let params = Path::<HashMap<String, String>>::from_request_parts(parts, state)
            .await
            .map(|p| p.0)
            .unwrap_or_default();
        match org_id_param(&params)? {
            Some(org_id) => load_org(&state.db, user.id, org_id).await,
            None => current_org(&state.db, user.id).await,
        }
    }
}

/// `Ok(None)` = pas d'organisation dans le chemin, l'org courante s'applique.
/// `Err(NotFound)` = `org_id` présent mais illisible ; retomber sur l'org courante
/// servirait silencieusement les données d'une autre organisation que celle demandée.
fn org_id_param(params: &HashMap<String, String>) -> Result<Option<Uuid>> {
    match params.get("org_id") {
        None => Ok(None),
        Some(raw) => raw.parse().map(Some).map_err(|_| AppError::NotFound),
    }
}

/// IP du client derrière le reverse-proxy. Sert au hachage (§4.1) et au rate limit ;
/// jamais stockée en clair.
pub fn client_ip(headers: &HeaderMap) -> Option<String> {
    let first = |v: &str| v.split(',').next().unwrap_or("").trim().to_string();
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .map(first)
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.trim().to_string())
        })
        .filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    /// Régression : `OrgAccess` lisait `org_id` **puis** `id`. Sur `/api/signatures/{id}`,
    /// `{id}` est une signature — la traiter comme une organisation faisait répondre 403 à
    /// toutes les routes applicatives, sans la moindre erreur de compilation.
    #[test]
    fn only_org_id_names_an_organisation() {
        let sig = Uuid::new_v4();
        assert_eq!(
            org_id_param(&params(&[("id", &sig.to_string())])).unwrap(),
            None
        );
        assert_eq!(org_id_param(&params(&[])).unwrap(), None);

        let org = Uuid::new_v4();
        assert_eq!(
            org_id_param(&params(&[("org_id", &org.to_string())])).unwrap(),
            Some(org)
        );
        // `/orgs/{org_id}/members/{user_id}` : les autres paramètres n'interfèrent pas
        let both = params(&[("org_id", &org.to_string()), ("user_id", &sig.to_string())]);
        assert_eq!(org_id_param(&both).unwrap(), Some(org));

        // un org_id illisible est un 404, jamais un repli silencieux sur l'org courante
        assert!(matches!(
            org_id_param(&params(&[("org_id", "pas-un-uuid")])),
            Err(AppError::NotFound)
        ));
    }
}
