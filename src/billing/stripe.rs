//! Client Stripe minimal — REST direct via `reqwest` (contrat §1 : `async-stripe` coûte
//! quatre minutes de compilation pour quatre endpoints).
//!
//! Deux pièges tenus ici une fois pour toutes :
//! - le corps est en `application/x-www-form-urlencoded`, jamais en JSON (l'API le refuse) ;
//! - toute création porte un en-tête `Idempotency-Key`.

use anyhow::anyhow;
use chrono::{DateTime, Utc};
use serde::{de::DeserializeOwned, Deserialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::{config::StripeConfig, error::Result};

const API: &str = "https://api.stripe.com/v1";

pub struct Stripe<'a> {
    http: &'a reqwest::Client,
    cfg: &'a StripeConfig,
}

impl<'a> Stripe<'a> {
    pub fn new(http: &'a reqwest::Client, cfg: &'a StripeConfig) -> Self {
        Self { http, cfg }
    }

    // ------------------------------------------------------------------ endpoints

    pub async fn create_customer(&self, org_id: Uuid, name: &str, email: &str) -> Result<Customer> {
        let form = vec![
            ("name", name.to_string()),
            ("email", email.to_string()),
            ("metadata[org_id]", org_id.to_string()),
        ];
        // Clé déterministe : un double clic sur « Passer à Pro » ne crée pas deux clients.
        self.post("/customers", &form, &format!("siglair-customer-{org_id}"))
            .await
    }

    pub async fn create_checkout_session(
        &self,
        org_id: Uuid,
        customer_id: &str,
        price_id: &str,
        quantity: u32,
        app_url: &str,
    ) -> Result<CheckoutSession> {
        let form = vec![
            ("mode", "subscription".to_string()),
            ("customer", customer_id.to_string()),
            ("line_items[0][price]", price_id.to_string()),
            ("line_items[0][quantity]", quantity.to_string()),
            (
                "success_url",
                format!(
                    "{app_url}/app/billing?checkout=success&session_id={{CHECKOUT_SESSION_ID}}"
                ),
            ),
            (
                "cancel_url",
                format!("{app_url}/app/billing?checkout=cancel"),
            ),
            // Sert uniquement à RETROUVER l'org dans le webhook. Le plan, lui, vient
            // toujours du price id de l'objet Stripe (contrat §8).
            ("client_reference_id", org_id.to_string()),
            ("subscription_data[metadata][org_id]", org_id.to_string()),
        ];
        self.post(
            "/checkout/sessions",
            &form,
            &format!("siglair-checkout-{}", Uuid::new_v4()),
        )
        .await
    }

    pub async fn create_portal_session(
        &self,
        customer_id: &str,
        return_url: &str,
    ) -> Result<PortalSession> {
        let form = vec![
            ("customer", customer_id.to_string()),
            ("return_url", return_url.to_string()),
        ];
        self.post(
            "/billing_portal/sessions",
            &form,
            &format!("siglair-portal-{}", Uuid::new_v4()),
        )
        .await
    }

    pub async fn get_subscription(&self, id: &str) -> Result<Subscription> {
        let res = self
            .http
            .get(format!("{API}/subscriptions/{id}"))
            .bearer_auth(&self.cfg.secret_key)
            .send()
            .await?;
        read(res, "/subscriptions").await
    }

    // ------------------------------------------------------------------ transport

    async fn post<T: DeserializeOwned>(
        &self,
        path: &str,
        form: &[(&str, String)],
        idempotency_key: &str,
    ) -> Result<T> {
        let res = self
            .http
            .post(format!("{API}{path}"))
            .bearer_auth(&self.cfg.secret_key)
            .header("Idempotency-Key", idempotency_key)
            .form(form)
            .send()
            .await?;
        read(res, path).await
    }
}

async fn read<T: DeserializeOwned>(res: reqwest::Response, path: &str) -> Result<T> {
    let status = res.status();
    let body = res.text().await?;
    if !status.is_success() {
        // Le message de Stripe part dans les logs, jamais dans la réponse : il contient
        // des identifiants internes et parfois la requête rejouée (contrat §5.4).
        let detail = serde_json::from_str::<ErrorEnvelope>(&body)
            .ok()
            .and_then(|e| e.error.message)
            .unwrap_or_default();
        tracing::error!(path, %status, %detail, "appel Stripe en échec");
        return Err(anyhow!("stripe {path} -> {status}").into());
    }
    Ok(serde_json::from_str(&body)?)
}

/// `pro` / `team` déduit du price id. `None` = price inconnu : on ne devine pas un plan.
///
/// `STRIPE_PRICE_PRO` et `STRIPE_PRICE_TEAM` acceptent plusieurs ids séparés par une
/// virgule, pour que le tarif annuel du contrat §6 pointe vers le même plan que le mensuel.
pub fn plan_for_price(cfg: &StripeConfig, price_id: &str) -> Option<&'static str> {
    if price_list(&cfg.price_pro).any(|p| p == price_id) {
        Some("pro")
    } else if price_list(&cfg.price_team).any(|p| p == price_id) {
        Some("team")
    } else {
        None
    }
}

/// Le premier id de la liste est celui proposé au Checkout.
pub fn primary_price(v: &Option<String>) -> Option<&str> {
    price_list(v).next()
}

fn price_list(v: &Option<String>) -> impl Iterator<Item = &str> + '_ {
    v.as_deref()
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|p| !p.is_empty())
}

// ---------------------------------------------------------------------- objets

#[derive(Debug, Deserialize)]
pub struct Customer {
    pub id: String,
}

#[derive(Debug, Deserialize)]
pub struct CheckoutSession {
    pub url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PortalSession {
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct Subscription {
    pub id: String,
    pub status: String,
    pub customer: String,
    #[serde(default)]
    pub current_period_end: Option<i64>,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
    #[serde(default)]
    pub items: SubscriptionItems,
}

#[derive(Debug, Default, Deserialize)]
pub struct SubscriptionItems {
    #[serde(default)]
    pub data: Vec<SubscriptionItem>,
}

#[derive(Debug, Deserialize)]
pub struct SubscriptionItem {
    #[serde(default)]
    pub quantity: Option<i64>,
    #[serde(default)]
    pub current_period_end: Option<i64>,
    pub price: Price,
}

#[derive(Debug, Deserialize)]
pub struct Price {
    pub id: String,
}

impl Subscription {
    pub fn price_id(&self) -> Option<&str> {
        self.items.data.first().map(|i| i.price.id.as_str())
    }

    /// Sièges facturés. La colonne `orgs.seats` a un CHECK `>= 1`.
    pub fn seats(&self) -> i32 {
        self.items
            .data
            .first()
            .and_then(|i| i.quantity)
            .unwrap_or(1)
            .clamp(1, 100_000) as i32
    }

    /// `current_period_end` est au niveau de l'abonnement dans les anciennes versions de
    /// l'API et au niveau de l'item depuis 2025 — on lit les deux plutôt que d'épingler
    /// une version qui divergera du réglage du compte.
    pub fn period_end(&self) -> Option<DateTime<Utc>> {
        self.current_period_end
            .or_else(|| self.items.data.first().and_then(|i| i.current_period_end))
            .and_then(|s| DateTime::from_timestamp(s, 0))
    }
}

#[derive(Deserialize)]
struct ErrorEnvelope {
    error: ErrorDetail,
}

#[derive(Deserialize)]
struct ErrorDetail {
    message: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> StripeConfig {
        StripeConfig {
            secret_key: "sk_test".into(),
            webhook_secret: "whsec_test".into(),
            price_pro: Some("price_pro_m, price_pro_y".into()),
            price_team: Some("price_team_m".into()),
        }
    }

    #[test]
    fn le_plan_vient_du_price_id() {
        let c = cfg();
        assert_eq!(plan_for_price(&c, "price_pro_m"), Some("pro"));
        assert_eq!(plan_for_price(&c, "price_pro_y"), Some("pro"));
        assert_eq!(plan_for_price(&c, "price_team_m"), Some("team"));
        // price d'un autre compte / env pas à jour : surtout ne rien accorder
        assert_eq!(plan_for_price(&c, "price_inconnu"), None);
        assert_eq!(plan_for_price(&c, ""), None);
        assert_eq!(primary_price(&c.price_pro), Some("price_pro_m"));
        assert_eq!(primary_price(&None), None);
    }

    #[test]
    fn periode_lue_aux_deux_emplacements() {
        let ancien: Subscription = serde_json::from_str(
            r#"{"id":"sub_1","status":"active","customer":"cus_1","current_period_end":1000,
                "items":{"data":[{"quantity":3,"price":{"id":"price_team_m"}}]}}"#,
        )
        .unwrap();
        let recent: Subscription = serde_json::from_str(
            r#"{"id":"sub_1","status":"active","customer":"cus_1",
                "items":{"data":[{"quantity":3,"current_period_end":1000,"price":{"id":"price_team_m"}}]}}"#,
        )
        .unwrap();
        assert_eq!(ancien.period_end(), recent.period_end());
        assert_eq!(recent.seats(), 3);
        assert_eq!(recent.price_id(), Some("price_team_m"));
    }
}
