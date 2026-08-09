//! Plans et quotas — contrat §6. Source unique : le front lit ces limites via `/api/me`
//! et ne code jamais un quota en dur.

use serde::Serialize;

use crate::error::{AppError, Result};

/// Les limites d'un plan, telles que le front les consomme.
///
/// La forme sérialisée est celle attendue par `web/src/lib/types.ts` (`Limits`). Ce n'est
/// pas une coquetterie : `tsc` ne type pas une réponse réseau, donc un désaccord de nom
/// entre Rust et TypeScript ne se voit ni à la compilation, ni aux tests — seulement à
/// l'écran, sous la forme d'un tarif vide. C'est exactement ce qui s'était produit ici.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Limits {
    /// `None` = illimité.
    pub signatures: Option<u32>,
    pub assets_bytes: u64,
    /// `0` = pas d'analytics.
    pub analytics_days: u32,
    pub hosted_gif: bool,
    pub org_templates: bool,
    /// Marque Siglair imposée dans l'export.
    pub branding: bool,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Plan {
    /// Identifiant technique : `free` | `pro` | `team`. Sérialisé en `plan`.
    #[serde(rename = "plan")]
    pub id: &'static str,
    /// Nom affiché. Il vit ici pour que le front n'ait pas à traduire un identifiant.
    pub name: &'static str,
    /// Prix mensuel en euros, contrat §6. **Seul endroit du dépôt où un prix est écrit.**
    pub price_eur_month: f32,
    /// `true` = prix par membre (Team), `false` = prix par organisation.
    pub per_seat: bool,
    /// Nombre de sièges minimum facturés.
    pub min_seats: u32,
    pub limits: Limits,
    /// Générations IA (§6bis.6). `None` = illimité — aucun plan ne l'est aujourd'hui.
    #[serde(skip)]
    pub ai_generations: Option<u32>,
    /// `false` = quota à vie (Free), `true` = remis à zéro chaque mois (Pro, Team).
    #[serde(skip)]
    pub ai_generations_monthly: bool,
}

impl Plan {
    /// Prix annuel : deux mois offerts (contrat §6). Dérivé, jamais saisi deux fois.
    pub fn price_eur_year(&self) -> f32 {
        (self.price_eur_month * 10.0 * 100.0).round() / 100.0
    }

    /// Ce que coûte réellement le plan à `seats` membres, sièges minimum appliqués.
    pub fn monthly_total(&self, seats: u32) -> f32 {
        let billed = seats.max(self.min_seats);
        let n = if self.per_seat { billed as f32 } else { 1.0 };
        (self.price_eur_month * n * 100.0).round() / 100.0
    }
}

pub const FREE: Plan = Plan {
    id: "free",
    name: "Free",
    price_eur_month: 0.0,
    per_seat: false,
    min_seats: 1,
    limits: Limits {
        signatures: Some(1),
        assets_bytes: 10 * 1024 * 1024,
        analytics_days: 0,
        hosted_gif: false,
        org_templates: false,
        branding: true,
    },
    // §6bis.6 : 1 génération AU TOTAL, pas par mois — c'est l'essai, pas un quota récurrent.
    ai_generations: Some(1),
    ai_generations_monthly: false,
};

pub const PRO: Plan = Plan {
    id: "pro",
    name: "Pro",
    price_eur_month: 7.90,
    per_seat: false,
    min_seats: 1,
    limits: Limits {
        // Contrat §6 : « Signatures : Pro = illimité ». Le verrou Free → Pro est le GIF
        // hébergé, pas un compte de signatures.
        signatures: None,
        assets_bytes: 500 * 1024 * 1024,
        analytics_days: 30,
        hosted_gif: true,
        org_templates: false,
        branding: false,
    },
    ai_generations: Some(30),
    ai_generations_monthly: true,
};

pub const TEAM: Plan = Plan {
    id: "team",
    name: "Team",
    price_eur_month: 5.90,
    per_seat: true,
    // Contrat §6 : minimum 3 sièges. Entrée à 17,70 €, et la facture suit la taille du
    // client — un forfait unique ferait payer une agence de 40 personnes comme un trio.
    min_seats: 3,
    limits: Limits {
        signatures: None,
        assets_bytes: 5 * 1024 * 1024 * 1024,
        analytics_days: 365,
        hosted_gif: true,
        org_templates: true,
        branding: false,
    },
    ai_generations: Some(100),
    ai_generations_monthly: true,
};

pub const ALL: [Plan; 3] = [FREE, PRO, TEAM];

impl Plan {
    /// Un identifiant inconnu retombe sur Free : moins de droits, jamais plus.
    pub fn get(id: &str) -> Plan {
        match id {
            "pro" => PRO,
            "team" => TEAM,
            _ => FREE,
        }
    }
}

/// Vérifié côté serveur à l'écriture — un bouton grisé n'est pas un quota (contrat §6).
pub fn check_signature_quota(plan: &Plan, current: u32) -> Result<()> {
    let Some(max) = plan.limits.signatures else {
        return Ok(());
    };
    if current < max {
        return Ok(());
    }
    // Seul Free est plafonné (§6) : les deux plans payants ont `max_signatures: None` et
    // n'atteignent jamais cette ligne.
    Err(AppError::QuotaExceeded(
        "Le plan Free est limité à 1 signature. Passez à Pro (7,90 €/mois) pour créer autant \
         de signatures que vous voulez, héberger vos GIF animés et suivre vos ouvertures."
            .to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verrouille la FORME SÉRIALISÉE, pas seulement les valeurs.
    ///
    /// `tsc` ne type pas une réponse réseau : un désaccord de nom entre ce `Serialize` et
    /// `web/src/lib/types.ts` ne se voit ni à la compilation Rust, ni à la compilation
    /// TypeScript, ni aux tests des deux côtés — uniquement à l'écran, sous la forme d'une
    /// page Tarifs vide. C'est arrivé une fois ; ce test est là pour que ça n'arrive plus.
    ///
    /// Si ce test casse, la question n'est pas « comment le faire passer » : c'est
    /// « est-ce que web/src/lib/types.ts a été mis à jour dans le même commit ».
    #[test]
    fn serialized_shape_matches_the_frontend_contract() {
        let v = serde_json::to_value(PRO).expect("Plan sérialisable");
        let obj = v.as_object().expect("objet JSON");

        let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
        keys.sort_unstable();
        // Exactement les champs de `interface PlanInfo`. Ni plus, ni moins : un champ en
        // trop côté serveur est une fuite d'information interne, un champ manquant est un
        // trou à l'affichage.
        assert_eq!(
            keys,
            vec![
                "limits",
                "min_seats",
                "name",
                "per_seat",
                "plan",
                "price_eur_month"
            ],
            "la forme de Plan a changé sans que web/src/lib/types.ts suive"
        );

        let mut lk: Vec<&str> = obj["limits"]
            .as_object()
            .expect("limits est un objet")
            .keys()
            .map(String::as_str)
            .collect();
        lk.sort_unstable();
        assert_eq!(
            lk,
            vec![
                "analytics_days",
                "assets_bytes",
                "branding",
                "hosted_gif",
                "org_templates",
                "signatures"
            ],
            "la forme de Limits a changé sans que web/src/lib/types.ts suive"
        );

        // Les quotas IA ne sortent pas : le front ne les affiche pas, et une limite non
        // affichée exposée au client est une information interne de trop.
        assert!(!obj.contains_key("ai_generations"));
    }

    /// Les prix du contrat §6, et le fait qu'ils ne vivent qu'ici.
    #[test]
    fn pricing_matches_the_contract() {
        assert_eq!(FREE.price_eur_month, 0.0);
        assert_eq!(PRO.price_eur_month, 7.90);
        assert_eq!(TEAM.price_eur_month, 5.90);

        // Team : par siège, minimum 3. C'est ce qui fait suivre la facture à la taille du
        // client au lieu de la plafonner.
        // Via ALL : sur les consts directement, clippy replie l'assertion à la compilation
        // et elle ne teste plus rien.
        assert_eq!(
            ALL.iter()
                .map(|p| (p.per_seat, p.min_seats))
                .collect::<Vec<_>>(),
            vec![(false, 1), (false, 1), (true, 3)]
        );
        assert_eq!(TEAM.monthly_total(1), 17.70, "sièges minimum non appliqués");
        assert_eq!(
            TEAM.monthly_total(40),
            236.0,
            "une agence de 40 doit payer 40 sièges"
        );
        assert_eq!(PRO.monthly_total(9), 7.90, "Pro n'est pas par siège");

        // Deux mois offerts sur l'annuel, dérivés — jamais saisis une seconde fois.
        assert_eq!(PRO.price_eur_year(), 79.0);
        assert_eq!(TEAM.price_eur_year(), 59.0);
    }

    #[test]
    fn quotas() {
        assert!(check_signature_quota(&FREE, 0).is_ok());
        assert!(matches!(
            check_signature_quota(&FREE, 1),
            Err(AppError::QuotaExceeded(_))
        ));
        // §6 : Pro et Team sont illimités en signatures — le verrou Free → Pro est le GIF
        // hébergé (`hosted_gif`), pas un compte de signatures.
        assert!(check_signature_quota(&PRO, 10_000).is_ok());
        // le verrou Free → Pro est le GIF hébergé (via ALL : clippy replierait un && de consts)
        assert_eq!(
            ALL.iter().map(|p| p.limits.hosted_gif).collect::<Vec<_>>(),
            vec![false, true, true]
        );
        assert!(check_signature_quota(&TEAM, 10_000).is_ok());
        // plan inconnu (colonne corrompue, nouveau plan pas encore déployé) = Free
        assert_eq!(Plan::get("enterprise").id, "free");
    }

    /// §6bis.6 : Free = 1 AU TOTAL, Pro = 30/mois, Team = 100/mois. La vérification, elle,
    /// est l'`UPDATE ... WHERE used < max` de `routes::onboarding::consume_ai_generation` :
    /// en deux requêtes, un double-clic consommerait deux fois la génération gratuite.
    #[test]
    fn quota_ia() {
        assert_eq!(
            ALL.iter()
                .map(|p| (p.ai_generations, p.ai_generations_monthly))
                .collect::<Vec<_>>(),
            vec![(Some(1), false), (Some(30), true), (Some(100), true)]
        );
    }
}
