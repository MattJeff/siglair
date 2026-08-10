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

use super::tax;
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
        let mut form = vec![
            ("name", name.to_string()),
            ("email", email.to_string()),
            ("metadata[org_id]", org_id.to_string()),
        ];
        // Mentions légales françaises portées par chaque facture d'échéance.
        form.extend(tax::customer_invoice_fields());
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
        let form = checkout_form(org_id, customer_id, price_id, quantity, app_url);
        self.post(
            "/checkout/sessions",
            &form,
            &format!("siglair-checkout-{}", Uuid::new_v4()),
        )
        .await
    }

    /// Aligne la quantité facturée de l'abonnement sur `quantity`.
    ///
    /// `item_id` est obligatoire : `items[0][quantity]` seul, sans l'identifiant de la ligne,
    /// est lu par Stripe comme la création d'une **seconde** ligne — l'org se retrouve
    /// facturée deux fois.
    ///
    /// La clé d'idempotence contient la quantité visée : deux appels concurrents pour le même
    /// effectif ne produisent qu'une seule proration (fenêtre Stripe de 24 h).
    pub async fn update_subscription_quantity(
        &self,
        subscription_id: &str,
        item_id: &str,
        quantity: u32,
    ) -> Result<Subscription> {
        let form = vec![
            ("items[0][id]", item_id.to_string()),
            ("items[0][quantity]", quantity.to_string()),
            // Un siège ajouté le 12 se paie au prorata, pas au mois plein, et un siège
            // retiré est remboursé au prorata sur l'échéance suivante.
            ("proration_behavior", "create_prorations".to_string()),
        ];
        self.post(
            &format!("/subscriptions/{subscription_id}"),
            &form,
            &format!("siglair-seats-{subscription_id}-{quantity}"),
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

/// Corps de la Checkout Session, isolé de l'appel réseau pour être vérifiable.
///
/// C'est du code de paiement : le seul moyen honnête de contrôler qu'un champ de TVA est
/// bien joint est de regarder le formulaire qui part, pas la fonction censée l'ajouter.
pub(crate) fn checkout_form(
    org_id: Uuid,
    customer_id: &str,
    price_id: &str,
    quantity: u32,
    app_url: &str,
) -> Vec<(&'static str, String)> {
    let mut form = vec![
        ("mode", "subscription".to_string()),
        ("customer", customer_id.to_string()),
        ("line_items[0][price]", price_id.to_string()),
        ("line_items[0][quantity]", quantity.to_string()),
        (
            "success_url",
            format!("{app_url}/app/billing?checkout=success&session_id={{CHECKOUT_SESSION_ID}}"),
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
    // TVA, n° intracommunautaire, adresse de facturation : voir `super::tax`.
    form.extend(tax::checkout_fields());
    form
}

async fn read<T: DeserializeOwned>(res: reqwest::Response, path: &str) -> Result<T> {
    let status = res.status();
    let body = res.text().await?;
    if !status.is_success() {
        // Le message de Stripe part dans les logs, jamais dans la réponse : il contient
        // des identifiants internes et parfois la requête rejouée (contrat §5.4).
        let detail = serde_json::from_str::<ErrorEnvelope>(&body)
            .map(|e| e.error)
            .unwrap_or_default();
        let (code, param, message) = (
            detail.code.unwrap_or_default(),
            detail.param.unwrap_or_default(),
            detail.message.unwrap_or_default(),
        );
        tracing::error!(path, %status, %code, %param, %message, "appel Stripe en échec");
        // Stripe Tax pas activé sur le compte : configuration à faire une fois, pas une
        // panne. Une 500 enverrait l'opérateur chercher un bug qui n'existe pas.
        if tax::is_tax_not_configured(&param, &code, &message) {
            return Err(tax::not_configured_error());
        }
        return Err(anyhow!("stripe {path} -> {status}").into());
    }
    Ok(serde_json::from_str(&body)?)
}

/// Périodicité facturée. `month` par défaut : c'est ce que poste le front existant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interval {
    Month,
    Year,
}

impl Interval {
    /// `None` (champ absent) vaut `month`. Toute autre valeur est refusée plutôt que
    /// repliée sur le mensuel : facturer une autre périodicité que celle choisie est un
    /// litige, pas une tolérance.
    pub fn parse(raw: Option<&str>) -> Option<Self> {
        match raw.unwrap_or("month").trim() {
            "month" => Some(Self::Month),
            "year" => Some(Self::Year),
            _ => None,
        }
    }
}

/// `pro` / `team` déduit du price id, sur les **quatre** ids possibles (mensuel + annuel).
/// `None` = price inconnu : on ne devine pas un plan.
///
/// `STRIPE_PRICE_PRO` et `STRIPE_PRICE_TEAM` acceptent en plus plusieurs ids séparés par une
/// virgule (anciens tarifs conservés pour les abonnements en cours).
pub fn plan_for_price(cfg: &StripeConfig, price_id: &str) -> Option<&'static str> {
    if price_id.is_empty() {
        return None;
    }
    if price_list(&cfg.price_pro)
        .chain(cfg.price_pro_yearly.as_deref())
        .any(|p| p == price_id)
    {
        Some("pro")
    } else if price_list(&cfg.price_team)
        .chain(cfg.price_team_yearly.as_deref())
        .any(|p| p == price_id)
    {
        Some("team")
    } else {
        None
    }
}

/// Le price à proposer au Checkout pour ce couple plan/périodicité.
///
/// `None` = ce couple n'est pas configuré sur ce serveur. L'appelant le dit à l'utilisateur ;
/// il ne se replie **jamais** sur l'autre périodicité.
pub fn price_for<'a>(cfg: &'a StripeConfig, plan: &str, interval: Interval) -> Option<&'a str> {
    match (plan, interval) {
        ("pro", Interval::Month) => primary_price(&cfg.price_pro),
        ("team", Interval::Month) => primary_price(&cfg.price_team),
        ("pro", Interval::Year) => cfg.price_pro_yearly.as_deref(),
        ("team", Interval::Year) => cfg.price_team_yearly.as_deref(),
        _ => None,
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
    /// `si_…`. Requis pour modifier la quantité sans créer une seconde ligne.
    /// `default` : les objets fabriqués dans les tests ne le portent pas.
    #[serde(default)]
    pub id: String,
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

    /// La ligne facturée. Un abonnement Siglair n'en a qu'une (un plan, une quantité).
    pub fn first_item(&self) -> Option<&SubscriptionItem> {
        self.items.data.first()
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

#[derive(Default, Deserialize)]
struct ErrorDetail {
    #[serde(default)]
    message: Option<String>,
    /// `resource_missing`, `card_declined`, `tax_origin_address_missing`…
    #[serde(default)]
    code: Option<String>,
    /// Le champ du formulaire refusé — seul repère stable pour classer l'erreur.
    #[serde(default)]
    param: Option<String>,
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
            price_pro_yearly: Some("price_pro_yearly".into()),
            price_team_yearly: Some("price_team_yearly".into()),
        }
    }

    /// Le même serveur sans tarif annuel configuré — le cas par défaut en production
    /// tant que les deux price ids ne sont pas créés dans le tableau de bord.
    fn cfg_sans_annuel() -> StripeConfig {
        StripeConfig {
            price_pro_yearly: None,
            price_team_yearly: None,
            ..cfg()
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

    /// Les quatre price ids du contrat §6. Un id annuel non reconnu ferait retomber
    /// l'abonné sur « price non reconnu » au premier webhook, donc sur l'ancien plan.
    #[test]
    fn les_quatre_price_ids_sont_reconnus() {
        let c = cfg();
        assert_eq!(plan_for_price(&c, "price_pro_m"), Some("pro"));
        assert_eq!(plan_for_price(&c, "price_pro_yearly"), Some("pro"));
        assert_eq!(plan_for_price(&c, "price_team_m"), Some("team"));
        assert_eq!(plan_for_price(&c, "price_team_yearly"), Some("team"));
        // Annuel non configuré : l'id d'un autre compte n'accorde toujours rien.
        assert_eq!(plan_for_price(&cfg_sans_annuel(), "price_pro_yearly"), None);
        // Un id vide ne doit pas correspondre à un `chain(None)` ni à une liste vide.
        assert_eq!(plan_for_price(&c, ""), None);
    }

    #[test]
    fn periodicite() {
        // absente = mensuel : c'est ce que poste le front existant
        assert_eq!(Interval::parse(None), Some(Interval::Month));
        assert_eq!(Interval::parse(Some("month")), Some(Interval::Month));
        assert_eq!(Interval::parse(Some("year")), Some(Interval::Year));
        // pas de repli silencieux sur le mensuel
        assert_eq!(Interval::parse(Some("annual")), None);
        assert_eq!(Interval::parse(Some("")), None);

        let c = cfg();
        assert_eq!(price_for(&c, "pro", Interval::Month), Some("price_pro_m"));
        assert_eq!(price_for(&c, "team", Interval::Month), Some("price_team_m"));
        assert_eq!(
            price_for(&c, "pro", Interval::Year),
            Some("price_pro_yearly")
        );
        assert_eq!(
            price_for(&c, "team", Interval::Year),
            Some("price_team_yearly")
        );
        assert_eq!(price_for(&c, "free", Interval::Month), None);
        // STRIPE_PRICE_*_YEARLY absent : l'annuel n'est pas proposé, et surtout il ne
        // retombe PAS sur le price mensuel — l'appelant répond 501.
        assert_eq!(price_for(&cfg_sans_annuel(), "pro", Interval::Year), None);
        assert_eq!(price_for(&cfg_sans_annuel(), "team", Interval::Year), None);
    }

    #[test]
    fn la_quantite_porte_l_identifiant_de_ligne() {
        let sub: Subscription = serde_json::from_str(
            r#"{"id":"sub_1","status":"active","customer":"cus_1",
                "items":{"data":[{"id":"si_42","quantity":3,"price":{"id":"price_team_m"}}]}}"#,
        )
        .unwrap();
        // Sans cet id, `items[0][quantity]` crée une SECONDE ligne et double la facture.
        assert_eq!(sub.first_item().map(|i| i.id.as_str()), Some("si_42"));
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
