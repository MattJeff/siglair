//! `GET /r/{code}` — le badge des signatures Free (contrat §11.2 et §11.3).
//!
//! Route **publique**, servie à des destinataires d'e-mails qui ne sont pas nos
//! utilisateurs. Trois règles tiennent le fichier :
//!
//! 1. **aucun cookie.** Le destinataire n'a rien accepté ; un cookie d'attribution relève
//!    d'ePrivacy et impose un bandeau de consentement, lequel ferait chuter précisément la
//!    conversion qu'on cherche à mesurer. L'attribution voyage dans l'URL (`?ref=`) et
//!    n'est inscrite en base qu'à la création du compte ;
//! 2. **rien de lent dans le chemin de la réponse** : l'événement part en tâche détachée,
//!    comme pour `/s` et `/c` ;
//! 3. **un code inconnu redirige quand même.** Un lien mort dans un e-mail déjà parti doit
//!    atterrir quelque part de correct, pas sur une page d'erreur.

use std::time::Duration;

use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64URL, Engine as _};
use uuid::Uuid;

use crate::{
    auth::client_ip,
    growth::{self, GrowthEvent, Visitor},
    util::{hash_ip, rate_limit},
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new().route("/r/{code}", get(referral))
}

/// Plafond de **comptage** par empreinte d'IP — jamais de la redirection : un destinataire
/// limité en débit doit quand même arriver sur la landing.
///
/// Un humain clique une fois, deux s'il revient. Un robot qui déplie tous les liens d'une
/// boîte mail (antivirus d'entreprise, prévisualisateur de lien, crawler) en produirait des
/// centaines et gonflerait la seule métrique d'acquisition du produit. Dix par heure laisse
/// passer un bureau entier derrière une IP partagée sans laisser passer un crawler.
const MAX_COUNTED_PER_HOUR: usize = 10;
const COUNT_WINDOW: Duration = Duration::from_secs(3600);

/// Un slug fait 12 caractères (`util::gen_slug`). Le plafond évite qu'une URL absurde
/// devienne une clé de cache ou une requête SQL inutile.
const MAX_CODE_LEN: usize = 64;

async fn referral(
    State(st): State<AppState>,
    Path(code): Path<String>,
    headers: HeaderMap,
) -> Response {
    if code.is_empty() || code.len() > MAX_CODE_LEN {
        return redirect(&st.cfg.app_url, None);
    }

    // Le code d'attribution est le slug public de la signature (contrat §11.2).
    // Pas de filtre sur `deleted_at` : un e-mail déjà parti continue de créditer son auteur
    // même s'il a supprimé le brouillon depuis. La suppression du compte, elle, emporte la
    // ligne (§11.5) et le clic retombe sur la landing sans paramètre.
    let row: Option<(Uuid, Uuid, String)> = sqlx::query_as(
        "SELECT s.id, s.org_id, s.public_slug::text \
         FROM signatures s WHERE s.public_slug = $1::citext",
    )
    .bind(&code)
    .fetch_optional(&st.db)
    .await
    .unwrap_or_else(|error| {
        // Une base indisponible ne doit pas transformer un lien d'e-mail en page d'erreur.
        tracing::warn!(?error, "code de parrainage non résolu");
        None
    });

    let Some((signature_id, org_id, slug)) = row else {
        return redirect(&st.cfg.app_url, None);
    };

    // `orgs.analytics_enabled` n'est pas relu ici : le SQL de `growth::record` porte déjà la
    // condition (§4.1), et deux endroits qui décident la même chose finissent par diverger.
    record(&st, org_id, signature_id, &headers);

    // Le paramètre reprend la valeur lue en base, jamais celle du chemin : rien de ce que
    // fournit l'appelant ne se retrouve dans l'en-tête `Location`.
    redirect(&st.cfg.app_url, Some(&slug))
}

/// Toute la réponse vue par le destinataire, dans une fonction pure — c'est aussi le seul
/// endroit d'où pourrait sortir un `Set-Cookie`, et le test le vérifie.
fn redirect(app_url: &str, code: Option<&str>) -> Response {
    let location = match code {
        Some(code) => format!("{app_url}/?ref={code}"),
        None => format!("{app_url}/"),
    };
    (
        StatusCode::FOUND,
        [
            (header::LOCATION, location),
            // Une redirection mise en cache par un proxy ne serait jamais recomptée.
            (header::CACHE_CONTROL, "no-store".to_string()),
        ],
    )
        .into_response()
}

/// Contrat §4.1 : ni IP en clair, ni User-Agent brut. Contrat §11.1 : `powered_by_click`
/// et rien d'autre — le badge n'a qu'une question à laquelle répondre, « combien de
/// destinataires mordent ».
fn record(st: &AppState, org_id: Uuid, signature_id: Uuid, headers: &HeaderMap) {
    let ip = client_ip(headers);
    // Le hachage du plafond est local ; celui qui part en base est fait par `growth::record`,
    // seul endroit où une IP brute est transformée (§4.1).
    if !countable(
        ip.as_deref()
            .map(|ip| hash_ip(ip, &st.cfg.ip_salt))
            .as_deref(),
    ) {
        return;
    }
    let ua = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);

    // Tâche détachée : le 302 part sans attendre une écriture en base. Un destinataire ne
    // patiente pas pour une métrique, et l'événement perdu vaut mieux que la page lente.
    let (db, salt) = (st.db.clone(), st.cfg.ip_salt.clone());
    tokio::spawn(async move {
        growth::record(
            &db,
            org_id,
            None, // un destinataire d'e-mail n'est pas un de nos utilisateurs (§11.3)
            Some(signature_id),
            GrowthEvent::PoweredByClick,
            serde_json::json!({}),
            Visitor {
                ip: ip.as_deref(),
                ua: ua.as_deref(),
                salt: &salt,
            },
        )
        .await;
    });
}

/// ponytail : compteur mémoire par instance (`util::rate_limit`), remis à zéro au
/// redéploiement. Le jour où il y a deux instances, le plafond devient deux fois plus haut
/// — comme pour toutes les autres limites du produit, à passer en table PG en même temps.
fn countable(ip_hash: Option<&[u8]>) -> bool {
    // Sans en-tête de proxy on ne distingue plus personne : une clé commune, plutôt qu'un
    // comptage sans plafond qu'un robot anonyme viderait de son sens.
    let key = match ip_hash {
        Some(h) => format!("ref:{}", B64URL.encode(h)),
        None => "ref:anon".to_string(),
    };
    rate_limit(&key, MAX_COUNTED_PER_HOUR, COUNT_WINDOW)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header_of(res: &Response, name: header::HeaderName) -> Option<&str> {
        res.headers().get(name)?.to_str().ok()
    }

    /// La règle la plus facile à casser par mégarde (contrat §11.3) : le destinataire d'un
    /// e-mail n'est pas notre utilisateur, il ne reçoit aucun cookie. Un cookie ici, et
    /// c'est un bandeau de consentement sur la landing.
    #[test]
    fn aucun_cookie_nest_pose_sur_le_destinataire() {
        for res in [
            redirect("https://siglair.com", None),
            redirect("https://siglair.com", Some("abcdefgh2345")),
        ] {
            assert!(
                res.headers().get(header::SET_COOKIE).is_none(),
                "un Set-Cookie est apparu sur /r/"
            );
        }
    }

    #[test]
    fn un_code_inconnu_atterrit_sur_la_landing_sans_parametre() {
        let res = redirect("https://siglair.com", None);
        assert_eq!(res.status(), StatusCode::FOUND);
        assert_eq!(
            header_of(&res, header::LOCATION),
            Some("https://siglair.com/")
        );
        assert!(!header_of(&res, header::LOCATION).unwrap().contains("ref"));
    }

    #[test]
    fn un_code_valide_transporte_lattribution_dans_lurl() {
        let res = redirect("https://siglair.com", Some("abcdefgh2345"));
        assert_eq!(res.status(), StatusCode::FOUND);
        assert_eq!(
            header_of(&res, header::LOCATION),
            Some("https://siglair.com/?ref=abcdefgh2345")
        );
        // un proxy qui met la redirection en cache ne la ferait jamais recompter
        assert_eq!(header_of(&res, header::CACHE_CONTROL), Some("no-store"));
    }

    /// Le plafond protège le compteur, jamais la redirection : `referral` appelle
    /// `redirect` quoi qu'il arrive.
    #[test]
    fn le_plafond_arrete_le_comptage_dun_robot() {
        let ip = hash_ip("203.0.113.7", "sel-de-test");
        assert!(
            (0..MAX_COUNTED_PER_HOUR).all(|_| countable(Some(&ip))),
            "les premiers clics doivent être comptés"
        );
        assert!(
            !countable(Some(&ip)),
            "le 11e clic de la même IP est ignoré"
        );
        // une autre IP ne paie pas pour le robot
        assert!(countable(Some(&hash_ip("198.51.100.3", "sel-de-test"))));
    }

    /// Le badge et cette route forment une seule fonctionnalité : le test vit ici parce que
    /// le lot ne me donne que `branding_link` et `branding_row` dans `html.rs`.
    #[test]
    fn le_badge_pointe_la_route_dattribution() {
        use crate::{
            doc::{Doc, Profile},
            render::{html::render_document, RenderMode, RenderOpts},
        };

        let opts = |slug: Option<&str>| RenderOpts {
            public_url: "https://siglair.com".into(),
            slug: slug.map(String::from),
            branding: true,
            ..Default::default()
        };
        let html = render_document(
            &Doc::default(),
            &Profile::new(),
            RenderMode::Safe,
            &opts(Some("abcdefgh2345")),
        );
        assert!(html.contains("Powered by siglair.com"));
        assert!(html.contains("href=\"https://siglair.com/r/abcdefgh2345\""));

        // sans slug publié il n'y a rien à attribuer : la racine, pas un `/r/` vide
        let sans_slug = render_document(
            &Doc::default(),
            &Profile::new(),
            RenderMode::Safe,
            &opts(None),
        );
        assert!(sans_slug.contains("href=\"https://siglair.com\""));
        assert!(!sans_slug.contains("/r/"));

        // plan payant : aucun badge
        let paid = render_document(
            &Doc::default(),
            &Profile::new(),
            RenderMode::Safe,
            &RenderOpts {
                branding: false,
                ..opts(Some("abcdefgh2345"))
            },
        );
        assert!(!paid.contains("Powered by siglair.com"));
    }
}
