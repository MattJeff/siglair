//! Croissance — la boucle virale et sa mesure (contrat §11).
//!
//! Trois règles tiennent ce fichier, et elles ne se négocient pas :
//!
//! 1. **Mesurer ne casse jamais ce qu'on mesure.** `record` ne renvoie pas d'erreur : une
//!    écriture de statistique qui fait échouer une publication ou un paiement transforme un
//!    tableau de bord en incident client. En cas d'échec, un `warn` et on continue.
//! 2. **Aucun cookie sur le destinataire** (§11.3). Il n'est pas notre utilisateur, il n'a
//!    rien accepté ; l'attribution passe par l'URL. Rien ici n'écrit de cookie, et rien ne
//!    doit en écrire un plus tard.
//! 3. **Ni IP en clair, ni User-Agent brut** (§4.1). Le hachage est fait *ici*, pas chez
//!    l'appelant : c'est le seul moyen de garantir qu'aucun chemin ne l'oublie.
//!
//! Sur un chemin de réponse (image publique, redirection, webhook Stripe), l'appelant
//! détache l'écriture — comme `routes::public` le fait déjà pour `events` :
//!
//! ```ignore
//! let (db, salt) = (st.db.clone(), st.cfg.ip_salt.clone());
//! tokio::spawn(async move {
//!     growth::record(&db, org_id, None, Some(sig_id), GrowthEvent::PoweredByClick,
//!                    json!({}), Visitor { ip: ip.as_deref(), ua: ua.as_deref(), salt: &salt }).await;
//! });
//! ```

use serde::Serialize;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    error::Result,
    util::{hash_ip, ua_family},
};

/// Les six événements du contrat §11.1. **Pas un de plus.** Chaque événement est une donnée
/// à conserver, à sécuriser, à purger sur demande RGPD et à justifier ; quarante événements
/// ne valent pas mieux que six, ils les noient. Un septième demande d'abord la question
/// précise à laquelle il répond, puis une migration — la liste est fermée par un CHECK.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GrowthEvent {
    /// Signature créée puis publiée : combien arrivent au bout de l'éditeur ?
    SignatureGenerated,
    /// Première ouverture depuis une IP différente de celle du propriétaire.
    SignatureInstalled,
    /// Clic sur le badge d'une signature Free : combien de destinataires mordent ?
    PoweredByClick,
    /// Session Stripe créée : combien atteignent la caisse ?
    UpgradeStarted,
    /// Abonnement actif : combien paient ?
    UpgradeCompleted,
    /// Invitation envoyée : la boucle interne d'une organisation tourne-t-elle ?
    TeamInvite,
}

impl GrowthEvent {
    pub const ALL: [GrowthEvent; 6] = [
        Self::SignatureGenerated,
        Self::SignatureInstalled,
        Self::PoweredByClick,
        Self::UpgradeStarted,
        Self::UpgradeCompleted,
        Self::TeamInvite,
    ];

    /// La valeur exacte de la colonne `kind` : ces chaînes sont celles du CHECK de la
    /// migration 0007, un écart donne une violation de contrainte à l'exécution.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SignatureGenerated => "signature_generated",
            Self::SignatureInstalled => "signature_installed",
            Self::PoweredByClick => "powered_by_click",
            Self::UpgradeStarted => "upgrade_started",
            Self::UpgradeCompleted => "upgrade_completed",
            Self::TeamInvite => "team_invite",
        }
    }
}

/// Ce qu'on sait du réseau au moment de l'événement, avant hachage.
///
/// Regroupé en une structure pour deux raisons : le sel voyage avec l'IP (sans lui on ne
/// peut pas hacher, et une IP en clair finirait en base), et sept paramètres suffisent.
/// Quand rien n'est connu — un webhook Stripe, une tâche de fond — `Visitor::unknown(salt)`.
#[derive(Clone, Copy)]
pub struct Visitor<'a> {
    /// IP client brute. Hachée par `record`, jamais stockée telle quelle.
    pub ip: Option<&'a str>,
    /// User-Agent brut. Réduit à une famille par `record`, jamais stocké tel quel.
    pub ua: Option<&'a str>,
    /// `SIGLAIR_IP_SALT` (`st.cfg.ip_salt`).
    pub salt: &'a str,
}

impl<'a> Visitor<'a> {
    pub fn unknown(salt: &'a str) -> Self {
        Self {
            ip: None,
            ua: None,
            salt,
        }
    }
}

/// Insertion normale. `ON CONFLICT DO NOTHING` couvre l'index unique partiel des
/// installations sans que l'appelant ait à savoir lequel des six kinds y est soumis.
const INSERT_SQL: &str = "\
    INSERT INTO growth_events (org_id, user_id, signature_id, kind, props, ip_hash, ua_family) \
    SELECT $1::uuid, $2::uuid, $3::uuid, $4::text, $5::jsonb, $6::bytea, $7::text \
    ON CONFLICT DO NOTHING";

/// Les deux événements produits par un **destinataire** d'e-mail passent par
/// `orgs.analytics_enabled` (§4.1 : le champ doit permettre de tout couper). Le filtre est
/// dans le SQL et pas chez l'appelant : un chemin qui oublierait de le lire écrirait quand
/// même une donnée qu'une org a explicitement refusée.
const INSERT_IF_ANALYTICS_SQL: &str = "\
    INSERT INTO growth_events (org_id, user_id, signature_id, kind, props, ip_hash, ua_family) \
    SELECT $1::uuid, $2::uuid, $3::uuid, $4::text, $5::jsonb, $6::bytea, $7::text \
    WHERE EXISTS (SELECT 1 FROM orgs o WHERE o.id = $1::uuid AND o.analytics_enabled) \
    ON CONFLICT DO NOTHING";

/// `signature_installed` : une seule ligne par signature, à la première ouverture qui ne
/// vient pas d'une IP connue du propriétaire.
///
/// « IP du propriétaire » = les `ip_hash` de ses sessions (`sessions.ip_hash`, migration
/// 0001). C'est la seule chose que la base sait déjà de son réseau, et ça évite d'inventer
/// une colonne pour la circonstance.
///
/// **C'est une approximation, et voilà exactement en quoi :**
/// - une session expirée puis purgée efface l'IP connue — une ouverture du propriétaire
///   depuis ce même réseau comptera alors pour une installation ;
/// - le propriétaire qui relit son propre e-mail en 4G ou depuis chez lui compte pour une
///   installation : ce n'est pas l'IP de sa session ;
/// - le proxy d'images de Gmail et Apple Mail Privacy Protection préchargent depuis leurs
///   propres serveurs, donc depuis une IP étrangère à coup sûr. L'événement dit donc en
///   réalité « la signature a été chargée hors du navigateur du propriétaire », ce qui est
///   plus faible que « quelqu'un d'autre l'a lue » ;
/// - à l'inverse, un vrai destinataire derrière le même NAT (même bureau, même VPN) ne
///   compte pas : faux négatif silencieux.
///
/// C'est donc un indicateur de tendance, comme les ouvertures (§11.1), pas un compteur
/// exact d'installations. Le tableau de bord doit le dire plutôt que laisser croire
/// l'inverse. La version exacte demanderait un signal explicite (un « j'ai collé ma
/// signature » dans l'app) : à faire le jour où la décision en dépend, pas avant.
const INSERT_INSTALL_SQL: &str = "\
    INSERT INTO growth_events (org_id, user_id, signature_id, kind, props, ip_hash, ua_family) \
    SELECT $1::uuid, $2::uuid, $3::uuid, $4::text, $5::jsonb, $6::bytea, $7::text \
    WHERE $3::uuid IS NOT NULL AND $6::bytea IS NOT NULL \
      AND EXISTS (SELECT 1 FROM orgs o WHERE o.id = $1::uuid AND o.analytics_enabled) \
      AND NOT EXISTS (SELECT 1 FROM signatures sg \
                        JOIN sessions se ON se.user_id = sg.owner_user_id \
                       WHERE sg.id = $3::uuid AND se.ip_hash = $6::bytea) \
    ON CONFLICT DO NOTHING";

fn statement(kind: GrowthEvent) -> &'static str {
    match kind {
        GrowthEvent::SignatureInstalled => INSERT_INSTALL_SQL,
        GrowthEvent::PoweredByClick => INSERT_IF_ANALYTICS_SQL,
        _ => INSERT_SQL,
    }
}

/// Enregistre un événement de croissance. **Ne renvoie jamais d'erreur** : c'est le point
/// entier de la fonction (règle 1 du module). Une base injoignable, une contrainte violée,
/// un `props` invalide — tout finit en `warn` et l'action mesurée continue.
///
/// `props` : `serde_json::json!({})` quand il n'y a rien à dire. `Value::Null` est
/// normalisé en objet vide, la colonne est `NOT NULL`.
pub async fn record(
    db: &PgPool,
    org_id: Uuid,
    user_id: Option<Uuid>,
    signature_id: Option<Uuid>,
    kind: GrowthEvent,
    props: Value,
    visitor: Visitor<'_>,
) {
    // §4.1 : le hachage se fait ici. Aucun appelant n'a de raison de manipuler une IP brute.
    let ip_hash = visitor.ip.map(|ip| hash_ip(ip, visitor.salt));
    let family = visitor.ua.map(ua_family);
    let props = if props.is_null() { json!({}) } else { props };

    let res = sqlx::query(statement(kind))
        .bind(org_id)
        .bind(user_id)
        .bind(signature_id)
        .bind(kind.as_str())
        .bind(props)
        .bind(ip_hash)
        .bind(family)
        .execute(db)
        .await;

    if let Err(e) = res {
        tracing::warn!(error = ?e, kind = kind.as_str(), %org_id, "événement de croissance non enregistré");
    }
}

/// L'entonnoir viral d'une org sur une fenêtre glissante (§11.4).
///
/// Ordre d'affichage imposé par le contrat : clics et conversions en gros, `views` en petit
/// et grisé, avec la mention de son imprécision — Apple MPP et le proxy Gmail gonflent les
/// ouvertures, et mettre en avant le chiffre le moins fiable du tableau, c'est vendre un
/// chiffre faux.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, sqlx::FromRow)]
pub struct Funnel {
    /// Ouvertures des signatures de l'org. Indicatif seulement.
    pub views: i64,
    /// Clics sur le badge « Powered by siglair.com ».
    pub badge_clicks: i64,
    /// Comptes créés attribués à une signature de l'org.
    pub signups: i64,
    /// Parmi eux, ceux qui appartiennent aujourd'hui à une org payante.
    pub paid: i64,
}

impl Funnel {
    /// clic / vue. Indicatif : le dénominateur est le chiffre le moins fiable du tableau.
    pub fn click_rate(&self) -> Option<f64> {
        rate(self.badge_clicks, self.views)
    }
    /// création / clic.
    pub fn signup_rate(&self) -> Option<f64> {
        rate(self.signups, self.badge_clicks)
    }
    /// payant / création. C'est celui-ci qui décide si le canal vaut quelque chose.
    pub fn paid_rate(&self) -> Option<f64> {
        rate(self.paid, self.signups)
    }
}

/// `None` plutôt que `0.0` quand il n'y a rien à diviser : « 0 % » et « pas encore de
/// donnée » ne se disent pas pareil à un utilisateur qui décide d'un budget.
fn rate(num: i64, den: i64) -> Option<f64> {
    (den > 0).then(|| num as f64 / den as f64)
}

/// Une seule requête, quatre agrégats. Une boucle Rust ferait quatre allers-retours pour
/// afficher une ligne de texte.
const FUNNEL_SQL: &str = "\
    WITH win AS (SELECT now() - make_interval(days => $2::int) AS since), \
    referred AS ( \
        SELECT u.id FROM users u \
          JOIN signatures s ON s.id = u.referred_by_signature_id \
         WHERE s.org_id = $1::uuid AND u.created_at >= (SELECT since FROM win)) \
    SELECT \
      (SELECT count(*) FROM events e JOIN signatures s2 ON s2.id = e.signature_id \
        WHERE s2.org_id = $1::uuid AND e.kind = 'open' \
          AND e.occurred_at >= (SELECT since FROM win)) AS views, \
      (SELECT count(*) FROM growth_events g \
        WHERE g.org_id = $1::uuid AND g.kind = 'powered_by_click' \
          AND g.occurred_at >= (SELECT since FROM win)) AS badge_clicks, \
      (SELECT count(*) FROM referred) AS signups, \
      (SELECT count(*) FROM referred r WHERE EXISTS ( \
          SELECT 1 FROM org_members m JOIN orgs o ON o.id = m.org_id \
           WHERE m.user_id = r.id AND o.plan <> 'free')) AS paid";

/// Entonnoir de l'org sur `days` jours.
///
/// Renvoie `Result` — contrairement à `record` — parce qu'ici l'erreur se voit : afficher
/// « 0 vue, 0 clic, 0 compte » sur une panne de base ferait conclure à l'utilisateur que le
/// canal est mort. Mieux vaut une erreur affichée qu'un zéro mensonger.
pub async fn funnel(db: &PgPool, org_id: Uuid, days: i32) -> Result<Funnel> {
    // La fenêtre vient d'un plan (§6 : 30 j en Pro, 365 en Team) ou d'une query string.
    // Bornée ici plutôt que validée chez chaque appelant.
    let days = days.clamp(1, 3650);
    Ok(sqlx::query_as(FUNNEL_SQL)
        .bind(org_id)
        .bind(days)
        .fetch_one(db)
        .await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    const MIGRATION: &str = include_str!("../migrations/0007_growth.sql");

    /// §11.1 : six événements, pas un de plus — et les mêmes des deux côtés. Un `kind`
    /// absent du CHECK échouerait en production, dans une tâche détachée, en silence.
    #[test]
    fn six_evenements_identiques_dans_lenum_et_dans_le_check() {
        let list = MIGRATION
            .split_once("kind IN (")
            .and_then(|(_, rest)| rest.split_once(')'))
            .expect("le CHECK de growth_events")
            .0;
        let in_sql: Vec<&str> = list.split('\'').skip(1).step_by(2).collect();
        let in_rust: Vec<&str> = GrowthEvent::ALL.iter().map(|k| k.as_str()).collect();

        assert_eq!(in_rust.len(), 6);
        assert_eq!(in_sql, in_rust, "l'enum et le CHECK ont divergé");
        // le sérialiseur JSON parle la même langue que la colonne
        for k in GrowthEvent::ALL {
            assert_eq!(serde_json::to_value(k).unwrap(), json!(k.as_str()));
        }
    }

    /// La déduplication de `signature_installed` a deux couches, et il faut les deux :
    /// la condition SQL décide *quand* la première ligne apparaît, l'index unique partiel
    /// garantit qu'il n'y en a *qu'une* même si trois préchargements Gmail arrivent en
    /// parallèle dans trois tâches détachées.
    ///
    /// ponytail : test de chaîne, faute de base en CI (le workflow n'a pas de service
    /// Postgres). Il ne prouve pas le comportement, il empêche la suppression silencieuse
    /// d'une des deux couches — c'est le seul garde-fou disponible ici.
    #[test]
    fn linstallation_est_dedupliquee_par_signature() {
        let sql = statement(GrowthEvent::SignatureInstalled);
        assert!(sql.contains("ON CONFLICT DO NOTHING"));
        // pas d'IP du propriétaire parmi celles de ses sessions
        assert!(sql.contains("NOT EXISTS"));
        assert!(sql.contains("se.ip_hash = $6::bytea"));
        // sans signature ni IP, l'événement n'a aucun sens : rien n'est inséré
        assert!(sql.contains("$3::uuid IS NOT NULL AND $6::bytea IS NOT NULL"));
        assert!(
            MIGRATION.contains(
                "CREATE UNIQUE INDEX growth_events_install_once_idx\n    \
                 ON growth_events (signature_id) WHERE kind = 'signature_installed'"
            ),
            "l'unicité doit rester garantie par la base"
        );

        // Les événements produits par un destinataire respectent orgs.analytics_enabled ;
        // ceux produits par nos propres utilisateurs n'ont pas à en dépendre.
        for k in [GrowthEvent::SignatureInstalled, GrowthEvent::PoweredByClick] {
            assert!(statement(k).contains("o.analytics_enabled"), "{k:?}");
        }
        for k in [
            GrowthEvent::SignatureGenerated,
            GrowthEvent::UpgradeStarted,
            GrowthEvent::UpgradeCompleted,
            GrowthEvent::TeamInvite,
        ] {
            assert!(!statement(k).contains("analytics_enabled"), "{k:?}");
        }
    }

    /// Le point entier de `record` : une base injoignable ne remonte rien à l'appelant.
    /// Si un jour cette fonction rend un `Result`, ce test cesse de compiler — c'est voulu.
    #[tokio::test]
    async fn record_ne_fait_jamais_echouer_laction_mesuree() {
        // port fermé : chaque appel échoue à se connecter, aussi vite que possible
        let db = sqlx::postgres::PgPoolOptions::new()
            .acquire_timeout(Duration::from_millis(50))
            .connect_lazy("postgres://siglair:siglair@127.0.0.1:1/absent")
            .expect("pool paresseux");

        for kind in GrowthEvent::ALL {
            // annotation volontaire : c'est la signature qui est testée, pas la valeur
            let _: () = record(
                &db,
                Uuid::new_v4(),
                Some(Uuid::new_v4()),
                Some(Uuid::new_v4()),
                kind,
                Value::Null, // normalisé en {} : la colonne est NOT NULL
                Visitor {
                    ip: Some("203.0.113.7"),
                    ua: Some("Mozilla/5.0 (via ggpht.com GoogleImageProxy)"),
                    salt: "sel",
                },
            )
            .await;
        }
    }

    /// §11.4 : les trois taux affichés, et surtout leurs bords. « 0 % » affiché à la place
    /// de « pas encore de donnée » ferait couper un canal qui n'a simplement pas démarré.
    #[test]
    fn lentonnoir_calcule_ses_taux_et_refuse_de_diviser_par_zero() {
        let f = Funnel {
            views: 870,
            badge_clicks: 14,
            signups: 3,
            paid: 1,
        };
        assert!((f.click_rate().unwrap() - 14.0 / 870.0).abs() < 1e-12);
        assert!((f.signup_rate().unwrap() - 3.0 / 14.0).abs() < 1e-12);
        assert!((f.paid_rate().unwrap() - 1.0 / 3.0).abs() < 1e-12);

        let vide = Funnel::default();
        assert_eq!(vide.click_rate(), None);
        assert_eq!(vide.signup_rate(), None);
        assert_eq!(vide.paid_rate(), None);

        // des vues sans clic : un vrai 0 %, pas une absence de donnée
        let vues_seules = Funnel {
            views: 100,
            ..Funnel::default()
        };
        assert_eq!(vues_seules.click_rate(), Some(0.0));
        assert_eq!(vues_seules.signup_rate(), None);

        // la fenêtre est bornée avant d'atteindre le SQL
        assert!(FUNNEL_SQL.contains("make_interval(days => $2::int)"));
        assert_eq!(0i32.clamp(1, 3650), 1);
        assert_eq!(i32::MAX.clamp(1, 3650), 3650);
    }
}
