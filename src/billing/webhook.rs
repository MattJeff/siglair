//! `POST /api/stripe/webhook` — contrat §5.3 et §8.
//!
//! Ordre imposé : vérifier `Stripe-Signature` sur le corps **brut** AVANT de désérialiser.
//! D'où `axum::body::Bytes` plutôt qu'un extracteur `Json` : ré-encoder le JSON change
//! d'un octet et l'HMAC ne correspond plus jamais.

use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::Utc;
use hmac::{Hmac, Mac};
use serde::Deserialize;
use serde_json::json;
use sha2::Sha256;
use sqlx::PgPool;
use subtle::ConstantTimeEq;
use uuid::Uuid;

use super::lifecycle;
use super::stripe::{plan_for_price, Stripe, Subscription};
use crate::{
    config::StripeConfig,
    growth::{self, GrowthEvent, Visitor},
    AppState,
};

/// Tolérance de dérive d'horloge (contrat §8). Au-delà, l'événement est un rejeu.
const TOLERANCE_SECS: i64 = 300;

pub async fn handle(State(state): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    let Some(cfg) = state.cfg.stripe.as_ref() else {
        return (StatusCode::NOT_IMPLEMENTED, "billing disabled").into_response();
    };

    let header = headers
        .get("stripe-signature")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    if let Err(e) = verify_signature(header, &body, &cfg.webhook_secret, Utc::now().timestamp()) {
        // Pas de détail dans la réponse : dire *pourquoi* une signature est refusée
        // aide à en forger une.
        tracing::warn!(reason = ?e, "webhook Stripe : signature refusée");
        return StatusCode::BAD_REQUEST.into_response();
    }

    match process(&state, cfg, &body).await {
        Ok(()) => {
            // ponytail : le balayage des impayés (relances J+1/J+3/J+6 et bascule vers
            // Free) roule sur le trafic webhook plutôt que sur un cron — et ce trafic est
            // précisément dense pendant un impayé, puisque Stripe réessaie. Détaché : ni
            // le délai ni l'échec du balayage ne doivent changer la réponse à Stripe, qui
            // rejouerait l'événement. Plafond connu, remplacement en une ligne :
            // `tokio::spawn` d'une boucle horaire sur `lifecycle::sweep` dans bin/api.rs.
            let st = state.clone();
            tokio::spawn(async move {
                if let Err(e) = lifecycle::sweep(&st).await {
                    tracing::error!(error = ?e, "balayage des impayés en échec");
                }
            });
            StatusCode::OK.into_response()
        }
        // 500 = Stripe réessaiera. C'est voulu : un échec transitoire de la DB ne doit pas
        // perdre l'activation d'un abonnement payé.
        Err(e) => {
            tracing::error!(error = ?e, "webhook Stripe : traitement en échec");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

// ---------------------------------------------------------------- signature

#[derive(Debug, PartialEq, Eq)]
pub enum SigError {
    /// En-tête absente, sans `t=` ou sans aucun `v1=`.
    Malformed,
    Expired,
    Mismatch,
}

/// Vérifie l'en-tête `Stripe-Signature` : `t=<ts>,v1=<hex>[,v1=<hex>]`.
/// La charge signée est littéralement `"{t}.{corps brut}"`.
pub fn verify_signature(
    header: &str,
    payload: &[u8],
    secret: &str,
    now: i64,
) -> Result<(), SigError> {
    let mut timestamp = None;
    let mut candidates: Vec<&str> = Vec::new();
    for part in header.split(',') {
        match part.trim().split_once('=') {
            Some(("t", v)) => timestamp = v.parse::<i64>().ok(),
            Some(("v1", v)) => candidates.push(v),
            _ => {} // v0 (Connect) et clés inconnues : ignorées
        }
    }

    let t = timestamp.ok_or(SigError::Malformed)?;
    if candidates.is_empty() {
        return Err(SigError::Malformed);
    }
    if (now - t).abs() > TOLERANCE_SECS {
        return Err(SigError::Expired);
    }

    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret.as_bytes()).map_err(|_| SigError::Malformed)?;
    mac.update(t.to_string().as_bytes());
    mac.update(b".");
    mac.update(payload);
    let expected = mac.finalize().into_bytes();

    // Comparaison en temps constant : un `==` sur les octets révèle par son temps
    // d'exécution la position du premier octet faux, donc permet de deviner l'HMAC.
    let ok = candidates
        .into_iter()
        .filter_map(hex_decode)
        .any(|got| bool::from(got[..].ct_eq(&expected[..])));

    if ok {
        Ok(())
    } else {
        Err(SigError::Mismatch)
    }
}

fn hex_decode(s: &str) -> Option<Vec<u8>> {
    let b = s.as_bytes();
    if b.is_empty() || b.len() % 2 != 0 {
        return None;
    }
    (0..b.len() / 2)
        .map(|i| u8::from_str_radix(std::str::from_utf8(&b[i * 2..i * 2 + 2]).ok()?, 16).ok())
        .collect()
}

// ---------------------------------------------------------------- traitement

#[derive(Deserialize)]
struct Event {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    data: EventData,
}

#[derive(Deserialize)]
struct EventData {
    object: serde_json::Value,
}

/// Objet `checkout.session` — seuls les champs dont on a besoin.
#[derive(Deserialize)]
struct SessionObject {
    #[serde(default)]
    customer: Option<String>,
    #[serde(default)]
    subscription: Option<String>,
    #[serde(default)]
    client_reference_id: Option<String>,
}

/// Objet `invoice` — le plan ne bouge pas sur un impayé, on ne lit donc que de quoi
/// retrouver l'org et, pour le 3-D Secure, la page où le client s'authentifie.
#[derive(Deserialize)]
struct InvoiceObject {
    #[serde(default)]
    customer: Option<String>,
    /// Page Stripe hébergée portant le bouton d'authentification bancaire.
    #[serde(default)]
    hosted_invoice_url: Option<String>,
}

/// Objet `dispute`. Il ne porte **pas** de `customer` : remonter à l'organisation
/// demanderait un aller-retour de plus chez Stripe, pour une alerte qui se traite de
/// toute façon dans le tableau de bord Stripe.
#[derive(Deserialize)]
struct DisputeObject {
    id: String,
    #[serde(default)]
    amount: i64,
    #[serde(default)]
    reason: Option<String>,
}

async fn process(state: &AppState, cfg: &StripeConfig, body: &[u8]) -> anyhow::Result<()> {
    let event: Event = serde_json::from_slice(body)?;
    let id = event.id.clone();

    // Idempotence (contrat §8) : la clé primaire de `stripe_events` fait le verrou.
    let fresh = sqlx::query("INSERT INTO stripe_events (id) VALUES ($1) ON CONFLICT DO NOTHING")
        .bind(&id)
        .execute(&state.db)
        .await?
        .rows_affected()
        == 1;
    if !fresh {
        return Ok(());
    }

    if let Err(e) = apply(state, cfg, event).await {
        // Sans ça, le réessai de Stripe verrait un doublon et l'événement serait perdu.
        let _ = sqlx::query("DELETE FROM stripe_events WHERE id = $1")
            .bind(&id)
            .execute(&state.db)
            .await;
        return Err(e);
    }
    Ok(())
}

async fn apply(state: &AppState, cfg: &StripeConfig, event: Event) -> anyhow::Result<()> {
    match event.kind.as_str() {
        "checkout.session.completed" => {
            let s: SessionObject = serde_json::from_value(event.data.object)?;
            let Some(org) = resolve_org(
                &state.db,
                s.customer.as_deref(),
                s.client_reference_id.as_deref(),
            )
            .await?
            else {
                return unknown_org(&event.kind);
            };
            if let Some(customer) = s.customer.as_deref() {
                // Paiement démarré hors de notre flux : on rattache le client à l'org.
                sqlx::query(
                    "UPDATE orgs SET stripe_customer_id = $1 \
                     WHERE id = $2 AND stripe_customer_id IS NULL",
                )
                .bind(customer)
                .bind(org)
                .execute(&state.db)
                .await?;
            }
            if let Some(sub_id) = s.subscription.as_deref() {
                // On relit l'abonnement chez Stripe : la session de Checkout ne porte pas
                // le price, et le price est la seule source du plan.
                let sub = Stripe::new(&state.http, cfg)
                    .get_subscription(sub_id)
                    .await?;
                apply_subscription(state, cfg, org, &sub).await?;
            }
        }

        "customer.subscription.created" | "customer.subscription.updated" => {
            let sub: Subscription = serde_json::from_value(event.data.object)?;
            let Some(org) = resolve_org(
                &state.db,
                Some(&sub.customer),
                sub.metadata.get("org_id").map(String::as_str),
            )
            .await?
            else {
                return unknown_org(&event.kind);
            };
            apply_subscription(state, cfg, org, &sub).await?;
        }

        "customer.subscription.deleted" => {
            let sub: Subscription = serde_json::from_value(event.data.object)?;
            let Some(org) = resolve_org(
                &state.db,
                Some(&sub.customer),
                sub.metadata.get("org_id").map(String::as_str),
            )
            .await?
            else {
                return unknown_org(&event.kind);
            };
            // Retour à Free. Aucune donnée n'est effacée : les signatures publiées
            // au-delà du quota Free cessent simplement d'être servies (402).
            sqlx::query(
                "UPDATE orgs SET plan = 'free', seats = 1, stripe_subscription_id = NULL, \
                 subscription_status = 'canceled', current_period_end = NULL WHERE id = $1",
            )
            .bind(org)
            .execute(&state.db)
            .await?;
        }

        // Le portail Stripe autorise la pause : sans ces deux branches, un client qui met
        // son abonnement en pause continue d'être servi, et celui qui le reprend reste
        // bloqué au plan Free.
        "customer.subscription.paused" => {
            let sub: Subscription = serde_json::from_value(event.data.object)?;
            let Some(org) = sub_org(state, &sub).await? else {
                return unknown_org(&event.kind);
            };
            lifecycle::pause(state, org).await?;
        }

        "customer.subscription.resumed" => {
            let sub: Subscription = serde_json::from_value(event.data.object)?;
            let Some(org) = sub_org(state, &sub).await? else {
                return unknown_org(&event.kind);
            };
            // Le plan est relu depuis le price id, comme partout ailleurs.
            apply_subscription(state, cfg, org, &sub).await?;
            lifecycle::clear_dunning(state, org).await?;
        }

        "customer.subscription.trial_will_end" => {
            let sub: Subscription = serde_json::from_value(event.data.object)?;
            let Some(org) = sub_org(state, &sub).await? else {
                return unknown_org(&event.kind);
            };
            lifecycle::trial_will_end(state, org).await?;
        }

        "invoice.payment_failed" => {
            let inv: InvoiceObject = serde_json::from_value(event.data.object)?;
            let Some(org) = resolve_org(&state.db, inv.customer.as_deref(), None).await? else {
                return unknown_org(&event.kind);
            };
            // Impayé ≠ résiliation : le plan et les données restent, l'accès aussi
            // pendant sept jours (lifecycle.rs), et Stripe relance de son côté.
            lifecycle::begin_grace(state, org, "past_due").await?;
        }

        // 3-D Secure. Obligatoire en Europe, donc ce cas arrive dès les premiers clients :
        // le paiement n'est ni accepté ni refusé, il attend le client, qui ne sait pas
        // qu'on l'attend.
        "invoice.payment_action_required" => {
            let inv: InvoiceObject = serde_json::from_value(event.data.object)?;
            let Some(org) = resolve_org(&state.db, inv.customer.as_deref(), None).await? else {
                return unknown_org(&event.kind);
            };
            lifecycle::require_action(state, org, inv.hosted_invoice_url.as_deref()).await?;
        }

        // Sortie propre de l'impayé : c'est le seul événement qui referme la période de
        // grâce. `invoice.payment_succeeded` ne suffirait pas — une facture peut être
        // soldée hors carte (virement, avoir).
        "invoice.paid" => {
            let inv: InvoiceObject = serde_json::from_value(event.data.object)?;
            let Some(org) = resolve_org(&state.db, inv.customer.as_deref(), None).await? else {
                return unknown_org(&event.kind);
            };
            lifecycle::clear_dunning(state, org).await?;
        }

        // On ne coupe rien : une contestation est le plus souvent un client qui ne
        // reconnaît pas un libellé. Mais ignorée, elle est perdue d'office.
        "charge.dispute.created" => {
            let d: DisputeObject = serde_json::from_value(event.data.object)?;
            lifecycle::dispute_opened(
                state,
                &d.id,
                d.amount,
                d.reason.as_deref().unwrap_or("non précisé"),
            )
            .await;
        }

        _ => {}
    }
    Ok(())
}

/// Org d'un événement d'abonnement. Les métadonnées ne servent qu'à identifier, jamais à
/// accorder un plan (contrat §8).
async fn sub_org(state: &AppState, sub: &Subscription) -> anyhow::Result<Option<Uuid>> {
    resolve_org(
        &state.db,
        Some(&sub.customer),
        sub.metadata.get("org_id").map(String::as_str),
    )
    .await
}

async fn apply_subscription(
    state: &AppState,
    cfg: &StripeConfig,
    org: Uuid,
    sub: &Subscription,
) -> anyhow::Result<()> {
    let priced = sub.price_id().and_then(|p| plan_for_price(cfg, p));
    // `unpaid` est dans la liste — ce n'est pas un oubli inversé. Stripe y passe quand SA
    // politique de relance est épuisée, et cette politique est réglable dans son tableau
    // de bord : réglée à trois jours, elle couperait un client à qui l'on vient de
    // promettre sept jours par e-mail. C'est `grace_until` qui tranche, et c'est
    // `lifecycle::sweep` qui bascule le plan à l'échéance.
    let active = matches!(
        sub.status.as_str(),
        "active" | "trialing" | "past_due" | "unpaid"
    );

    // État d'avant, pour ne compter l'abonnement qu'au moment où il DEVIENT actif (§11.1).
    // Stripe envoie un `customer.subscription.updated` à chaque renouvellement, à chaque
    // changement de sièges et à chaque changement de carte : sans cette comparaison, un
    // client fidèle compterait comme une conversion tous les mois.
    //
    // `.ok().flatten()` et pas `?` : cette lecture ne sert qu'à la mesure. Une mesure ne
    // fait pas échouer ce qu'elle mesure — un `?` ici transformerait un hoquet de base en
    // abonnement non appliqué, donc en client qui a payé sans recevoir son plan.
    let plan_before: Option<String> = sqlx::query_scalar("SELECT plan FROM orgs WHERE id = $1")
        .bind(org)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

    let plan = match (active, priced) {
        (true, Some(p)) => Some(p),
        // incomplete_expired, canceled, unpaid…
        (false, _) => Some("free"),
        // Price absent de STRIPE_PRICE_* : on ne devine pas un plan, on laisse l'existant.
        (true, None) => {
            tracing::warn!(price = ?sub.price_id(), %org, "price Stripe non reconnu");
            None
        }
    };

    sqlx::query(
        "UPDATE orgs SET plan = COALESCE($2::text, plan), seats = $3, \
         stripe_subscription_id = $4, subscription_status = $5, current_period_end = $6 \
         WHERE id = $1",
    )
    .bind(org)
    .bind(plan)
    .bind(sub.seats())
    .bind(&sub.id)
    .bind(&sub.status)
    .bind(sub.period_end())
    .execute(&state.db)
    .await?;

    // §11.1 `upgrade_completed` : « combien paient ». Une seule fois par passage au payant,
    // et jamais sur un renouvellement — cf. `plan_before` juste au-dessus.
    if let Some(paid_plan) = plan.filter(|p| *p != "free") {
        // « Payait déjà » se lit sur la colonne `plan`, pas sur une seconde liste de statuts
        // recopiée ici : c'est cette colonne qui donne les droits, et la résiliation comme
        // la fin de grâce la remettent à `free`. Une org passée à Pro à la main puis
        // abonnée pour de bon ne comptera pas — cas de secours interne, pas une conversion.
        // Positivement « était gratuite » : si la lecture ci-dessus a échoué, on ne compte
        // rien plutôt que d'inventer une conversion à chaque renouvellement.
        let was_free = plan_before.as_deref() == Some("free");
        if active && was_free {
            let (db, salt) = (state.db.clone(), state.cfg.ip_salt.clone());
            tokio::spawn(async move {
                growth::record(
                    &db,
                    org,
                    // Stripe parle d'une organisation, pas d'un utilisateur : personne
                    // n'est connecté ici, et deviner le propriétaire serait une invention.
                    None,
                    None,
                    GrowthEvent::UpgradeCompleted,
                    json!({ "plan": paid_plan }),
                    Visitor::unknown(&salt),
                )
                .await;
            });
        }
    }

    // Filet : un `customer.subscription.updated` peut annoncer l'impayé sans qu'aucun
    // `invoice.payment_failed` ne nous soit parvenu (webhook manqué, abonnement importé).
    // Sans fenêtre de grâce, `sweep` ne verrait jamais cette org et le client garderait
    // son plan payant indéfiniment. `begin_grace` est idempotent : elle ne repousse rien.
    if matches!(sub.status.as_str(), "past_due" | "unpaid") {
        lifecycle::begin_grace(state, org, &sub.status).await?;
    }
    Ok(())
}

/// L'org se retrouve par `stripe_customer_id` (posé par nous au Checkout) ; les métadonnées
/// ne servent que de repli d'identification — jamais à décider d'un plan.
async fn resolve_org(
    db: &PgPool,
    customer: Option<&str>,
    meta_org_id: Option<&str>,
) -> anyhow::Result<Option<Uuid>> {
    if let Some(c) = customer {
        let found: Option<Uuid> =
            sqlx::query_scalar("SELECT id FROM orgs WHERE stripe_customer_id = $1")
                .bind(c)
                .fetch_optional(db)
                .await?;
        if found.is_some() {
            return Ok(found);
        }
    }
    let Some(id) = meta_org_id.and_then(|s| Uuid::parse_str(s).ok()) else {
        return Ok(None);
    };
    Ok(sqlx::query_scalar("SELECT id FROM orgs WHERE id = $1")
        .bind(id)
        .fetch_optional(db)
        .await?)
}

/// Événement d'un client qui n'existe pas chez nous (compte Stripe partagé, org supprimée).
/// On répond 200 : réessayer ne le fera pas apparaître.
fn unknown_org(kind: &str) -> anyhow::Result<()> {
    tracing::warn!(kind, "webhook Stripe : aucune org correspondante");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "whsec_abc123";
    const PAYLOAD: &[u8] = br#"{"id":"evt_1","type":"customer.subscription.updated"}"#;

    fn hex_encode(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }

    fn sign(secret: &str, t: i64, payload: &[u8]) -> String {
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(t.to_string().as_bytes());
        mac.update(b".");
        mac.update(payload);
        format!("t={t},v1={}", hex_encode(&mac.finalize().into_bytes()))
    }

    #[test]
    fn signature_valide() {
        let now = 1_700_000_000;
        let h = sign(SECRET, now, PAYLOAD);
        assert_eq!(verify_signature(&h, PAYLOAD, SECRET, now), Ok(()));
        // dérive acceptable
        assert_eq!(verify_signature(&h, PAYLOAD, SECRET, now + 299), Ok(()));
        // plusieurs v1 : il suffit qu'un seul corresponde (rotation de secret Stripe)
        let multi = format!("{h},v1={}", hex_encode(&[0u8; 32]));
        assert_eq!(verify_signature(&multi, PAYLOAD, SECRET, now), Ok(()));
    }

    #[test]
    fn signature_falsifiee() {
        let now = 1_700_000_000;
        let h = sign(SECRET, now, PAYLOAD);

        // corps modifié après signature
        let altered = br#"{"id":"evt_1","type":"customer.subscription.deleted"}"#;
        assert_eq!(
            verify_signature(&h, altered, SECRET, now),
            Err(SigError::Mismatch)
        );

        // horodatage recollé sur une autre signature
        let moved = h.replace("t=1700000000", "t=1700000001");
        assert_eq!(
            verify_signature(&moved, PAYLOAD, SECRET, now),
            Err(SigError::Mismatch)
        );

        // mauvais secret
        assert_eq!(
            verify_signature(&h, PAYLOAD, "whsec_autre", now),
            Err(SigError::Mismatch)
        );

        // en-têtes inexploitables
        assert_eq!(
            verify_signature("", PAYLOAD, SECRET, now),
            Err(SigError::Malformed)
        );
        assert_eq!(
            verify_signature(&format!("t={now}"), PAYLOAD, SECRET, now),
            Err(SigError::Malformed)
        );
        assert_eq!(
            verify_signature(&format!("t={now},v1=zz"), PAYLOAD, SECRET, now),
            Err(SigError::Mismatch)
        );
    }

    #[test]
    fn horodatage_expire() {
        let now = 1_700_000_000;
        let vieux = sign(SECRET, now - 301, PAYLOAD);
        assert_eq!(
            verify_signature(&vieux, PAYLOAD, SECRET, now),
            Err(SigError::Expired)
        );
        // horloge du serveur en retard : même verdict
        let futur = sign(SECRET, now + 301, PAYLOAD);
        assert_eq!(
            verify_signature(&futur, PAYLOAD, SECRET, now),
            Err(SigError::Expired)
        );
    }
}
