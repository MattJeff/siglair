//! Organisation : membres, invitations, rôles et déploiement en masse — contrat §5.3 et §6.
//!
//! Règle qui traverse tout le fichier : une organisation garde toujours au moins un
//! propriétaire. Sans elle, un `owner` qui se rétrograde rend son organisation
//! inadministrable — personne ne peut plus inviter, ni facturer, ni supprimer.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, patch, post},
    Json, Router,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    auth::{client_ip, CurrentUser, OrgAccess},
    doc::{Doc, Profile},
    error::{AppError, Result},
    growth::{self, GrowthEvent, Visitor},
    routes::signatures,
    util::{hash_token, random_token, rate_limit},
    AppState,
};

/// Fenêtre des plafonds de débit du contrat §8.
const HOUR: std::time::Duration = std::time::Duration::from_secs(3600);

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/orgs/{org_id}", patch(update))
        .route("/orgs/{org_id}/members", get(members))
        .route(
            "/orgs/{org_id}/members/{user_id}",
            patch(update_member).delete(remove_member),
        )
        .route(
            "/orgs/{org_id}/invites",
            get(list_invites).post(create_invite),
        )
        .route("/orgs/{org_id}/rollout", post(rollout))
        // hors `/orgs/...` : l'invité n'est pas encore membre et n'a pas forcément de session
        .route("/invites/{token}", get(invite_preview))
}

/// Une organisation dont on n'est pas membre n'existe pas : 404, pas 403.
fn same_org(access: &OrgAccess, id: Uuid) -> Result<()> {
    if access.org_id == id {
        Ok(())
    } else {
        Err(AppError::NotFound)
    }
}

#[derive(Serialize, sqlx::FromRow)]
struct OrgRow {
    id: Uuid,
    name: String,
    slug: String,
    plan: String,
    seats: i32,
    analytics_enabled: bool,
}

const ORG_COLS: &str = "id, name, slug::text AS slug, plan, seats, analytics_enabled";

async fn load_org(db: &PgPool, id: Uuid) -> Result<OrgRow> {
    Ok(
        sqlx::query_as::<_, OrgRow>(&format!("SELECT {ORG_COLS} FROM orgs WHERE id = $1"))
            .bind(id)
            .fetch_one(db)
            .await?,
    )
}

// ------------------------------------------------------------------ organisation

#[derive(Deserialize)]
struct UpdateOrg {
    name: Option<String>,
    analytics_enabled: Option<bool>,
}

async fn update(
    State(st): State<AppState>,
    access: OrgAccess,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateOrg>,
) -> Result<Json<OrgRow>> {
    same_org(&access, id)?;
    access.require_admin()?;
    let name = match req.name.as_deref().map(str::trim) {
        Some(n) if n.is_empty() || n.chars().count() > 120 => {
            return Err(AppError::validation(
                "Le nom de l'organisation doit faire entre 1 et 120 caractères.",
            ))
        }
        other => other.map(str::to_string),
    };
    let row = sqlx::query_as::<_, OrgRow>(&format!(
        "UPDATE orgs SET name = coalesce($2, name), \
                         analytics_enabled = coalesce($3, analytics_enabled) \
         WHERE id = $1 RETURNING {ORG_COLS}"
    ))
    .bind(id)
    .bind(name)
    .bind(req.analytics_enabled)
    .fetch_one(&st.db)
    .await?;
    Ok(Json(row))
}

// ------------------------------------------------------------------ membres

#[derive(Serialize, sqlx::FromRow)]
struct MemberRow {
    user_id: Uuid,
    email: String,
    name: Option<String>,
    avatar_url: Option<String>,
    role: String,
    profile: Value,
    created_at: DateTime<Utc>,
}

const MEMBER_SQL: &str = "SELECT u.id AS user_id, u.email::text AS email, u.name, u.avatar_url, \
                                 m.role, m.profile, m.created_at \
                          FROM org_members m JOIN users u ON u.id = m.user_id \
                          WHERE m.org_id = $1";

async fn members(
    State(st): State<AppState>,
    access: OrgAccess,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<MemberRow>>> {
    same_org(&access, id)?;
    let sql = format!("{MEMBER_SQL} ORDER BY m.created_at");
    Ok(Json(sqlx::query_as(&sql).bind(id).fetch_all(&st.db).await?))
}

async fn owner_count(db: &PgPool, org_id: Uuid) -> Result<i64> {
    Ok(
        sqlx::query_scalar("SELECT count(*) FROM org_members WHERE org_id = $1 AND role = 'owner'")
            .bind(org_id)
            .fetch_one(db)
            .await?,
    )
}

async fn role_of(db: &PgPool, org_id: Uuid, user_id: Uuid) -> Result<String> {
    sqlx::query_scalar("SELECT role FROM org_members WHERE org_id = $1 AND user_id = $2")
        .bind(org_id)
        .bind(user_id)
        .fetch_optional(db)
        .await?
        .ok_or(AppError::NotFound)
}

const LAST_OWNER: &str = "Cette organisation doit garder au moins un propriétaire. \
                          Nommez d'abord quelqu'un d'autre propriétaire.";

/// Le garde du dernier propriétaire, en un seul endroit : rétrogradation (`next = Some(rôle)`)
/// et retrait (`next = None`) sont le même risque. Faux = l'opération laisserait
/// l'organisation sans propriétaire, donc inadministrable.
pub(crate) fn keeps_an_owner(current: &str, next: Option<&str>, owners: i64) -> bool {
    current != "owner" || next == Some("owner") || owners > 1
}

#[derive(Deserialize)]
struct MemberPatch {
    role: Option<String>,
    profile: Option<Profile>,
}

/// `PATCH /orgs/{id}/members/{user_id}` — deux gestes distincts sous une même route :
/// un admin change un rôle, un membre remplit son propre profil (celui que le rollout §5.3
/// injecte dans ses signatures). Les droits ne sont donc pas les mêmes selon le champ.
async fn update_member(
    State(st): State<AppState>,
    access: OrgAccess,
    Path((id, user_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<MemberPatch>,
) -> Result<Json<MemberRow>> {
    same_org(&access, id)?;
    if req.role.is_none() && req.profile.is_none() {
        return Err(AppError::validation(
            "Rien à modifier : indiquez un rôle ou un profil.",
        ));
    }
    // son propre profil ne demande aucun privilège ; celui d'un autre, oui
    if req.profile.is_some() && user_id != access.user_id {
        access.require_admin()?;
    }

    if let Some(role) = req.role.as_deref() {
        access.require_admin()?;
        if !matches!(role, "owner" | "admin" | "member") {
            return Err(AppError::validation(
                "Rôle inconnu : attendu owner, admin ou member.",
            ));
        }
        // seul un propriétaire peut en désigner un autre : un admin ne s'auto-promeut pas
        if role == "owner" && access.role != "owner" {
            return Err(AppError::Forbidden);
        }
        let current = role_of(&st.db, id, user_id).await?;
        if !keeps_an_owner(&current, Some(role), owner_count(&st.db, id).await?) {
            return Err(AppError::conflict(LAST_OWNER));
        }
    }

    let profile = req.profile.map(serde_json::to_value).transpose()?;
    sqlx::query(
        "UPDATE org_members SET role = coalesce($3, role), profile = coalesce($4, profile) \
         WHERE org_id = $1 AND user_id = $2",
    )
    .bind(id)
    .bind(user_id)
    .bind(req.role.as_deref())
    .bind(profile)
    .execute(&st.db)
    .await?;

    let sql = format!("{MEMBER_SQL} AND m.user_id = $2");
    sqlx::query_as(&sql)
        .bind(id)
        .bind(user_id)
        .fetch_optional(&st.db)
        .await?
        .map(Json)
        .ok_or(AppError::NotFound)
}

async fn remove_member(
    State(st): State<AppState>,
    access: OrgAccess,
    Path((id, user_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode> {
    same_org(&access, id)?;
    access.require_admin()?;
    let current = role_of(&st.db, id, user_id).await?;
    if !keeps_an_owner(&current, None, owner_count(&st.db, id).await?) {
        return Err(AppError::conflict(LAST_OWNER));
    }
    let n = sqlx::query("DELETE FROM org_members WHERE org_id = $1 AND user_id = $2")
        .bind(id)
        .bind(user_id)
        .execute(&st.db)
        .await?
        .rows_affected();
    if n == 0 {
        return Err(AppError::NotFound);
    }
    // Après le garde `n == 0` : un retrait qui n'a rien supprimé ne doit pas déclencher
    // d'écriture Stripe. Best effort, le retrait aboutit même si Stripe est injoignable.
    if let Err(e) = crate::billing::sync_seats(&st, id).await {
        tracing::warn!(error = ?e, org_id = %id, "sièges non synchronisés chez Stripe");
    }
    Ok(StatusCode::NO_CONTENT)
}

// ------------------------------------------------------------------ invitations

#[derive(Serialize, sqlx::FromRow)]
struct InviteRow {
    id: Uuid,
    email: String,
    role: String,
    expires_at: DateTime<Utc>,
    created_at: DateTime<Utc>,
}

async fn list_invites(
    State(st): State<AppState>,
    access: OrgAccess,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<InviteRow>>> {
    same_org(&access, id)?;
    access.require_admin()?;
    let rows = sqlx::query_as::<_, InviteRow>(
        "SELECT id, email::text AS email, role, expires_at, created_at FROM invites \
         WHERE org_id = $1 AND accepted_at IS NULL AND expires_at > now() \
         ORDER BY created_at DESC",
    )
    .bind(id)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(rows))
}

/// Tout ce que `GET /api/invites/{token}` a le droit de dire. Le jeton voyage dans une URL,
/// donc dans un historique, un presse-papier et parfois un canal partagé : cette réponse
/// doit rester servable à un inconnu. Ni l'adresse invitée (on la confirmerait à qui
/// possède le lien), ni la liste des membres, ni qui a invité.
#[derive(Serialize)]
struct InvitePreview {
    org: String,
    role: String,
}

/// Aperçu avant connexion : « vous êtes invité à rejoindre X en tant que Y ». Sans lui,
/// l'invité doit se connecter à l'aveugle pour savoir qui l'invite.
async fn invite_preview(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(token): Path<String>,
) -> Result<Json<InvitePreview>> {
    // Route publique portant un jeton dans l'URL : plafonnée par IP, sinon elle se prête à
    // l'énumération de jetons (contrat §8).
    let ip = client_ip(&headers).unwrap_or_default();
    if !rate_limit(&format!("invite-preview:{ip}"), 60, HOUR) {
        return Err(AppError::RateLimited);
    }
    // Comparaison par sha256 : la base n'a jamais le jeton en clair.
    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT o.name, i.role FROM invites i JOIN orgs o ON o.id = i.org_id \
         WHERE i.token_hash = $1 AND i.accepted_at IS NULL AND i.expires_at > now()",
    )
    .bind(hash_token(&token))
    .fetch_optional(&st.db)
    .await?;
    // Expirée, déjà acceptée ou inexistante : la même 404 dans les trois cas.
    let (org, role) = row.ok_or(AppError::NotFound)?;
    Ok(Json(InvitePreview { org, role }))
}

#[derive(Deserialize)]
struct InviteReq {
    email: String,
    role: Option<String>,
}

/// Vérification minimale : Brevo tranchera pour de bon. Refuser ici une adresse valide
/// avec une regex trop maligne coûte plus cher qu'un e-mail qui rebondit.
fn clean_email(raw: &str) -> Result<String> {
    let e = raw.trim().to_lowercase();
    let ok = (5..=254).contains(&e.len())
        && !e.contains(char::is_whitespace)
        && matches!(e.split_once('@'), Some((local, domain)) if !local.is_empty() && domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.'));
    if ok {
        Ok(e)
    } else {
        Err(AppError::validation(
            "Cette adresse e-mail n'est pas valide.",
        ))
    }
}

async fn create_invite(
    State(st): State<AppState>,
    access: OrgAccess,
    inviter: CurrentUser,
    Path(id): Path<Uuid>,
    Json(req): Json<InviteReq>,
) -> Result<(StatusCode, Json<InviteRow>)> {
    same_org(&access, id)?;
    access.require_admin()?;

    let org = load_org(&st.db, id).await?;
    let plan = access.plan;
    if !plan.limits.org_templates {
        // Le prix vient de `plans.rs` : recopié ici, il survivrait à un changement de tarif.
        return Err(AppError::QuotaExceeded(format!(
            "Les membres et les rôles sont inclus dans le plan Team ({prix}/membre, {min} \
             sièges minimum). Passez à Team pour inviter votre équipe.",
            prix = crate::plans::TEAM.price_label(),
            min = crate::plans::TEAM.min_seats,
        )));
    }

    let email = clean_email(&req.email)?;
    let role = match req.role.as_deref().unwrap_or("member") {
        // le rôle `owner` ne se donne pas par invitation, il se transmet après acceptation
        r @ ("admin" | "member") => r,
        _ => {
            return Err(AppError::validation(
                "Rôle d'invitation inconnu : attendu admin ou member.",
            ))
        }
    };

    let already: bool = sqlx::query_scalar(
        "SELECT exists(SELECT 1 FROM org_members m JOIN users u ON u.id = m.user_id \
         WHERE m.org_id = $1 AND u.email = $2::citext)",
    )
    .bind(id)
    .bind(&email)
    .fetch_one(&st.db)
    .await?;
    if already {
        return Err(AppError::conflict(
            "Cette personne fait déjà partie de l'organisation.",
        ));
    }

    // les sièges sont ce qui est facturé : une invitation en attente en consomme un
    let taken: i64 = sqlx::query_scalar(
        "SELECT (SELECT count(*) FROM org_members WHERE org_id = $1) \
              + (SELECT count(*) FROM invites \
                 WHERE org_id = $1 AND accepted_at IS NULL AND expires_at > now())",
    )
    .bind(id)
    .fetch_one(&st.db)
    .await?;
    if taken >= i64::from(org.seats) {
        return Err(AppError::QuotaExceeded(format!(
            "Vos {} sièges sont tous occupés (invitations en attente comprises). Ajoutez des \
             sièges depuis la facturation pour inviter quelqu'un de plus.",
            org.seats
        )));
    }

    let token = random_token();
    let expires_at = Utc::now() + Duration::days(7);
    let row = sqlx::query_as::<_, InviteRow>(
        "INSERT INTO invites (id, org_id, email, role, token_hash, invited_by, expires_at) \
         VALUES ($1, $2, $3::citext, $4, $5, $6, $7) \
         RETURNING id, email::text AS email, role, expires_at, created_at",
    )
    .bind(Uuid::new_v4())
    .bind(id)
    .bind(&email)
    .bind(role)
    .bind(hash_token(&token))
    .bind(access.user_id)
    .bind(expires_at)
    .fetch_one(&st.db)
    .await?;

    // le jeton en clair ne quitte le serveur que par l'e-mail ; la base n'en a que le sha256
    let url = format!("{}/invite/{}", st.cfg.app_url, token);
    if let Err(e) = crate::email::send_invite(
        &st,
        &email,
        &org.name,
        inviter.name.as_deref().unwrap_or(&inviter.email),
        &url,
    )
    .await
    {
        // sinon l'invitation resterait affichée « envoyée » alors que personne ne l'a reçue
        let _ = sqlx::query("DELETE FROM invites WHERE id = $1")
            .bind(row.id)
            .execute(&st.db)
            .await;
        return Err(e);
    }

    // §11.1 `team_invite` : « la boucle interne d'une organisation tourne-t-elle ». Après
    // l'envoi réussi seulement — une invitation que Brevo a refusée n'est pas partie, et
    // la ligne vient d'être supprimée juste au-dessus.
    let (db, salt) = (st.db.clone(), st.cfg.ip_salt.clone());
    let (org_id, inviter_id, role) = (id, access.user_id, role.to_string());
    tokio::spawn(async move {
        growth::record(
            &db,
            org_id,
            Some(inviter_id),
            None,
            GrowthEvent::TeamInvite,
            // Le rôle, jamais l'adresse : l'invité n'a pas de compte, son e-mail n'a rien à
            // faire dans une table de mesure (§4.1) — il est déjà dans `invites`.
            json!({ "role": role }),
            Visitor::unknown(&salt),
        )
        .await;
    });

    Ok((StatusCode::CREATED, Json(row)))
}

// ------------------------------------------------------------------ rollout

#[derive(Deserialize)]
struct RolloutReq {
    template_id: Uuid,
}

#[derive(Serialize)]
struct RolloutItem {
    user_id: Uuid,
    signature_id: Uuid,
    slug: String,
    job_id: Option<Uuid>,
}

#[derive(sqlx::FromRow)]
struct RolloutMember {
    user_id: Uuid,
    profile: Value,
    name: Option<String>,
    email: String,
}

/// `POST /orgs/{id}/rollout` — la fonctionnalité qui justifie le plan Team : un modèle,
/// N membres, N signatures cohérentes, chacune avec le profil de son porteur.
async fn rollout(
    State(st): State<AppState>,
    access: OrgAccess,
    Path(id): Path<Uuid>,
    Json(req): Json<RolloutReq>,
) -> Result<Json<Value>> {
    same_org(&access, id)?;
    access.require_admin()?;
    let plan = access.plan;
    if !plan.limits.org_templates {
        return Err(AppError::QuotaExceeded(
            "Le déploiement en masse est inclus dans le plan Team. Passez à Team pour générer \
             la signature de chaque membre d'un seul geste."
                .into(),
        ));
    }

    let (name, doc_json): (String, Value) = sqlx::query_as(
        "SELECT name, doc FROM signatures \
         WHERE id = $1 AND org_id = $2 AND kind = 'org_template' AND deleted_at IS NULL",
    )
    .bind(req.template_id)
    .bind(id)
    .fetch_optional(&st.db)
    .await?
    .ok_or_else(|| {
        AppError::validation("Ce modèle d'organisation n'existe pas ou a été supprimé.")
    })?;

    let doc: Doc = serde_json::from_value(doc_json)?;
    // le modèle part en rendu pour chaque membre : on revalide une fois, pas N fois
    doc.validate()?;
    let doc_value = serde_json::to_value(&doc)?;

    let members: Vec<RolloutMember> = sqlx::query_as(
        "SELECT m.user_id, m.profile, u.name, u.email::text AS email \
         FROM org_members m JOIN users u ON u.id = m.user_id \
         WHERE m.org_id = $1 ORDER BY m.created_at",
    )
    .bind(id)
    .fetch_all(&st.db)
    .await?;

    // ponytail : une poignée de requêtes par membre. Une agence de 40 personnes tient
    // largement dans le budget d'une requête HTTP ; à 500 membres il faudra un job de fond.
    let mut out = Vec::with_capacity(members.len());
    for m in members {
        let profile = member_profile(&m);
        let profile_value = serde_json::to_value(&profile)?;

        let existing: Option<(Uuid, Option<String>)> = sqlx::query_as(
            "SELECT id, public_slug::text FROM signatures \
             WHERE org_id = $1 AND owner_user_id = $2 AND name = $3 AND kind = 'personal' \
               AND deleted_at IS NULL \
             ORDER BY created_at LIMIT 1",
        )
        .bind(id)
        .bind(m.user_id)
        .bind(&name)
        .fetch_optional(&st.db)
        .await?;

        let (sig_id, slug) = match existing {
            Some((sig_id, slug)) => {
                sqlx::query(
                    "UPDATE signatures SET doc = $1, profile = $2, updated_at = now() WHERE id = $3",
                )
                .bind(&doc_value)
                .bind(&profile_value)
                .bind(sig_id)
                .execute(&st.db)
                .await?;
                (sig_id, slug)
            }
            None => {
                let sig_id = Uuid::new_v4();
                sqlx::query(
                    "INSERT INTO signatures (id, org_id, owner_user_id, name, kind, doc, profile) \
                     VALUES ($1, $2, $3, $4, 'personal', $5, $6)",
                )
                .bind(sig_id)
                .bind(id)
                .bind(m.user_id)
                .bind(&name)
                .bind(&doc_value)
                .bind(&profile_value)
                .execute(&st.db)
                .await?;
                (sig_id, None)
            }
        };

        let p = signatures::publish_doc(&st.db, sig_id, slug, &doc, &profile).await?;
        out.push(RolloutItem {
            user_id: m.user_id,
            signature_id: sig_id,
            slug: p.slug,
            job_id: p.job_id,
        });
    }

    Ok(Json(
        json!({ "template": name, "count": out.len(), "results": out }),
    ))
}

/// Le profil du membre complété par ce qu'on sait déjà de lui : un `{{email}}` vide dans la
/// signature d'un collaborateur qui n'a pas rempli son profil serait un bug visible par ses
/// clients. Les valeurs saisies par le membre gagnent toujours.
fn member_profile(m: &RolloutMember) -> Profile {
    let mut p: Profile = serde_json::from_value(m.profile.clone()).unwrap_or_default();
    p.retain(|_, v| !v.trim().is_empty());
    p.entry("email".to_string())
        .or_insert_with(|| m.email.clone());
    if let Some(name) = m.name.as_deref().filter(|n| !n.trim().is_empty()) {
        p.entry("name".to_string())
            .or_insert_with(|| name.to_string());
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Le garde qui empêche une organisation de devenir inadministrable.
    #[test]
    fn an_organisation_never_loses_its_last_owner() {
        // dernier propriétaire : ni rétrogradation, ni retrait
        assert!(!keeps_an_owner("owner", Some("admin"), 1));
        assert!(!keeps_an_owner("owner", Some("member"), 1));
        assert!(!keeps_an_owner("owner", None, 1));
        // un autre propriétaire existe : les deux redeviennent possibles
        assert!(keeps_an_owner("owner", Some("member"), 2));
        assert!(keeps_an_owner("owner", None, 2));
        // owner → owner n'enlève rien, même seul
        assert!(keeps_an_owner("owner", Some("owner"), 1));
        // qui n'est pas propriétaire ne peut pas retirer le dernier
        assert!(keeps_an_owner("admin", None, 1));
        assert!(keeps_an_owner("member", Some("admin"), 1));
        // compteur incohérent (0 propriétaire) : on refuse plutôt que d'aggraver
        assert!(!keeps_an_owner("owner", None, 0));
    }

    /// Le jeton d'invitation est devinable ; l'aperçu doit donc rester servable à un inconnu.
    #[test]
    fn the_invite_preview_reveals_the_org_and_the_role_and_nothing_else() {
        let body = serde_json::to_value(InvitePreview {
            org: "Atelier Nord".into(),
            role: "member".into(),
        })
        .unwrap();

        let keys: Vec<&str> = body
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            keys,
            ["org", "role"],
            "aucun champ ne s'ajoute sans y avoir pensé"
        );

        let raw = body.to_string();
        for leak in ["@", "email", "invited_by", "members", "token", "expires"] {
            assert!(
                !raw.contains(leak),
                "l'aperçu ne doit pas divulguer « {leak} » : {raw}"
            );
        }
    }

    #[test]
    fn emails_are_checked_before_reaching_the_provider() {
        assert_eq!(
            clean_email("  Ada@Example.COM ").unwrap(),
            "ada@example.com"
        );
        for bad in [
            "",
            "ada",
            "ada@",
            "@example.com",
            "ada@example",
            "a b@c.dev",
            "ada@.com",
        ] {
            assert!(clean_email(bad).is_err(), "{bad} aurait dû être refusé");
        }
    }

    #[test]
    fn member_profile_fills_the_gaps_without_overwriting() {
        let m = RolloutMember {
            user_id: Uuid::nil(),
            profile: json!({ "name": "Ada L.", "role": "  ", "company": "Analytical" }),
            name: Some("Ada Lovelace".into()),
            email: "ada@example.com".into(),
        };
        let p = member_profile(&m);
        assert_eq!(p.get("name").map(String::as_str), Some("Ada L."));
        assert_eq!(p.get("email").map(String::as_str), Some("ada@example.com"));
        assert_eq!(p.get("company").map(String::as_str), Some("Analytical"));
        assert!(
            !p.contains_key("role"),
            "une valeur vide ne doit pas masquer un jeton"
        );
    }
}
