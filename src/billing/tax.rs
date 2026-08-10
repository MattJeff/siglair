//! TVA et mentions de facture — l'éditeur est **KAIROS, SAS française**, qui vend en euros
//! à des **professionnels de l'Union européenne**. Ce n'est pas un réglage : ça détermine
//! le taux, les mentions obligatoires de la facture et l'absence de droit de rétractation.
//!
//! Les trois régimes, dans l'ordre où Stripe Tax les applique une fois le pays connu :
//!
//! | Preneur                                        | Taux                  | Mention |
//! |------------------------------------------------|-----------------------|---------|
//! | France                                          | TVA française 20 %    | — |
//! | Assujetti UE hors France, n° de TVA **valide**  | 0 %, autoliquidation  | « Autoliquidation — article 283-2 du CGI » |
//! | UE hors France **sans** n° de TVA valide        | TVA du pays du preneur (OSS) | — |
//! | Hors UE                                         | hors champ            | — |
//!
//! Rien de tout cela n'est calculé ici : le calcul est celui de Stripe Tax, qui connaît les
//! 27 taux et les met à jour. Notre travail est (1) de lui donner ce qu'il lui faut pour
//! déterminer le pays et le statut du preneur, (2) de porter sur la facture les mentions que
//! la loi française exige et que Stripe ne connaît pas.
//!
//! Deux pièges tenus ici une fois pour toutes :
//! - `automatic_tax` sans `customer_update[address]` échoue à la validation : Stripe ne
//!   s'autorise pas à écrire l'adresse collectée sur un client existant, donc il n'a aucun
//!   pays, donc il refuse la session ;
//! - `automatic_tax` échoue aussi tant que Stripe Tax n'est pas activé dans le tableau de
//!   bord. C'est une configuration de compte, pas une panne : voir [`is_tax_not_configured`].

use crate::error::AppError;

/// Champs de TVA à joindre à la création d'une Checkout Session.
///
/// Ordre sans importance pour Stripe ; groupés par intention pour la relecture.
pub fn checkout_fields() -> Vec<(&'static str, String)> {
    vec![
        // Le taux, calculé par Stripe Tax d'après le pays et le n° de TVA du preneur.
        ("automatic_tax[enabled]", "true".into()),
        // Collecte du n° de TVA intracommunautaire : sans lui, pas d'autoliquidation
        // possible, et un client belge se voit facturer la TVA belge au lieu de 0 %.
        ("tax_id_collection[enabled]", "true".into()),
        // Le pays conditionne le taux : on ne peut pas le déduire d'une carte bancaire.
        ("billing_address_collection", "required".into()),
        // Sans ces deux-là, `automatic_tax` est refusé sur un client déjà créé (cf. en-tête).
        ("customer_update[address]", "auto".into()),
        ("customer_update[name]", "auto".into()),
        // Une ligne aujourd'hui, zéro redéploiement le jour d'une remise.
        ("allow_promotion_codes", "true".into()),
    ]
}

/// Champs de facturation à joindre à la création du **client** Stripe.
///
/// Le pied de page vit sur le client et non sur la session : en `mode=subscription`, Stripe
/// émet lui-même les factures de chaque échéance et leur applique
/// `customer.invoice_settings.footer`. (`invoice_creation[enabled]` n'existe QUE pour
/// `mode=payment` — le passer sur un abonnement fait échouer la création de session.)
pub fn customer_invoice_fields() -> Vec<(&'static str, String)> {
    vec![("invoice_settings[footer]", invoice_footer())]
}

/// Identité de l'éditeur, surchargeable par l'environnement.
///
/// Valeurs par défaut = celles de `web/src/pages/Legal.tsx`, pour qu'une facture ne parte
/// jamais anonyme si la variable manque. La clé du n° de TVA est vérifiée par un test.
fn legal(key: &str, default: &str) -> String {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| default.to_string())
}

/// Pied de page porté par toutes les factures.
///
/// Il est **statique** : les trois régimes ci-dessus sont formulés de façon à rester exacts
/// quel que soit celui que Stripe Tax retient, parce que le taux n'est connu qu'à l'émission
/// de la facture, bien après la création du client. La mention d'autoliquidation est donc
/// conditionnée dans sa propre phrase plutôt qu'affirmée.
pub fn invoice_footer() -> String {
    let siren = legal("SIGLAIR_LEGAL_SIREN", "887 948 818");
    // La clé est dérivée du SIREN RÉELLEMENT configuré : dérivée d'une constante, un
    // exploitant qui renseigne son propre SIREN sans renseigner son n° de TVA verrait
    // partir des factures portant le numéro d'une autre société.
    let derived = vat_fr(
        &siren
            .chars()
            .filter(char::is_ascii_digit)
            .collect::<String>(),
    );
    format!(
        "{name} — SIREN {siren} — TVA intracommunautaire {vat}\n\
         Prestation de services fournie par voie électronique.\n\
         Lorsque la TVA figure à 0 % pour un preneur assujetti établi dans un autre État \
         membre de l'Union européenne : Autoliquidation — article 283-2 du CGI.\n\
         Contrat conclu entre professionnels : le droit de rétractation des consommateurs \
         (article L221-18 du code de la consommation) ne s'applique pas.\n\
         Pénalités de retard : trois fois le taux d'intérêt légal. Indemnité forfaitaire \
         pour frais de recouvrement : 40 € (article L441-10 du code de commerce).",
        name = legal("SIGLAIR_LEGAL_NAME", "KAIROS, SAS"),
        vat = legal("SIGLAIR_LEGAL_VAT", &derived),
    )
}

/// Numéro de TVA intracommunautaire français : `FR` + clé + SIREN, la clé valant
/// `(12 + 3 × (SIREN mod 97)) mod 97`. Dérivé plutôt que recopié : une clé fausse sur une
/// facture, c'est une facture à refaire pour chaque client.
fn vat_fr(siren: &str) -> String {
    let n: u64 = siren.parse().unwrap_or(0);
    format!("FR{:02}{siren}", (12 + 3 * (n % 97)) % 97)
}

/// Vrai quand Stripe refuse la session parce que **Stripe Tax n'est pas activé** sur le
/// compte — configuration à faire une fois dans le tableau de bord, pas une panne serveur.
///
/// On regarde `param` en premier : c'est le seul champ stable. Le message, lui, est de la
/// prose anglaise que Stripe reformule ; les motifs retenus sont ceux qui ont survécu à
/// plusieurs reformulations (`tax settings`, l'URL du réglage, `Stripe Tax`).
pub fn is_tax_not_configured(param: &str, code: &str, message: &str) -> bool {
    let m = message.to_ascii_lowercase();
    param.starts_with("automatic_tax")
        || code == "tax_origin_address_missing"
        || m.contains("settings/tax")
        || m.contains("tax settings")
        || m.contains("stripe tax")
        || m.contains("origin address")
}

/// Message rendu à l'appelant quand Stripe Tax n'est pas activé : 501, pas 500 — réessayer
/// n'y changera rien, et le texte dit exactement quoi faire.
pub fn not_configured_error() -> AppError {
    AppError::NotImplemented(
        "Le paiement est momentanément indisponible : le calcul de la TVA n'est pas encore \
         activé sur ce serveur. Activez Stripe Tax dans le tableau de bord Stripe \
         (Réglages → Taxes : renseigner l'adresse d'origine, choisir « Logiciel » / \
         « Services numériques » comme catégorie par défaut, puis activer le calcul \
         automatique), puis relancez le paiement."
            .into(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::billing::stripe::checkout_form;

    /// Le corps réellement envoyé à Stripe, encodé comme il partira sur le réseau.
    ///
    /// C'est du code de paiement : il ne se relit pas à l'œil. On vérifie le corps sérialisé
    /// et pas la `Vec` intermédiaire, parce que l'oubli qui coûte cher n'est pas « la
    /// constante est fausse », c'est « les champs de TVA ne sont pas joints au formulaire ».
    #[test]
    fn le_corps_du_checkout_porte_la_tva() {
        let form = checkout_form(
            uuid::Uuid::nil(),
            "cus_1",
            "price_pro_m",
            1,
            "https://siglair.com",
        );
        let body = serde_urlencoded::to_string(&form).expect("corps encodable");

        for expected in [
            "automatic_tax%5Benabled%5D=true",
            "tax_id_collection%5Benabled%5D=true",
            "billing_address_collection=required",
            "customer_update%5Baddress%5D=auto",
            "customer_update%5Bname%5D=auto",
            "allow_promotion_codes=true",
        ] {
            assert!(
                body.contains(expected),
                "champ manquant : {expected}\n{body}"
            );
        }

        // `invoice_creation` est réservé à `mode=payment` : sur un abonnement, Stripe
        // rejette la session entière. Le pied de page passe par le client (§ en-tête).
        assert!(!body.contains("invoice_creation"));
        assert!(body.contains("mode=subscription"));
    }

    /// Le pied de page porte les trois mentions que Stripe ne connaît pas.
    #[test]
    fn le_pied_de_facture_porte_les_mentions_francaises() {
        let f = invoice_footer();
        assert!(f.contains("SIREN 887 948 818"));
        assert!(f.contains("FR68887948818"));
        assert!(f.contains("Autoliquidation — article 283-2 du CGI"));
        assert!(
            f.contains("L221-18"),
            "absence de droit de rétractation B2B"
        );
    }

    /// Clé de contrôle du n° de TVA — recopiée à la main, elle serait fausse une fois sur dix.
    #[test]
    fn cle_du_numero_de_tva() {
        assert_eq!(vat_fr("887948818"), "FR68887948818");
        // clé < 10 : deux chiffres obligatoires, `FR9…` serait invalide
        assert_eq!(vat_fr("000000097"), "FR12000000097");
    }

    #[test]
    fn tax_non_active_reconnu_sans_confondre_avec_une_vraie_panne() {
        assert!(is_tax_not_configured(
            "automatic_tax[enabled]",
            "",
            "You cannot enable automatic tax…"
        ));
        assert!(is_tax_not_configured(
            "",
            "",
            "…configure your tax settings at https://dashboard.stripe.com/settings/tax"
        ));
        assert!(is_tax_not_configured("", "tax_origin_address_missing", ""));
        // Une carte refusée ou un price inexistant reste une erreur ordinaire.
        assert!(!is_tax_not_configured(
            "line_items[0][price]",
            "resource_missing",
            "No such price"
        ));
        assert!(!is_tax_not_configured(
            "",
            "card_declined",
            "Your card was declined."
        ));
    }
}
