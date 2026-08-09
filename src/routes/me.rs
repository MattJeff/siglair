//! `/api/me` — l'utilisateur connecté : lecture, organisation active, suppression du compte.
//!
//! Le `GET` reste celui de `crate::auth::me` : une seconde implémentation de la même
//! réponse finirait par diverger. Ce module ajoute les deux verbes qui manquaient au front
//! (`PATCH`, `DELETE`) et regroupe les trois au même endroit — voir le rapport
//! d'intégration : la ligne `/api/me` doit être retirée de `auth::router()`, sinon axum
//! répond 405 sur `PATCH` et `DELETE` (la route littérale gagne sur le `nest("/api")`).

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, patch},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    auth::{self, load_org, session, CurrentUser, OrgAccess},
    doc::Profile,
    error::{AppError, Result},
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/me",
            get(auth::me).patch(switch_org).delete(delete_account),
        )
        .route("/me/profile", patch(update_profile))
}

// ------------------------------------------------------------------ profil personnel

#[derive(Deserialize)]
struct ProfilePatch {
    profile: Profile,
}

#[derive(Serialize)]
struct ProfilePatchResult {
    updated_signatures: u64,
}

/// `PATCH /api/me/profile` met à jour le profil membre et toutes ses signatures dans une
/// seule transaction. L'écran Réglages ne peut donc plus laisser les deux copies divergentes
/// si l'une des requêtes échoue au milieu de la sauvegarde.
async fn update_profile(
    State(st): State<AppState>,
    access: OrgAccess,
    Json(req): Json<ProfilePatch>,
) -> Result<Json<ProfilePatchResult>> {
    let profile = serde_json::to_value(req.profile)?;
    let mut tx = st.db.begin().await?;

    sqlx::query("UPDATE org_members SET profile = $3 WHERE org_id = $1 AND user_id = $2")
        .bind(access.org_id)
        .bind(access.user_id)
        .bind(&profile)
        .execute(&mut *tx)
        .await?;

    let updated_signatures = sqlx::query(
        "UPDATE signatures SET profile = $3, updated_at = now() \
         WHERE org_id = $1 AND owner_user_id = $2 AND deleted_at IS NULL",
    )
    .bind(access.org_id)
    .bind(access.user_id)
    .bind(&profile)
    .execute(&mut *tx)
    .await?
    .rows_affected();

    tx.commit().await?;
    Ok(Json(ProfilePatchResult { updated_signatures }))
}

// ------------------------------------------------------------------ organisation active

#[derive(Deserialize)]
struct SwitchOrg {
    current_org: Uuid,
}

/// `PATCH /api/me` — change l'organisation active.
///
/// Le seul contrôle qui compte est `load_org` : il répond 403 quand l'utilisateur n'est pas
/// membre. Un uuid deviné ne donne donc rien, et il n'y a pas de second garde à écrire.
async fn switch_org(
    State(st): State<AppState>,
    user: CurrentUser,
    Json(req): Json<SwitchOrg>,
) -> Result<Json<Value>> {
    let org = load_org(&st.db, user.id, req.current_org).await?;
    // ponytail : une colonne, pas une table de préférences. Nécessite la migration 0003 et
    // la lecture correspondante dans `auth::middleware::current_org` — les deux sont hors
    // de ce périmètre, lignes exactes dans le rapport d'intégration.
    sqlx::query("UPDATE users SET current_org_id = $2 WHERE id = $1")
        .bind(user.id)
        .bind(org.org_id)
        .execute(&st.db)
        .await?;
    Ok(Json(
        json!({ "current_org": org.org_id, "plan": org.plan.id, "role": org.role }),
    ))
}

// ------------------------------------------------------------------ suppression de compte

/// Ce que le compte entraîne avec lui. Une organisation dont l'utilisateur est le seul
/// membre disparaît ; une organisation partagée survit à son départ.
#[derive(sqlx::FromRow)]
struct Membership {
    org_id: Uuid,
    name: String,
    role: String,
    members: i64,
    owners: i64,
}

/// `DELETE /api/me` — RGPD, contrat §4.1. Purge `users`, `sessions`, `identities`,
/// `signatures`, `assets` et `events`. Irréversible : la double confirmation est côté
/// front, ici il ne reste qu'à le faire et à le tracer.
async fn delete_account(State(st): State<AppState>, user: CurrentUser) -> Result<Response> {
    let memberships: Vec<Membership> = sqlx::query_as(
        "SELECT m.org_id, o.name, m.role, \
                (SELECT count(*) FROM org_members x WHERE x.org_id = m.org_id) AS members, \
                (SELECT count(*) FROM org_members x WHERE x.org_id = m.org_id \
                                                     AND x.role = 'owner') AS owners \
         FROM org_members m JOIN orgs o ON o.id = m.org_id \
         WHERE m.user_id = $1",
    )
    .bind(user.id)
    .fetch_all(&st.db)
    .await?;

    // Même règle que `orgs::keeps_an_owner` : partir en laissant une organisation partagée
    // sans propriétaire la rend inadministrable pour ceux qui restent.
    if let Some(m) = memberships
        .iter()
        .find(|m| blocks_deletion(&m.role, m.members, m.owners))
    {
        return Err(AppError::conflict(format!(
            "Vous êtes le seul propriétaire de « {} », qui compte d'autres membres. Nommez \
             quelqu'un d'autre propriétaire avant de supprimer votre compte.",
            m.name
        )));
    }

    let doomed: Vec<Uuid> = memberships
        .iter()
        .filter(|m| m.members <= 1)
        .map(|m| m.org_id)
        .collect();

    // Les clés se lisent AVANT les suppressions : après, plus rien ne dit quels octets
    // laisser sur le disque, et un rendu oublié est une donnée personnelle résiduelle.
    let mut keys: Vec<String> =
        sqlx::query_scalar("SELECT storage_key FROM assets WHERE org_id = ANY($1)")
            .bind(&doomed)
            .fetch_all(&st.db)
            .await?;
    let renders: Vec<(String, String)> = sqlx::query_as(
        "SELECT r.gif_key, r.png_key FROM renders r JOIN signatures s ON s.id = r.signature_id \
         WHERE s.org_id = ANY($1) OR (s.owner_user_id = $2 AND s.kind = 'personal')",
    )
    .bind(&doomed)
    .bind(user.id)
    .fetch_all(&st.db)
    .await?;
    keys.extend(renders.into_iter().flat_map(|(gif, png)| [gif, png]));

    let mut tx = st.db.begin().await?;
    // Dans une organisation qui survit, `signatures.owner_user_id` passerait à NULL et le
    // `profile` (nom, e-mail, téléphone) resterait en base : on supprime pour de bon.
    // Les `renders`, `render_jobs` et `events` suivent en cascade.
    sqlx::query("DELETE FROM signatures WHERE owner_user_id = $1 AND kind = 'personal'")
        .bind(user.id)
        .execute(&mut *tx)
        .await?;
    // Cascade : org_members, invites, assets, signatures → renders / render_jobs / events.
    sqlx::query("DELETE FROM orgs WHERE id = ANY($1)")
        .bind(&doomed)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM magic_links WHERE email = $1::citext")
        .bind(&user.email)
        .execute(&mut *tx)
        .await?;
    // Cascade : sessions, identities, org_members restants.
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user.id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    // Uniquement des uuid : journaliser l'e-mail au moment de l'effacer viderait l'effacement
    // de son sens (contrat §4.1).
    tracing::warn!(
        user_id = %user.id,
        orgs_deleted = doomed.len(),
        files = keys.len(),
        "compte supprimé sur demande (RGPD) — irréversible"
    );

    // Meilleur effort après le commit : un fichier orphelin ne doit pas faire échouer une
    // suppression déjà validée en base, il se nettoie par ailleurs.
    for key in &keys {
        if let Err(e) = st.storage.delete(key).await {
            tracing::warn!(error = ?e, %key, "fichier non supprimé après effacement du compte");
        }
    }

    let cookie = session::clear_cookie(
        &session::cookie_name(&st.cfg, session::SESSION_COOKIE),
        session::secure(&st.cfg),
    );
    // La session est déjà détruite par la cascade `users` → `sessions` ; le cookie effacé
    // évite juste un aller-retour 401 au prochain appel du navigateur.
    Ok(auth::with_cookies(
        StatusCode::NO_CONTENT.into_response(),
        [cookie],
    ))
}

/// Vrai quand ce départ laisserait une organisation partagée sans propriétaire. Le garde
/// est celui de `orgs::keeps_an_owner`, pas une seconde copie : une organisation dont on
/// est le seul membre part avec le compte, il n'y a personne à protéger.
fn blocks_deletion(role: &str, members: i64, owners: i64) -> bool {
    members > 1 && !crate::routes::orgs::keeps_an_owner(role, None, owners)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_shared_org_left_ownerless_blocks_the_deletion() {
        // seul membre de son org perso : rien à protéger, l'org part avec le compte
        assert!(!blocks_deletion("owner", 1, 1));
        // dernier propriétaire d'une équipe : refus
        assert!(blocks_deletion("owner", 4, 1));
        // un autre propriétaire reste : départ possible
        assert!(!blocks_deletion("owner", 4, 2));
        // admin ou membre : jamais bloquant
        assert!(!blocks_deletion("admin", 4, 1));
        assert!(!blocks_deletion("member", 4, 1));
    }
}
