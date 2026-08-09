//! Campagnes datées — contrat §6, plans Pro et Team.
//!
//! Une campagne est une bannière programmée au niveau de l'ORGANISATION : elle vaut pour
//! toutes ses signatures, c'est ce qui la rend utile à une équipe. Tout est filtré par
//! `OrgAccess.org_id` ; une campagne d'une autre organisation répond 404, jamais 403.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    auth::OrgAccess,
    doc::{Doc, Element, ElementType},
    error::{AppError, Result},
    plans::Plan,
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/campaigns", get(list).post(create))
        // `/campaigns/active` avant `/campaigns/{id}` : le segment statique est prioritaire,
        // sinon « active » serait lu comme un uuid illisible.
        .route("/campaigns/active", get(active))
        .route("/campaigns/{id}", get(get_one).patch(update).delete(remove))
}

const COLS: &str = "id, org_id, name, message, cta, href, color, \
                    starts_at, ends_at, created_at, updated_at";

/// La plus récemment démarrée d'abord. `created_at, id` départagent deux campagnes qui
/// démarrent à la même seconde : sans eux, la bannière pourrait changer d'une requête à
/// l'autre pour le même instant.
const ORDER: &str = "ORDER BY starts_at DESC, created_at DESC, id";

const DEFAULT_COLOR: &str = "#2563eb";

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Campaign {
    pub id: Uuid,
    pub org_id: Uuid,
    pub name: String,
    pub message: String,
    pub cta: String,
    pub href: String,
    pub color: String,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

async fn load_all(db: &PgPool, org_id: Uuid) -> Result<Vec<Campaign>> {
    Ok(sqlx::query_as::<_, Campaign>(&format!(
        "SELECT {COLS} FROM campaigns WHERE org_id = $1 {ORDER}"
    ))
    .bind(org_id)
    .fetch_all(db)
    .await?)
}

async fn fetch(db: &PgPool, org_id: Uuid, id: Uuid) -> Result<Campaign> {
    sqlx::query_as::<_, Campaign>(&format!(
        "SELECT {COLS} FROM campaigns WHERE id = $1 AND org_id = $2"
    ))
    .bind(id)
    .bind(org_id)
    .fetch_optional(db)
    .await?
    .ok_or(AppError::NotFound)
}

// ------------------------------------------------------------------ règles

/// Contrat §6 : les campagnes datées commencent au plan Pro. Même verrou que le GIF hébergé,
/// et ce n'est pas un hasard — une bannière programmée n'a d'intérêt que sur une signature
/// hébergée, celle qui se met à jour sans que personne ne recolle son HTML.
fn require_paid_plan(plan: &Plan) -> Result<()> {
    if plan.limits.hosted_gif {
        return Ok(());
    }
    Err(AppError::QuotaExceeded(
        "Les campagnes datées sont incluses à partir du plan Pro (7,90 €/mois) : programmez \
         une bannière une fois, elle apparaît et disparaît toute seule dans les signatures \
         déjà collées dans les emails de votre équipe."
            .into(),
    ))
}

/// `starts_at <= now < ends_at`. Si plusieurs se chevauchent, la plus récemment démarrée
/// gagne — c'est-à-dire `ORDER BY starts_at DESC LIMIT 1` sur la fenêtre courante, évalué
/// ici pour rester testable sans base et indépendant de l'ordre des lignes reçues.
/// La clé de tri complète (démarrage, création, id) rend le choix strictement déterministe :
/// deux requêtes au même instant renvoient toujours la même bannière.
fn pick_active(rows: &[Campaign], now: DateTime<Utc>) -> Option<&Campaign> {
    rows.iter()
        .filter(|c| c.starts_at <= now && now < c.ends_at)
        .max_by_key(|c| (c.starts_at, c.created_at, c.id))
}

fn text(label: &str, raw: &str, min: usize, max: usize) -> Result<String> {
    let t = raw.trim().to_string();
    if !(min..=max).contains(&t.chars().count()) {
        return Err(AppError::validation(format!(
            "{label} doit faire entre {min} et {max} caractères."
        )));
    }
    Ok(t)
}

/// Le message, la couleur et le lien d'une campagne finissent tels quels dans un élément
/// `banner` du document, donc dans du HTML envoyé par email. On les valide avec le
/// validateur du document (`Doc::validate`, contrat §8) plutôt qu'avec une seconde copie
/// des mêmes règles : couleur `#rrggbb`, schémas `http https mailto tel`, longueurs.
fn check_banner(message: &str, color: &str, href: &str) -> Result<()> {
    Doc {
        elements: vec![Element {
            id: "banner0".into(),
            kind: ElementType::Banner,
            content: message.into(),
            color: color.into(),
            background: color.into(),
            href: href.into(),
            ..Default::default()
        }],
        ..Default::default()
    }
    .validate()
}

/// Toutes les bornes en un seul endroit : `create` et `update` écrivent la même chose.
fn clean(c: &mut Campaign) -> Result<()> {
    c.name = text("Le nom de la campagne", &c.name, 1, 120)?;
    c.message = text("Le message de la bannière", &c.message, 1, 300)?;
    c.cta = text("Le libellé du bouton", &c.cta, 0, 60)?;
    c.href = c.href.trim().to_string();
    if c.ends_at <= c.starts_at {
        return Err(AppError::validation(
            "La fin de la campagne doit être postérieure à son début.",
        ));
    }
    check_banner(&c.message, &c.color, &c.href)
}

// ------------------------------------------------------------------ lecture

async fn list(State(st): State<AppState>, access: OrgAccess) -> Result<Json<Vec<Campaign>>> {
    Ok(Json(load_all(&st.db, access.org_id).await?))
}

async fn get_one(
    State(st): State<AppState>,
    access: OrgAccess,
    Path(id): Path<Uuid>,
) -> Result<Json<Campaign>> {
    Ok(Json(fetch(&st.db, access.org_id, id).await?))
}

/// `GET /api/campaigns/active` — la campagne active maintenant, ou `null`. Une seule.
///
/// ponytail : une organisation a une poignée de campagnes, on les charge et on tranche en
/// Rust. Le jour où il y en a des milliers, la même clause devient un `WHERE starts_at <=
/// now() AND ends_at > now() {ORDER} LIMIT 1`.
async fn active(State(st): State<AppState>, access: OrgAccess) -> Result<Json<Option<Campaign>>> {
    let rows = load_all(&st.db, access.org_id).await?;
    Ok(Json(pick_active(&rows, Utc::now()).cloned()))
}

// ------------------------------------------------------------------ écriture

#[derive(Deserialize)]
struct CreateReq {
    name: String,
    message: String,
    cta: Option<String>,
    href: Option<String>,
    color: Option<String>,
    starts_at: DateTime<Utc>,
    ends_at: DateTime<Utc>,
}

async fn create(
    State(st): State<AppState>,
    access: OrgAccess,
    Json(req): Json<CreateReq>,
) -> Result<(StatusCode, Json<Campaign>)> {
    require_paid_plan(&access.plan)?;
    access.require_admin()?;

    let now = Utc::now();
    let mut c = Campaign {
        id: Uuid::new_v4(),
        org_id: access.org_id,
        name: req.name,
        message: req.message,
        cta: req.cta.unwrap_or_default(),
        href: req.href.unwrap_or_default(),
        color: req.color.unwrap_or_else(|| DEFAULT_COLOR.into()),
        starts_at: req.starts_at,
        ends_at: req.ends_at,
        created_at: now,
        updated_at: now,
    };
    clean(&mut c)?;

    let row = sqlx::query_as::<_, Campaign>(&format!(
        "INSERT INTO campaigns (id, org_id, name, message, cta, href, color, starts_at, ends_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) RETURNING {COLS}"
    ))
    .bind(c.id)
    .bind(c.org_id)
    .bind(&c.name)
    .bind(&c.message)
    .bind(&c.cta)
    .bind(&c.href)
    .bind(&c.color)
    .bind(c.starts_at)
    .bind(c.ends_at)
    .fetch_one(&st.db)
    .await?;
    Ok((StatusCode::CREATED, Json(row)))
}

#[derive(Deserialize)]
struct UpdateReq {
    name: Option<String>,
    message: Option<String>,
    cta: Option<String>,
    href: Option<String>,
    color: Option<String>,
    starts_at: Option<DateTime<Utc>>,
    ends_at: Option<DateTime<Utc>>,
}

async fn update(
    State(st): State<AppState>,
    access: OrgAccess,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateReq>,
) -> Result<Json<Campaign>> {
    require_paid_plan(&access.plan)?;
    access.require_admin()?;

    // lecture puis écriture complète : les bornes se vérifient sur la campagne entière
    // (`ends_at > starts_at` n'a pas de sens sur un champ isolé).
    let mut c = fetch(&st.db, access.org_id, id).await?;
    if let Some(v) = req.name {
        c.name = v;
    }
    if let Some(v) = req.message {
        c.message = v;
    }
    if let Some(v) = req.cta {
        c.cta = v;
    }
    if let Some(v) = req.href {
        c.href = v;
    }
    if let Some(v) = req.color {
        c.color = v;
    }
    if let Some(v) = req.starts_at {
        c.starts_at = v;
    }
    if let Some(v) = req.ends_at {
        c.ends_at = v;
    }
    clean(&mut c)?;

    let row = sqlx::query_as::<_, Campaign>(&format!(
        "UPDATE campaigns SET name = $3, message = $4, cta = $5, href = $6, color = $7, \
                              starts_at = $8, ends_at = $9, updated_at = now() \
         WHERE id = $1 AND org_id = $2 RETURNING {COLS}"
    ))
    .bind(id)
    .bind(access.org_id)
    .bind(&c.name)
    .bind(&c.message)
    .bind(&c.cta)
    .bind(&c.href)
    .bind(&c.color)
    .bind(c.starts_at)
    .bind(c.ends_at)
    .fetch_optional(&st.db)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(Json(row))
}

/// Pas de verrou de plan sur la suppression : après un retour à Free, l'utilisateur doit
/// pouvoir faire le ménage dans ce qu'il a créé. Le plan garde l'écriture, pas la sortie.
async fn remove(
    State(st): State<AppState>,
    access: OrgAccess,
    Path(id): Path<Uuid>,
) -> Result<StatusCode> {
    access.require_admin()?;
    let n = sqlx::query("DELETE FROM campaigns WHERE id = $1 AND org_id = $2")
        .bind(id)
        .bind(access.org_id)
        .execute(&st.db)
        .await?
        .rows_affected();
    if n == 0 {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn at(minutes: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(1_700_000_000, 0).unwrap() + Duration::minutes(minutes)
    }

    fn campaign(name: &str, start: i64, end: i64) -> Campaign {
        Campaign {
            id: Uuid::new_v4(),
            org_id: Uuid::nil(),
            name: name.into(),
            message: "✨ Une nouveauté".into(),
            cta: String::new(),
            href: String::new(),
            color: DEFAULT_COLOR.into(),
            starts_at: at(start),
            ends_at: at(end),
            created_at: at(0),
            updated_at: at(0),
        }
    }

    /// Deux campagnes qui se chevauchent ne doivent pas donner une bannière différente
    /// selon la requête : la plus récemment démarrée gagne, toujours.
    #[test]
    fn overlapping_campaigns_resolve_to_the_latest_started() {
        let now = at(100);
        let old = campaign("ancienne", 0, 500);
        let recent = campaign("récente", 60, 200);
        let ended = campaign("terminée", -100, 50);
        let future = campaign("à venir", 300, 400);

        let mut rows = vec![old.clone(), recent.clone(), ended, future];
        assert_eq!(pick_active(&rows, now).unwrap().name, "récente");

        // l'ordre d'arrivée des lignes ne change rien
        rows.reverse();
        assert_eq!(pick_active(&rows, now).unwrap().name, "récente");

        // une seule à la fois : la plus ancienne reste en base mais ne sort pas
        assert_ne!(pick_active(&rows, now).unwrap().id, old.id);

        // égalité parfaite sur starts_at et created_at : l'id tranche, et toujours pareil
        let mut a = campaign("a", 60, 200);
        let mut b = campaign("b", 60, 200);
        a.id = Uuid::from_u128(1);
        b.id = Uuid::from_u128(2);
        let ab = vec![a.clone(), b.clone()];
        let ba = vec![b, a];
        assert_eq!(
            pick_active(&ab, now).unwrap().id,
            pick_active(&ba, now).unwrap().id
        );
    }

    #[test]
    fn the_window_is_start_inclusive_and_end_exclusive() {
        let c = vec![campaign("c", 0, 100)];
        assert!(
            pick_active(&c, at(0)).is_some(),
            "starts_at == now : active"
        );
        assert!(pick_active(&c, at(99)).is_some());
        assert!(
            pick_active(&c, at(100)).is_none(),
            "ends_at == now : terminée"
        );
        assert!(pick_active(&c, at(-1)).is_none());
        assert!(pick_active(&[], at(0)).is_none());
    }

    /// Ces valeurs partent dans du HTML envoyé par email : un `javascript:` ou un
    /// `url(...)` en guise de couleur, c'est du XSS stocké distribué à des clients.
    #[test]
    fn banner_values_are_checked_before_they_reach_an_email() {
        assert!(check_banner("Promo", "#2563eb", "https://siglair.app").is_ok());
        assert!(check_banner("Promo", "#FFF000", "mailto:a@b.dev").is_ok());
        assert!(check_banner("Promo", "#000000", "tel:+33600000000").is_ok());
        assert!(
            check_banner("Promo", "#000000", "").is_ok(),
            "une bannière peut ne pas être cliquable"
        );

        for bad in [
            "javascript:alert(1)",
            "JavaScript:alert(1)",
            "data:text/html,x",
            "/relatif",
        ] {
            assert!(
                check_banner("Promo", "#000000", bad).is_err(),
                "{bad} aurait dû être refusé"
            );
        }
        for bad in ["red", "#fff", "#12345g", "url(evil)", "", "#0000001"] {
            assert!(
                check_banner("Promo", bad, "").is_err(),
                "{bad} aurait dû être refusé"
            );
        }
    }

    #[test]
    fn clean_rejects_an_impossible_window_and_trims() {
        let mut c = campaign("  Soldes  ", 100, 200);
        c.href = "  https://siglair.app  ".into();
        assert!(clean(&mut c).is_ok());
        assert_eq!(c.name, "Soldes");
        assert_eq!(c.href, "https://siglair.app");

        let mut backwards = campaign("x", 200, 100);
        assert!(matches!(
            clean(&mut backwards),
            Err(AppError::Validation(_))
        ));

        let mut same = campaign("x", 100, 100);
        assert!(
            clean(&mut same).is_err(),
            "une campagne de durée nulle n'est jamais active"
        );

        let mut unnamed = campaign("   ", 0, 100);
        assert!(clean(&mut unnamed).is_err());
    }

    /// `/campaigns/active` et `/campaigns/{id}` sont voisines : un conflit de route ne se
    /// verrait qu'au démarrage du serveur, en panique.
    #[test]
    fn routes_do_not_conflict() {
        let _ = router();
    }
}
