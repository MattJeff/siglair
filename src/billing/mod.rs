//! Facturation Stripe — contrat §5.3 (routes) et §6 (grille tarifaire).

pub mod lifecycle;
pub mod stripe;
pub mod tax;
pub mod webhook;

use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

// Seul point de couplage avec `src/auth` : l'extracteur de session `CurrentUser`.
use crate::{auth::CurrentUser, config::StripeConfig, error::AppError, plans::Plan, AppState};
use stripe::{price_for, Interval, Stripe};

/// Routes de facturation, session requise. À monter ainsi :
/// `.nest("/api/billing", billing::router())`, derrière le middleware de session.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/checkout", post(checkout))
        .route("/portal", post(portal))
        .route("/subscription", get(subscription))
}

/// Webhook Stripe — chemin absolu, à `.merge()` **hors** du middleware de session :
/// Stripe n'envoie pas de cookie, l'authentification est la signature HMAC.
pub fn webhook_router() -> Router<AppState> {
    Router::new().route("/api/stripe/webhook", post(webhook::handle))
}

/// Sièges minimum du plan Team (contrat §6).
const MIN_SEATS: u32 = 3;

// ---------------------------------------------------------------- handlers

/// Plafond de sièges d'une session de Checkout. Au-delà, c'est un devis, pas un self-service.
const MAX_SEATS: u32 = 1000;

#[derive(Deserialize)]
struct CheckoutReq {
    plan: String,
    /// `"month"` | `"year"`. Absent = mensuel (le front d'avant l'annuel ne l'envoie pas).
    #[serde(default)]
    interval: Option<String>,
    seats: Option<u32>,
}

/// Sièges réellement facturés : jamais moins que le nombre de membres présents, jamais moins
/// que le minimum du contrat §6.
///
/// Le `max(members)` n'est pas une précaution théorique : facturer 3 sièges à une équipe de
/// 7 est une facture fausse, et retirer un siège occupé donne un compte qui plante à la
/// prochaine écriture — très pénible à diagnostiquer, très facile à éviter ici.
fn billed_seats(members: u32, wanted: Option<u32>) -> u32 {
    // Plancher : l'effectif présent, et jamais moins que le minimum du contrat §6.
    // Le plafond cède devant le plancher — mieux vaut une facture juste qu'un plafond tenu.
    let floor = members.max(MIN_SEATS);
    wanted.unwrap_or(0).clamp(floor, MAX_SEATS.max(floor))
}

async fn member_count(state: &AppState, org_id: Uuid) -> Result<u32, AppError> {
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM org_members WHERE org_id = $1")
        .bind(org_id)
        .fetch_one(&state.db)
        .await?;
    Ok(n.clamp(0, MAX_SEATS as i64) as u32)
}

async fn checkout(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(req): Json<CheckoutReq>,
) -> Result<Json<Value>, AppError> {
    // §8 : chaque appel crée un client et une session chez Stripe. Plafonné par utilisateur.
    if !crate::util::rate_limit(
        &format!("checkout:{}", user.id),
        10,
        std::time::Duration::from_secs(3600),
    ) {
        return Err(AppError::RateLimited);
    }
    let cfg = stripe_cfg(&state)?;
    let org = billable_org(&state, user.id).await?;

    if has_live_subscription(&org) {
        return Err(AppError::conflict(
            "Un abonnement est déjà actif sur cette organisation. Utilisez le portail de \
             facturation pour changer de plan ou de nombre de sièges.",
        ));
    }

    let interval = Interval::parse(req.interval.as_deref()).ok_or_else(|| {
        AppError::validation("Périodicité inconnue. Choisissez « month » ou « year ».")
    })?;

    let quantity = match req.plan.as_str() {
        "pro" => 1,
        // Les membres déjà présents sont facturés : `seats` du client ne peut que monter.
        "team" => billed_seats(member_count(&state, org.id).await?, req.seats),
        _ => {
            return Err(AppError::validation(
                "Plan inconnu. Choisissez « pro » ou « team ».",
            ))
        }
    };
    // clé Stripe présente mais STRIPE_PRICE_… absent : même 501, autre cause
    let price = price_for(cfg, &req.plan, interval).ok_or_else(|| {
        AppError::NotImplemented(match interval {
            Interval::Year => format!(
                "La facturation annuelle n'est pas encore disponible pour le plan « {} ». \
                 Choisissez le paiement mensuel.",
                req.plan
            ),
            Interval::Month => {
                format!("Le plan « {} » n'est pas proposé sur ce serveur.", req.plan)
            }
        })
    })?;

    let customer = ensure_customer(&state, cfg, &org).await?;
    let checkout = Stripe::new(&state.http, cfg)
        .create_checkout_session(org.id, &customer, price, quantity, &state.cfg.app_url)
        .await?;
    let url = checkout
        .url
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("checkout session sans url")))?;

    Ok(Json(json!({ "url": url })))
}

async fn portal(State(state): State<AppState>, user: CurrentUser) -> Result<Json<Value>, AppError> {
    let cfg = stripe_cfg(&state)?;
    let org = billable_org(&state, user.id).await?;
    let Some(customer) = org.stripe_customer_id.as_deref() else {
        return Err(AppError::conflict(
            "Aucun abonnement à gérer pour le moment. Choisissez d'abord un plan.",
        ));
    };

    let s = Stripe::new(&state.http, cfg)
        .create_portal_session(customer, &format!("{}/app/billing", state.cfg.app_url))
        .await?;
    Ok(Json(json!({ "url": s.url })))
}

async fn subscription(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<Value>, AppError> {
    stripe_cfg(&state)?;
    let org = billable_org(&state, user.id).await?;
    // Le plan affiché est celui qui donne accès, pas la colonne : sinon l'écran annonce
    // « Pro » à une org que le reste de l'API traite déjà comme Free.
    let plan = lifecycle::effective_plan(&lifecycle::OrgBilling {
        plan: org.plan.clone(),
        subscription_status: org.subscription_status.clone(),
        grace_until: org.grace_until,
    });
    Ok(Json(json!({
        "plan": plan,
        "seats": org.seats,
        "status": org.subscription_status,
        "current_period_end": org.current_period_end,
        // Sans ça l'écran Facturation ne peut pas dire combien de jours il reste avant
        // la bascule, et invente un décompte ou n'en montre aucun.
        "grace_until": org.grace_until,
        "has_subscription": org.stripe_subscription_id.is_some(),
        "min_seats": plan.min_seats,
    })))
}

// ---------------------------------------------------------------- org courante

#[derive(sqlx::FromRow)]
struct BillingOrg {
    id: Uuid,
    name: String,
    plan: String,
    seats: i32,
    stripe_customer_id: Option<String>,
    stripe_subscription_id: Option<String>,
    subscription_status: Option<String>,
    current_period_end: Option<DateTime<Utc>>,
    grace_until: Option<DateTime<Utc>>,
    owner_email: String,
}

/// L'org facturable de l'utilisateur : celle dont il est **propriétaire**. Un membre ou un
/// admin n'engage pas de dépense au nom de l'organisation.
async fn billable_org(state: &AppState, user_id: Uuid) -> Result<BillingOrg, AppError> {
    sqlx::query_as::<_, BillingOrg>(
        "SELECT o.id, o.name, o.plan, o.seats, o.stripe_customer_id, o.stripe_subscription_id, \
                o.subscription_status, o.current_period_end, o.grace_until, \
                u.email AS owner_email \
         FROM orgs o \
         JOIN org_members m ON m.org_id = o.id \
         JOIN users u ON u.id = m.user_id \
         WHERE m.user_id = $1 AND m.role = 'owner' \
         ORDER BY o.created_at LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::Forbidden)
}

fn has_live_subscription(org: &BillingOrg) -> bool {
    org.stripe_subscription_id.is_some()
        && matches!(
            org.subscription_status.as_deref(),
            Some("active" | "trialing" | "past_due")
        )
}

async fn ensure_customer(
    state: &AppState,
    cfg: &StripeConfig,
    org: &BillingOrg,
) -> Result<String, AppError> {
    if let Some(id) = &org.stripe_customer_id {
        return Ok(id.clone());
    }
    let customer = Stripe::new(&state.http, cfg)
        .create_customer(org.id, &org.name, &org.owner_email)
        .await?;
    sqlx::query("UPDATE orgs SET stripe_customer_id = $1 WHERE id = $2")
        .bind(&customer.id)
        .bind(org.id)
        .execute(&state.db)
        .await?;
    Ok(customer.id)
}

// ---------------------------------------------------------------- sièges

/// Aligne la quantité facturée chez Stripe sur l'effectif réel de l'organisation.
/// Renvoie le nombre de sièges facturés.
///
/// **À appeler après chaque ajout et chaque retrait de membre** — sinon la facture dérive
/// dans un sens ou dans l'autre, et l'écart ne se voit qu'à la relance annuelle du client.
///
/// Idempotente : la quantité visée est relue chez Stripe et l'appel est sauté si elle n'a pas
/// bougé, donc deux appels pour le même effectif ne créent pas deux prorations. La clé
/// d'idempotence de [`stripe::Stripe::update_subscription_quantity`] couvre en plus le cas de
/// deux appels concurrents qui liraient tous les deux l'ancienne quantité.
///
/// Ne descend jamais sous l'effectif présent (cf. [`billed_seats`]).
///
/// Jamais bloquante pour l'appelant : un membre s'ajoute même si Stripe est indisponible.
/// L'appelant journalise l'erreur et continue — c'est le webhook
/// `customer.subscription.updated` qui rattrape la vérité au prochain événement.
pub async fn sync_seats(state: &AppState, org_id: Uuid) -> Result<u32, AppError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        plan: String,
        seats: i32,
        stripe_subscription_id: Option<String>,
    }

    let org: Row =
        sqlx::query_as("SELECT plan, seats, stripe_subscription_id FROM orgs WHERE id = $1")
            .bind(org_id)
            .fetch_optional(&state.db)
            .await?
            .ok_or(AppError::NotFound)?;

    // Free et Pro sont facturés à l'organisation : ajouter un membre ne change aucun montant.
    //
    // Seul `Plan::get` du dépôt qui ne passe PAS par `effective_plan`, et c'est voulu : on
    // décide ici de ce que Stripe FACTURE, pas de ce à quoi l'org a droit. Un client en
    // grâce doit continuer d'avoir ses sièges alignés sur son effectif — c'est son
    // abonnement Team qui est impayé, pas son abonnement qui a disparu. Après la bascule,
    // la colonne vaut déjà `free` et cette garde sort d'elle-même.
    if !Plan::get(&org.plan).per_seat {
        return Ok(1);
    }

    let target = billed_seats(member_count(state, org_id).await?, None);

    // Pas d'abonnement (essai, plan posé à la main, résiliation en cours) ou facturation
    // désactivée sur ce serveur : on tient la colonne à jour, il n'y a rien à facturer.
    let (Some(cfg), Some(sub_id)) = (
        state.cfg.stripe.as_ref(),
        org.stripe_subscription_id.as_deref(),
    ) else {
        return update_seats_column(state, org_id, org.seats, target).await;
    };

    let client = Stripe::new(&state.http, cfg);
    let sub = client.get_subscription(sub_id).await?;
    let Some(item) = sub.first_item() else {
        // Abonnement sans ligne : anomalie côté Stripe, on ne fabrique pas de ligne.
        tracing::warn!(%org_id, sub_id, "abonnement Stripe sans ligne de facturation");
        return update_seats_column(state, org_id, org.seats, target).await;
    };

    if item.quantity == Some(i64::from(target)) {
        // Déjà à jour : surtout ne pas POSTer, chaque écriture crée une proration.
        return update_seats_column(state, org_id, org.seats, target).await;
    }

    client
        .update_subscription_quantity(sub_id, &item.id, target)
        .await?;
    // Le webhook `customer.subscription.updated` réécrira la même valeur ; on l'écrit tout
    // de suite pour que l'écran Facturation ne montre pas l'ancien effectif entre-temps.
    update_seats_column(state, org_id, org.seats, target).await
}

async fn update_seats_column(
    state: &AppState,
    org_id: Uuid,
    current: i32,
    target: u32,
) -> Result<u32, AppError> {
    if current != target as i32 {
        sqlx::query("UPDATE orgs SET seats = $2 WHERE id = $1")
            .bind(org_id)
            .bind(target as i32)
            .execute(&state.db)
            .await?;
    }
    Ok(target)
}

fn stripe_cfg(state: &AppState) -> Result<&StripeConfig, AppError> {
    state.cfg.stripe.as_ref().ok_or_else(|| {
        AppError::NotImplemented("La facturation n'est pas activée sur ce serveur.".into())
    })
}

#[cfg(test)]
mod tests {
    use super::stripe::plan_for_price;
    use super::*;

    fn org(sub: Option<&str>, status: Option<&str>) -> BillingOrg {
        BillingOrg {
            id: Uuid::nil(),
            name: "Acme".into(),
            plan: "free".into(),
            seats: 1,
            stripe_customer_id: None,
            stripe_subscription_id: sub.map(Into::into),
            subscription_status: status.map(Into::into),
            current_period_end: None,
            grace_until: None,
            owner_email: "a@b.c".into(),
        }
    }

    /// Le calcul que `sync_seats` applique après chaque arrivée et chaque départ.
    #[test]
    fn les_sieges_factures_suivent_l_effectif() {
        // §6 : minimum 3, même pour un fondateur seul.
        assert_eq!(billed_seats(1, None), MIN_SEATS);
        assert_eq!(billed_seats(0, None), MIN_SEATS);
        // Au-delà du minimum, la facture suit l'effectif.
        assert_eq!(billed_seats(7, None), 7);
        // Le client peut acheter de l'avance…
        assert_eq!(billed_seats(3, Some(10)), 10);
        // …mais jamais retirer un siège occupé : le compte du 7ᵉ membre planterait
        // à sa prochaine écriture, et c'est très pénible à diagnostiquer.
        assert_eq!(billed_seats(7, Some(3)), 7);
        assert_eq!(billed_seats(7, Some(0)), 7);
        // Plafond self-service, sauf si l'effectif le dépasse : on facture le vrai.
        assert_eq!(billed_seats(3, Some(99_999)), MAX_SEATS);
        assert_eq!(billed_seats(1200, None), 1200);
    }

    #[test]
    fn abonnement_en_cours() {
        assert!(has_live_subscription(&org(Some("sub_1"), Some("active"))));
        assert!(has_live_subscription(&org(Some("sub_1"), Some("past_due"))));
        // résilié : on doit pouvoir repasser au Checkout
        assert!(!has_live_subscription(&org(
            Some("sub_1"),
            Some("canceled")
        )));
        assert!(!has_live_subscription(&org(None, None)));
    }

    /// Le plan ne vient jamais de ce que poste le client (contrat §8).
    #[test]
    fn plan_non_deductible_du_corps_de_requete() {
        let cfg = StripeConfig {
            secret_key: "sk".into(),
            webhook_secret: "wh".into(),
            price_pro: Some("price_pro".into()),
            price_team: None,
            price_pro_yearly: None,
            price_team_yearly: None,
        };
        assert_eq!(plan_for_price(&cfg, "price_pro"), Some("pro"));
        // Team pas configuré : aucun price ne l'accorde
        assert_eq!(plan_for_price(&cfg, "price_team"), None);
    }
}
