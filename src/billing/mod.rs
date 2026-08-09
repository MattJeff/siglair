//! Facturation Stripe — contrat §5.3 (routes) et §6 (grille tarifaire).

pub mod stripe;
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
use crate::{
    auth::CurrentUser,
    config::StripeConfig,
    error::AppError,
    plans::{Plan, TEAM},
    AppState,
};
use stripe::{primary_price, Stripe};

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

#[derive(Deserialize)]
struct CheckoutReq {
    plan: String,
    seats: Option<u32>,
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

    let (price, quantity) = match req.plan.as_str() {
        "pro" => (primary_price(&cfg.price_pro), 1),
        "team" => (
            primary_price(&cfg.price_team),
            req.seats.unwrap_or(MIN_SEATS).clamp(MIN_SEATS, 1000),
        ),
        _ => {
            return Err(AppError::validation(
                "Plan inconnu. Choisissez « pro » ou « team ».",
            ))
        }
    };
    // clé Stripe présente mais STRIPE_PRICE_{PRO,TEAM} absent : même 501, autre cause
    let price = price.ok_or_else(|| {
        AppError::NotImplemented(format!(
            "Le plan « {} » n'est pas proposé sur ce serveur.",
            req.plan
        ))
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
    Ok(Json(json!({
        "plan": Plan::get(&org.plan),
        "seats": org.seats,
        "status": org.subscription_status,
        "current_period_end": org.current_period_end,
        "has_subscription": org.stripe_subscription_id.is_some(),
        "min_seats": if org.plan == TEAM.id { MIN_SEATS } else { 1 },
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
    owner_email: String,
}

/// L'org facturable de l'utilisateur : celle dont il est **propriétaire**. Un membre ou un
/// admin n'engage pas de dépense au nom de l'organisation.
async fn billable_org(state: &AppState, user_id: Uuid) -> Result<BillingOrg, AppError> {
    sqlx::query_as::<_, BillingOrg>(
        "SELECT o.id, o.name, o.plan, o.seats, o.stripe_customer_id, o.stripe_subscription_id, \
                o.subscription_status, o.current_period_end, u.email AS owner_email \
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
            owner_email: "a@b.c".into(),
        }
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
        };
        assert_eq!(plan_for_price(&cfg, "price_pro"), Some("pro"));
        // Team pas configuré : aucun price ne l'accorde
        assert_eq!(plan_for_price(&cfg, "price_team"), None);
    }
}
