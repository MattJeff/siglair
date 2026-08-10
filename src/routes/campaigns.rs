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
use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    auth::OrgAccess,
    billing::lifecycle::{effective_plan_at, OrgBilling},
    doc::{Anim, AnimPreset, Doc, Element, ElementType, Profile},
    error::{AppError, Result},
    plans::{CampaignScope, Plan},
    routes::signatures,
    util::doc_hash,
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

const COLS: &str = "id, org_id, owner_user_id, name, message, cta, href, color, \
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
    pub owner_user_id: Option<Uuid>,
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

async fn load_all(db: &PgPool, access: &OrgAccess) -> Result<Vec<Campaign>> {
    Ok(sqlx::query_as::<_, Campaign>(&format!(
        "SELECT {COLS} FROM campaigns WHERE org_id = $1 \
         AND ($2::bool OR owner_user_id = $3) {ORDER}"
    ))
    .bind(access.org_id)
    .bind(access.plan.limits.campaigns == CampaignScope::Team)
    .bind(access.user_id)
    .fetch_all(db)
    .await?)
}

async fn fetch(db: &PgPool, access: &OrgAccess, id: Uuid) -> Result<Campaign> {
    sqlx::query_as::<_, Campaign>(&format!(
        "SELECT {COLS} FROM campaigns WHERE id = $1 AND org_id = $2 \
         AND ($3::bool OR owner_user_id = $4)"
    ))
    .bind(id)
    .bind(access.org_id)
    .bind(access.plan.limits.campaigns == CampaignScope::Team)
    .bind(access.user_id)
    .fetch_optional(db)
    .await?
    .ok_or(AppError::NotFound)
}

// ------------------------------------------------------------------ règles

/// Contrat §6 : les campagnes datées commencent au plan Pro. Même verrou que le GIF hébergé,
/// et ce n'est pas un hasard — une bannière programmée n'a d'intérêt que sur une signature
/// hébergée, celle qui se met à jour sans que personne ne recolle son HTML.
fn require_paid_plan(plan: &Plan) -> Result<()> {
    // `hosted_gif` ne verrouille plus rien : Free l'a désormais à `true` (§6), ce qui
    // ouvrait la création de campagnes à Free par l'API alors que sa portée est `None`.
    // C'est la PORTÉE qui décide, et elle seule.
    if plan.limits.campaigns != CampaignScope::None {
        return Ok(());
    }
    Err(AppError::QuotaExceeded(format!(
        "Les campagnes datées sont incluses à partir du plan Pro ({prix}/mois) : programmez \
         une bannière une fois, posez-la sur vos signatures et republiez — l'URL hébergée \
         se met à jour sans que personne ne recolle son HTML.",
        prix = crate::plans::PRO.price_label(),
    )))
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

/// Document effectif d'une campagne. Le document enregistré reste intact : quand la fenêtre
/// se ferme, le scheduler republie simplement cette source sans bannière injectée.
pub(crate) fn apply_to_doc(doc: &mut Doc, campaign: &Campaign) {
    let content = if campaign.cta.trim().is_empty() {
        campaign.message.clone()
    } else {
        format!("{}  {} →", campaign.message, campaign.cta)
    };
    if let Some(banner) = doc
        .elements
        .iter_mut()
        .find(|element| element.kind == ElementType::Banner)
    {
        banner.content = content;
        banner.href = campaign.href.clone();
        banner.background = campaign.color.clone();
        return;
    }

    let primary: String = campaign.id.simple().to_string().chars().take(7).collect();
    let id = std::iter::once(primary)
        .chain((1..=99).map(|n| format!("cmpgn{n:02}")))
        .find(|id| !doc.elements.iter().any(|element| element.id == *id))
        .unwrap_or_else(|| "cmpgn99".into());
    doc.elements.push(Element {
        id,
        kind: ElementType::Banner,
        x: 25.0,
        y: (doc.canvas.height - 72.0).max(10.0),
        w: (doc.canvas.width - 50.0).max(120.0),
        h: 50.0,
        content,
        href: campaign.href.clone(),
        color: "#ffffff".into(),
        background: campaign.color.clone(),
        radius: 12.0,
        align: "center".into(),
        anim: Anim {
            preset: AnimPreset::Shimmer,
            duration: 3.0,
            delay: 0.3,
            intensity: 0.7,
            ..Default::default()
        },
        ..Default::default()
    });
}

/// Fige le document publié sur lequel la campagne vient se superposer. Les sauvegardes
/// automatiques de l'éditeur peuvent ensuite continuer sans partir en production.
pub(crate) async fn remember_base(
    db: &PgPool,
    signature_id: Uuid,
    doc: &Doc,
    profile: &Profile,
    overwrite: bool,
) -> Result<()> {
    sqlx::query(
        "UPDATE signatures SET \
           campaign_base_doc = CASE WHEN $4 OR campaign_base_doc IS NULL THEN $2 ELSE campaign_base_doc END, \
           campaign_base_profile = CASE WHEN $4 OR campaign_base_profile IS NULL THEN $3 ELSE campaign_base_profile END \
         WHERE id = $1",
    )
    .bind(signature_id)
    .bind(serde_json::to_value(doc)?)
    .bind(serde_json::to_value(profile)?)
    .bind(overwrite)
    .execute(db)
    .await?;
    Ok(())
}

/// Campagne à inclure lors d'une publication manuelle.
pub(crate) async fn active_for_signature(
    db: &PgPool,
    access: &OrgAccess,
    owner_user_id: Option<Uuid>,
    now: DateTime<Utc>,
) -> Result<Option<Campaign>> {
    if access.plan.limits.campaigns == CampaignScope::None {
        return Ok(None);
    }
    let rows = load_all(db, access).await?;
    Ok(pick_active(
        &rows
            .into_iter()
            .filter(|campaign| {
                access.plan.limits.campaigns == CampaignScope::Team
                    || campaign.owner_user_id == owner_user_id
            })
            .collect::<Vec<_>>(),
        now,
    )
    .cloned())
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
    Ok(Json(load_all(&st.db, &access).await?))
}

async fn get_one(
    State(st): State<AppState>,
    access: OrgAccess,
    Path(id): Path<Uuid>,
) -> Result<Json<Campaign>> {
    Ok(Json(fetch(&st.db, &access, id).await?))
}

/// `GET /api/campaigns/active` — la campagne active maintenant, ou `null`. Une seule.
///
/// ponytail : une organisation a une poignée de campagnes, on les charge et on tranche en
/// Rust. Le jour où il y en a des milliers, la même clause devient un `WHERE starts_at <=
/// now() AND ends_at > now() {ORDER} LIMIT 1`.
async fn active(State(st): State<AppState>, access: OrgAccess) -> Result<Json<Option<Campaign>>> {
    let rows = load_all(&st.db, &access).await?;
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
        owner_user_id: Some(access.user_id),
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
        "INSERT INTO campaigns (id, org_id, owner_user_id, name, message, cta, href, color, starts_at, ends_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) RETURNING {COLS}"
    ))
    .bind(c.id)
    .bind(c.org_id)
    .bind(c.owner_user_id)
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
    let mut c = fetch(&st.db, &access, id).await?;
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
    fetch(&st.db, &access, id).await?;
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

// ------------------------------------------------------------------ automatisation

#[derive(sqlx::FromRow)]
struct ScheduledSignature {
    id: Uuid,
    org_id: Uuid,
    owner_user_id: Option<Uuid>,
    doc: Value,
    profile: Value,
    public_slug: String,
    published_campaign_id: Option<Uuid>,
    campaign_base_doc: Option<Value>,
    campaign_base_profile: Option<Value>,
    published_doc: Option<Value>,
    published_profile: Option<Value>,
    published_hash: Option<Vec<u8>>,
    plan: String,
    subscription_status: Option<String>,
    grace_until: Option<DateTime<Utc>>,
}

/// Applique les ouvertures/fermetures de fenêtres aux signatures déjà publiées.
///
/// La comparaison `(campaign_id, doc_hash)` rend la passe idempotente : elle peut tourner
/// toutes les 30 secondes, redémarrer ou s'exécuter sur deux API sans republier inutilement.
pub async fn sweep(state: &AppState) -> anyhow::Result<u64> {
    let now = Utc::now();
    let rows: Vec<ScheduledSignature> = sqlx::query_as(
        "SELECT s.id, s.org_id, s.owner_user_id, s.doc, s.profile, \
                s.public_slug::text AS public_slug, s.published_campaign_id, \
                s.campaign_base_doc, s.campaign_base_profile, r.doc AS published_doc, \
                r.profile AS published_profile, r.doc_hash AS published_hash, \
                o.plan, o.subscription_status, o.grace_until \
         FROM signatures s \
         JOIN orgs o ON o.id = s.org_id \
         LEFT JOIN renders r ON r.id = s.published_render_id \
         WHERE s.deleted_at IS NULL AND s.kind = 'personal' AND s.public_slug IS NOT NULL \
           AND (o.plan IN ('pro', 'team') OR s.published_campaign_id IS NOT NULL)",
    )
    .fetch_all(&state.db)
    .await?;

    let campaigns: Vec<Campaign> = sqlx::query_as(&format!(
        "SELECT {COLS} FROM campaigns WHERE starts_at <= $1 AND ends_at > $1 {ORDER}"
    ))
    .bind(now)
    .fetch_all(&state.db)
    .await?;
    let mut by_org: HashMap<Uuid, Vec<Campaign>> = HashMap::new();
    for campaign in campaigns {
        by_org.entry(campaign.org_id).or_default().push(campaign);
    }

    let mut queued = 0;
    for row in rows {
        let plan = effective_plan_at(
            &OrgBilling {
                plan: row.plan,
                subscription_status: row.subscription_status,
                grace_until: row.grace_until,
            },
            now,
        );
        let candidates = by_org.get(&row.org_id).map(Vec::as_slice).unwrap_or(&[]);
        let active: Option<Campaign> = match plan.limits.campaigns {
            CampaignScope::None => None,
            CampaignScope::Team => pick_active(candidates, now).cloned(),
            CampaignScope::Own => pick_active(
                &candidates
                    .iter()
                    .filter(|campaign| campaign.owner_user_id == row.owner_user_id)
                    .cloned()
                    .collect::<Vec<_>>(),
                now,
            )
            .cloned(),
        };

        // Pas de campagne et aucun état automatisé à retirer : ne jamais publier un brouillon
        // simplement parce que le scheduler l'a remarqué.
        if active.is_none()
            && row.published_campaign_id.is_none()
            && row.campaign_base_doc.is_none()
        {
            continue;
        }

        let (base_doc, base_profile) = if active.is_some() {
            match (row.campaign_base_doc, row.campaign_base_profile) {
                (Some(doc), Some(profile)) => (doc, profile),
                _ => (
                    row.published_doc.unwrap_or(row.doc),
                    row.published_profile.unwrap_or(row.profile),
                ),
            }
        } else {
            (
                row.campaign_base_doc.unwrap_or(row.doc),
                row.campaign_base_profile.unwrap_or(row.profile),
            )
        };
        let mut doc: Doc = match serde_json::from_value(base_doc) {
            Ok(doc) => doc,
            Err(error) => {
                tracing::error!(signature = %row.id, ?error, "campagne ignorée : document illisible");
                continue;
            }
        };
        let profile: Profile = serde_json::from_value(base_profile).unwrap_or_default();
        if let Some(campaign) = active.as_ref() {
            if let Err(error) = remember_base(&state.db, row.id, &doc, &profile, false).await {
                tracing::error!(signature = %row.id, ?error, "base de campagne non mémorisée");
                continue;
            }
            apply_to_doc(&mut doc, campaign);
        }
        if let Err(error) = doc.validate() {
            tracing::error!(signature = %row.id, ?error, "campagne ignorée : document invalide");
            continue;
        }
        let desired_campaign = active.as_ref().map(|campaign| campaign.id);
        let desired_hash = doc_hash(&doc, &profile);
        if row.published_campaign_id == desired_campaign
            && row.published_hash.as_deref() == Some(desired_hash.as_slice())
        {
            continue;
        }

        if let Err(error) = signatures::publish_snapshot(
            &state.db,
            row.id,
            Some(row.public_slug),
            &doc,
            &profile,
            desired_campaign,
        )
        .await
        {
            tracing::error!(signature = %row.id, ?error, "campagne non mise en file");
            continue;
        }
        queued += 1;
    }
    Ok(queued)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    /// Le verrou porte sur la PORTÉE, pas sur `hosted_gif` : Free l'a désormais à `true`,
    /// et le verrou d'avant laissait donc Free créer des campagnes par l'API alors que
    /// l'interface les lui présente comme éteintes (§6 : un bouton grisé n'est pas un quota).
    #[test]
    fn free_ne_peut_pas_creer_de_campagne() {
        use crate::plans::{FREE, PRO, TEAM};
        assert!(require_paid_plan(&FREE).is_err());
        assert!(require_paid_plan(&PRO).is_ok());
        assert!(require_paid_plan(&TEAM).is_ok());
    }

    fn at(minutes: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(1_700_000_000, 0).unwrap() + Duration::minutes(minutes)
    }

    fn campaign(name: &str, start: i64, end: i64) -> Campaign {
        Campaign {
            id: Uuid::new_v4(),
            org_id: Uuid::nil(),
            owner_user_id: Some(Uuid::nil()),
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

    #[test]
    fn campaign_overlay_does_not_mutate_the_saved_document() {
        let base = Doc::default();
        let mut effective = base.clone();
        let mut c = campaign("Lancement", 0, 100);
        c.message = "Nouveau guide".into();
        c.cta = "Télécharger".into();
        c.href = "https://siglair.com/guide".into();

        apply_to_doc(&mut effective, &c);

        assert!(base.elements.is_empty());
        assert_eq!(effective.elements.len(), 1);
        assert_eq!(effective.elements[0].kind, ElementType::Banner);
        assert!(effective.elements[0].content.contains("Télécharger"));
        assert_eq!(effective.elements[0].href, c.href);
        assert!(effective.validate().is_ok());
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
        assert!(check_banner("Promo", "#2563eb", "https://siglair.com").is_ok());
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
        c.href = "  https://siglair.com  ".into();
        assert!(clean(&mut c).is_ok());
        assert_eq!(c.name, "Soldes");
        assert_eq!(c.href, "https://siglair.com");

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
