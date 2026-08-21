//! Machine à états d'un abonnement impayé — migration 0005.
//!
//! ```text
//!   active ──payment_failed──▶ past_due ──grâce 7 j──▶ unpaid ──▶ free
//!      ▲                          │
//!      └────── invoice.paid ──────┘
//! ```
//!
//! **Pendant la grâce, l'accès reste COMPLET.** Une carte réémise par la banque est le
//! cas le plus banal du monde ; couper le service d'un client qui paie depuis six mois
//! pour ça est le meilleur moyen de le perdre définitivement. On relance à J+1, J+3 et
//! J+6, puis on bascule.
//!
//! La bascule n'efface **jamais** une signature. Mais au-delà du quota Free, le GIF
//! hébergé cesse d'être servi : le client mail affiche une image cassée dans des e-mails
//! partis il y a des mois, chez des destinataires qu'on ne contrôle pas. C'est brutal et
//! irrattrapable, donc l'utilisateur est prévenu AVANT — c'est tout l'objet des relances.

use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;

use crate::{
    plans::{Plan, FREE},
    AppState,
};

/// Durée de la période de grâce, en jours. Écrite ici et nulle part ailleurs.
pub const GRACE_DAYS: i64 = 7;

/// Jours de grâce auxquels une relance part. Le dernier (J+6) est un avertissement de
/// dernière minute, pas une relance de plus : c'est celui qui doit faire agir.
const DUNNING_DAYS: [i64; 3] = [1, 3, 6];

/// Statuts Stripe pendant lesquels la période de grâce fait foi.
const IN_DUNNING: [&str; 4] = ["past_due", "unpaid", "paused", "incomplete"];

// ---------------------------------------------------------------- droit d'accès

/// Les trois colonnes qui décident du droit d'accès. Toute requête qui charge une org
/// pour en déduire des droits doit les sélectionner et passer par [`effective_plan`].
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct OrgBilling {
    pub plan: String,
    pub subscription_status: Option<String>,
    pub grace_until: Option<DateTime<Utc>>,
}

/// **Source unique du droit d'accès.** Aucune route ne lit `orgs.plan` directement : une
/// route oubliée laisserait passer un client impayé, ou pire, couperait un client en
/// grâce — celui qui va payer et à qui l'on vient d'afficher une image cassée.
pub fn effective_plan(org: &OrgBilling) -> Plan {
    effective_plan_at(org, Utc::now())
}

/// `effective_plan` avec l'horloge injectée. Publique pour les tests, et parce qu'un
/// « quel plan aura-t-il le 12 ? » n'a pas à passer par une horloge globale.
pub fn effective_plan_at(org: &OrgBilling, now: DateTime<Utc>) -> Plan {
    let subscribed = Plan::get(&org.plan);
    match org.subscription_status.as_deref() {
        // Jamais facturé : org gratuite, compte offert, migration antérieure à Stripe.
        // La colonne fait foi.
        None | Some("active" | "trialing") => subscribed,
        // Impayé : le plan payé reste entier tant que la grâce court. `grace_until` NULL
        // ici veut dire que la fenêtre a déjà été refermée (bascule faite, ou pause).
        Some(s) if IN_DUNNING.contains(&s) => match org.grace_until {
            Some(end) if end > now => subscribed,
            _ => FREE,
        },
        // canceled, incomplete_expired, statut inventé par une future version de l'API :
        // moins de droits, jamais plus.
        Some(_) => FREE,
    }
}

/// Numéro de la relance due (1..=3), `None` s'il est trop tôt ou si elle est déjà partie.
///
/// `sent` est le compteur persisté : un webhook rejoué relit le même compteur et repart
/// avec `None`. Si le balayage a sauté un jour, on ne rattrape pas les relances
/// manquées — deux e-mails d'un coup ne rendent pas le client plus solvable, ils le
/// rendent agacé.
pub fn due_dunning_stage(grace_until: DateTime<Utc>, now: DateTime<Utc>, sent: i32) -> Option<i32> {
    let elapsed = (now - (grace_until - Duration::days(GRACE_DAYS))).num_days();
    let stage = DUNNING_DAYS.iter().filter(|d| **d <= elapsed).count() as i32;
    (stage > sent).then_some(stage)
}

// ---------------------------------------------------------------- transitions

/// Entrée en impayé : ouvre la période de grâce si elle ne l'est pas déjà.
///
/// `COALESCE` sur `grace_until` : Stripe réessaie plusieurs fois et émet un
/// `payment_failed` à chaque tentative. Sans ça, la troisième tentative repousserait la
/// date de bascule et la grâce durerait indéfiniment.
///
/// `status` est le statut Stripe qui a déclenché l'impayé (`past_due`, puis `unpaid`
/// quand Stripe abandonne ses propres relances). On le recopie tel quel : c'est
/// `grace_until` qui décide de l'accès, pas lui. La politique de relance de Stripe est
/// réglable dans son tableau de bord ; notre promesse de sept jours, elle, ne l'est pas.
pub async fn begin_grace(state: &AppState, org: Uuid, status: &str) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE orgs SET subscription_status = $3, \
         grace_until = COALESCE(grace_until, now() + make_interval(days => $2)) \
         WHERE id = $1",
    )
    .bind(org)
    .bind(GRACE_DAYS as i32)
    .bind(status)
    .execute(&state.db)
    .await?;
    tracing::info!(%org, status, "abonnement en impayé : période de grâce ouverte");
    Ok(())
}

/// `invoice.paid` : sortie propre de l'impayé. Referme la grâce, remet le compteur de
/// relances à zéro et efface l'alerte 3-D Secure.
///
/// Le `WHERE` sur le statut est délibéré : une facture soldée après résiliation
/// (dernier prorata) ne doit pas ressusciter un abonnement `canceled`.
pub async fn clear_dunning(state: &AppState, org: Uuid) -> anyhow::Result<()> {
    let n = sqlx::query(
        "UPDATE orgs SET subscription_status = 'active', grace_until = NULL, \
         dunning_emails_sent = 0, requires_action = false \
         WHERE id = $1 AND (grace_until IS NOT NULL OR requires_action \
                            OR subscription_status IN ('past_due', 'unpaid'))",
    )
    .bind(org)
    .execute(&state.db)
    .await?
    .rows_affected();
    if n == 1 {
        tracing::info!(%org, "paiement encaissé : accès rétabli, alerte effacée");
    }
    Ok(())
}

/// `invoice.payment_action_required` — 3-D Secure. En Europe l'authentification forte
/// est la règle, pas l'exception : sans cette branche le paiement reste en attente et
/// l'utilisateur ne voit rien, jusqu'à ce que Stripe abandonne la facture. C'est un
/// abonnement déjà signé que l'on perd en silence.
pub async fn require_action(
    state: &AppState,
    org: Uuid,
    hosted_invoice_url: Option<&str>,
) -> anyhow::Result<()> {
    // L'UPDATE conditionnel est le verrou, comme pour les relances : Stripe émet un
    // `payment_action_required` à CHAQUE tentative, et chaque tentative est un événement
    // distinct que `stripe_events` ne déduplique pas. Sans ce `NOT requires_action`, le
    // client reçoit le même e-mail trois fois pour une seule facture en attente.
    let claimed =
        sqlx::query("UPDATE orgs SET requires_action = true WHERE id = $1 AND NOT requires_action")
            .bind(org)
            .execute(&state.db)
            .await?
            .rows_affected()
            == 1;
    if !claimed {
        return Ok(());
    }
    tracing::warn!(%org, "3-D Secure : authentification bancaire requise");

    // Sans le lien Stripe on ne peut pas déclencher l'authentification depuis chez nous :
    // on renvoie vers la facturation, d'où le portail Stripe est joignable.
    let fallback = billing_url(state);
    let link = hosted_invoice_url.unwrap_or(&fallback);
    notify_owner(
        state,
        org,
        "Action requise : confirmez votre paiement Siglair",
        "Votre banque demande une confirmation",
        &[
            "Votre banque exige une authentification (3-D Secure) pour valider le paiement de \
             votre abonnement Siglair. Tant qu'elle n'est pas faite, le paiement reste en \
             attente et sera abandonné au bout de quelques jours."
                .to_string(),
            "L'opération prend moins d'une minute et se passe sur la page sécurisée de notre \
             prestataire de paiement, Stripe."
                .to_string(),
        ],
        Some(("Confirmer le paiement", link)),
    )
    .await;
    Ok(())
}

/// `customer.subscription.paused` — mise en pause depuis le portail Stripe. Plus de
/// prélèvement, donc plus de plan payant ; le plan souscrit reste en colonne pour que
/// `resumed` le retrouve depuis le price id.
///
/// `plan = 'free'` en base, et pas seulement dans [`effective_plan`] : tant que toutes
/// les routes ne passent pas par `effective_plan`, une colonne restée à `pro` servirait
/// un plan payant à un abonnement qui ne prélève plus rien.
pub async fn pause(state: &AppState, org: Uuid) -> anyhow::Result<()> {
    // Même verrou que partout : un second `subscription.paused` (autre event id, même
    // pause) ne doit pas renvoyer l'e-mail. Le `WHERE` sur le statut fait la garde.
    let claimed = sqlx::query(
        "UPDATE orgs SET plan = 'free', seats = 1, subscription_status = 'paused', \
         grace_until = NULL WHERE id = $1 AND subscription_status IS DISTINCT FROM 'paused'",
    )
    .bind(org)
    .execute(&state.db)
    .await?
    .rows_affected()
        == 1;
    if !claimed {
        return Ok(());
    }
    tracing::info!(%org, "abonnement mis en pause");
    notify_owner(
        state,
        org,
        "Votre abonnement Siglair est en pause",
        "Abonnement en pause",
        &[
            "Votre abonnement est suspendu : plus aucun prélèvement n'aura lieu. Vos signatures \
             sont conservées, mais votre organisation revient aux limites du plan Free."
                .to_string(),
            "Au-delà de la limite Free, les GIF hébergés ne sont plus servis : les signatures \
             concernées apparaîtront cassées dans les e-mails déjà envoyés. Reprenez \
             l'abonnement pour les remettre en ligne immédiatement."
                .to_string(),
        ],
        Some(("Reprendre mon abonnement", &billing_url(state))),
    )
    .await;
    Ok(())
}

/// `customer.subscription.trial_will_end` — aucun essai n'est configuré aujourd'hui,
/// mais l'événement arrivera le jour où l'on en ajoutera un, et un essai qui se termine
/// sans prévenir se termine en résiliation.
pub async fn trial_will_end(state: &AppState, org: Uuid) -> anyhow::Result<()> {
    tracing::info!(%org, "fin d'essai imminente");
    notify_owner(
        state,
        org,
        "Votre essai Siglair se termine bientôt",
        "Fin d'essai imminente",
        &[
            "Votre période d'essai se termine dans quelques jours. À son terme, le premier \
             prélèvement aura lieu automatiquement et vos signatures resteront en ligne sans \
             aucune action de votre part."
                .to_string(),
            "Si vous ne souhaitez pas continuer, résiliez depuis votre espace de facturation \
             avant la fin de l'essai : rien ne sera prélevé."
                .to_string(),
        ],
        Some(("Voir ma facturation", &billing_url(state))),
    )
    .await;
    Ok(())
}

// ---------------------------------------------------------------- balayage

#[derive(sqlx::FromRow)]
struct GraceRow {
    id: Uuid,
    grace_until: DateTime<Utc>,
    dunning_emails_sent: i32,
}

/// Relances dues et bascules à échéance, en une passe.
///
/// ponytail : pas de tâche planifiée ni de table de jobs pour trois e-mails. La passe est
/// idempotente et déclenchée par le trafic webhook, qui est justement dense pendant une
/// période d'impayé (Stripe réessaie). Plafond connu : une org en grâce sur un compte
/// sans aucun autre trafic Stripe ne serait relancée qu'au réessai suivant. Le jour où ça
/// gêne, un `tokio::spawn` d'une boucle horaire dans `src/bin/api.rs` appelant `sweep`
/// suffit — la fonction est déjà écrite pour ça.
pub async fn sweep(state: &AppState) -> anyhow::Result<()> {
    let rows: Vec<GraceRow> = sqlx::query_as(
        "SELECT id, grace_until, dunning_emails_sent FROM orgs WHERE grace_until IS NOT NULL",
    )
    .fetch_all(&state.db)
    .await?;

    let now = Utc::now();
    for row in rows {
        let r = if row.grace_until <= now {
            expire(state, row.id).await
        } else {
            dun(state, &row, now).await
        };
        // Une org qui échoue ne doit pas empêcher les suivantes d'être relancées.
        if let Err(e) = r {
            tracing::error!(org = %row.id, error = ?e, "balayage des impayés : org ignorée");
        }
    }
    Ok(())
}

/// Envoie la relance due, une fois et une seule.
///
/// L'UPDATE conditionnel est le verrou : il est tenté AVANT l'envoi. Perdre une relance
/// sur un crash entre les deux est préférable à en envoyer deux à un client déjà agacé
/// par un prélèvement refusé.
async fn dun(state: &AppState, row: &GraceRow, now: DateTime<Utc>) -> anyhow::Result<()> {
    let Some(stage) = due_dunning_stage(row.grace_until, now, row.dunning_emails_sent) else {
        return Ok(());
    };
    let claimed = sqlx::query(
        "UPDATE orgs SET dunning_emails_sent = $2 WHERE id = $1 AND dunning_emails_sent < $2",
    )
    .bind(row.id)
    .bind(stage)
    .execute(&state.db)
    .await?
    .rows_affected()
        == 1;
    if !claimed {
        return Ok(());
    }

    let left = (row.grace_until - now).num_days().max(0);
    let jours = if left > 1 { "jours" } else { "jour" };
    let last = stage as usize == DUNNING_DAYS.len();
    tracing::warn!(org = %row.id, stage, left, "relance impayé envoyée");

    notify_owner(
        state,
        row.id,
        if last {
            "Dernier rappel : votre abonnement Siglair sera suspendu demain"
        } else {
            "Votre paiement Siglair n'a pas abouti"
        },
        if last {
            "Dernier rappel avant retour au plan Free"
        } else {
            "Nous n'avons pas pu encaisser votre abonnement"
        },
        &[
            format!(
                "Le prélèvement de votre abonnement Siglair a été refusé. Il vous reste \
                 {left} {jours} pour mettre à jour votre moyen de paiement. D'ici là, votre \
                 accès reste entier : rien n'est bloqué, rien n'est supprimé."
            ),
            "Passé ce délai, votre organisation revient au plan Free. Vos signatures sont \
             conservées, mais au-delà de la limite Free les GIF hébergés cessent d'être \
             servis : ils s'afficheront comme des images cassées dans les e-mails que vous \
             avez déjà envoyés, chez leurs destinataires."
                .to_string(),
            "La cause la plus fréquente est une carte expirée ou réémise par la banque. La \
             corriger prend une minute."
                .to_string(),
        ],
        Some(("Mettre à jour ma carte", &billing_url(state))),
    )
    .await;
    Ok(())
}

/// Fin de la grâce : retour à Free.
///
/// Aucune signature n'est supprimée, jamais. `stripe_subscription_id` est conservé :
/// l'abonnement existe toujours chez Stripe en `unpaid` et peut être régularisé depuis
/// le portail. L'UPDATE conditionnel garantit un seul e-mail de bascule.
async fn expire(state: &AppState, org: Uuid) -> anyhow::Result<()> {
    let flipped = sqlx::query(
        "UPDATE orgs SET plan = 'free', seats = 1, subscription_status = 'unpaid', \
         grace_until = NULL, dunning_emails_sent = 0, requires_action = false \
         WHERE id = $1 AND grace_until IS NOT NULL AND grace_until <= now()",
    )
    .bind(org)
    .execute(&state.db)
    .await?
    .rows_affected()
        == 1;
    if !flipped {
        return Ok(());
    }
    tracing::warn!(%org, "fin de période de grâce : retour au plan Free");

    notify_owner(
        state,
        org,
        "Votre organisation Siglair est repassée au plan Free",
        "Retour au plan Free",
        &[
            "Faute de paiement, votre organisation est revenue au plan Free. Aucune de vos \
             signatures n'a été supprimée : elles vous attendent telles quelles."
                .to_string(),
            "En revanche, au-delà de la limite du plan Free, les GIF hébergés ne sont plus \
             servis. Concrètement, les signatures concernées apparaissent désormais comme des \
             images cassées dans les e-mails que vous avez déjà envoyés."
                .to_string(),
            "Réactiver votre abonnement remet tout en ligne immédiatement, sans republier ni \
             recoller quoi que ce soit dans votre client mail."
                .to_string(),
        ],
        Some(("Réactiver mon abonnement", &billing_url(state))),
    )
    .await;
    Ok(())
}

// ---------------------------------------------------------------- notifications

fn billing_url(state: &AppState) -> String {
    format!("{}/app/billing", state.cfg.app_url)
}

/// E-mail au propriétaire de l'organisation — c'est lui qui paie, et lui seul qui peut
/// corriger une carte (cf. `billable_org`).
///
/// Ne renvoie pas d'erreur : un Brevo indisponible ne doit pas faire répondre 500 au
/// webhook Stripe, ce qui ferait rejouer l'événement et re-tenter l'envoi en boucle.
async fn notify_owner(
    state: &AppState,
    org: Uuid,
    subject: &str,
    heading: &str,
    paragraphs: &[String],
    cta: Option<(&str, &str)>,
) {
    let to: Result<Option<String>, _> = sqlx::query_scalar(
        "SELECT u.email::text FROM org_members m JOIN users u ON u.id = m.user_id \
         WHERE m.org_id = $1 AND m.role = 'owner' ORDER BY m.created_at LIMIT 1",
    )
    .bind(org)
    .fetch_optional(&state.db)
    .await;

    let to = match to {
        Ok(Some(to)) => to,
        Ok(None) => {
            tracing::warn!(%org, subject, "organisation sans propriétaire : e-mail non envoyé");
            return;
        }
        Err(e) => {
            tracing::error!(%org, error = ?e, "propriétaire introuvable : e-mail non envoyé");
            return;
        }
    };

    if let Err(e) = crate::email::send(state, &to, subject, heading, paragraphs, cta).await {
        tracing::error!(%org, subject, error = ?e, "e-mail de facturation non envoyé");
    }
}

/// `charge.dispute.created` — une contestation bancaire. **On ne coupe pas l'accès** :
/// c'est le plus souvent un client qui ne reconnaît pas un libellé sur son relevé, et
/// couper son service au moment où il doute est le meilleur moyen de perdre le litige
/// ET le client. En revanche une contestation ignorée est perdue d'office (Stripe donne
/// une semaine pour répondre), donc l'exploitant doit la voir tout de suite.
///
/// L'objet `dispute` ne porte pas de `customer` : on ne remonte pas à l'organisation
/// sans un aller-retour supplémentaire chez Stripe. L'identifiant du litige suffit à le
/// retrouver dans le tableau de bord, qui est de toute façon l'endroit où l'on répond.
pub async fn dispute_opened(state: &AppState, id: &str, amount_cents: i64, reason: &str) {
    tracing::warn!(
        dispute = id,
        amount_cents,
        reason,
        "contestation bancaire ouverte — répondre sous 7 jours"
    );

    // ponytail : pas de variable d'environnement de plus pour une adresse d'exploitant.
    // `BREVO_FROM` est notre propre boîte ; c'est là que ça doit arriver.
    let Some(to) = state.cfg.brevo.as_ref().map(|r| r.from.clone()) else {
        return;
    };
    let amount = format!("{:.2} €", amount_cents as f64 / 100.0);
    if let Err(e) = crate::email::send(
        state,
        &to,
        &format!("Litige Stripe ouvert ({amount})"),
        "Contestation bancaire à traiter",
        &[
            format!(
                "Un client conteste un prélèvement de {amount}. Motif déclaré : {reason}. \
                 Litige {id}."
            ),
            "Stripe laisse environ 7 jours pour fournir les preuves. Sans réponse, le litige \
             est perdu d'office et les frais restent dus. L'accès du client n'a pas été coupé."
                .to_string(),
        ],
        Some((
            "Ouvrir le tableau de bord Stripe",
            "https://dashboard.stripe.com/disputes",
        )),
    )
    .await
    {
        tracing::error!(dispute = id, error = ?e, "alerte de litige non envoyée");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn org(plan: &str, status: Option<&str>, grace: Option<DateTime<Utc>>) -> OrgBilling {
        OrgBilling {
            plan: plan.into(),
            subscription_status: status.map(Into::into),
            grace_until: grace,
        }
    }

    fn t0() -> DateTime<Utc> {
        DateTime::from_timestamp(1_700_000_000, 0).unwrap()
    }

    /// La règle qui fait perdre un client si elle est fausse dans un sens, et de l'argent
    /// si elle est fausse dans l'autre.
    #[test]
    fn acces_complet_pendant_la_grace_puis_free() {
        let now = t0();
        let en_grace = org("pro", Some("past_due"), Some(now + Duration::days(3)));
        assert_eq!(effective_plan_at(&en_grace, now).id, "pro");

        // dernière seconde de la grâce : toujours Pro
        let extremis = org("pro", Some("past_due"), Some(now + Duration::seconds(1)));
        assert_eq!(effective_plan_at(&extremis, now).id, "pro");

        // échue : Free, même si la colonne `plan` n'a pas encore été basculée par le
        // balayage. La lecture ne dépend jamais d'une tâche de fond.
        let echue = org("team", Some("past_due"), Some(now - Duration::seconds(1)));
        assert_eq!(effective_plan_at(&echue, now).id, "free");

        // impayé sans fenêtre ouverte (bascule déjà faite) : Free
        assert_eq!(
            effective_plan_at(&org("pro", Some("unpaid"), None), now).id,
            "free"
        );
    }

    #[test]
    fn transitions_de_la_machine_a_etats() {
        let now = t0();
        let grace = Some(now + Duration::days(2));
        let cas = [
            // (statut, grâce ouverte, plan effectif)
            (None, false, "pro"), // jamais facturé / compte offert
            (Some("active"), false, "pro"),
            (Some("trialing"), false, "pro"),
            (Some("past_due"), true, "pro"), // grâce : accès complet
            (Some("past_due"), false, "free"),
            (Some("unpaid"), true, "pro"), // Stripe a abandonné, pas nous
            (Some("unpaid"), false, "free"),
            (Some("incomplete"), false, "free"), // 3-D Secure jamais confirmé
            (Some("paused"), false, "free"),
            (Some("canceled"), false, "free"),
            (Some("incomplete_expired"), false, "free"),
            // statut inventé par une future version de l'API : moins de droits, jamais plus
            (Some("something_new"), false, "free"),
        ];
        for (status, ouverte, attendu) in cas {
            let o = org("pro", status, ouverte.then_some(grace).flatten());
            assert_eq!(
                effective_plan_at(&o, now).id,
                attendu,
                "statut {status:?}, grâce ouverte = {ouverte}"
            );
        }
    }

    /// Le rejeu est la garantie que la relance ne part qu'une fois : le compteur persisté
    /// est relu inchangé, `due_dunning_stage` répond `None`, aucun e-mail ne part.
    #[test]
    fn une_relance_ne_part_quune_fois() {
        let start = t0();
        let fin = start + Duration::days(GRACE_DAYS);
        let at = |d: i64, sent: i32| due_dunning_stage(fin, start + Duration::days(d), sent);

        assert_eq!(at(0, 0), None, "J+0 : Stripe vient de refuser, on attend");
        assert_eq!(at(1, 0), Some(1));
        assert_eq!(at(1, 1), None, "webhook rejoué le même jour");
        assert_eq!(at(2, 1), None);
        assert_eq!(at(3, 1), Some(2));
        assert_eq!(at(3, 2), None, "webhook rejoué");
        assert_eq!(at(6, 2), Some(3));
        assert_eq!(at(6, 3), None, "webhook rejoué");
        assert_eq!(at(7, 3), None, "plus rien à envoyer : c'est la bascule");

        // balayage qui a sauté plusieurs jours : on rattrape la dernière étape, pas les
        // trois d'un coup — trois e-mails simultanés ne rendent pas un client solvable.
        assert_eq!(at(6, 0), Some(3));
    }
}
