//! `GET /api/growth` — le tableau de bord de croissance du contrat §11.4.
//!
//! Une seule route, une seule phrase à afficher :
//!
//! ```text
//! Mathis · 870 vues · 14 clics sur le badge · 3 comptes créés · 1 Pro
//! ```
//!
//! Trois règles reprises du contrat, dans cet ordre :
//!
//! 1. **les vues sont le chiffre le moins fiable du tableau** (§11.1) : Apple MPP précharge,
//!    le proxy Gmail met en cache. Elles sortent d'ici avec `views_note`, pour que le front
//!    n'ait pas à inventer l'avertissement — ni à l'oublier ;
//! 2. **la rétention du plan borne la fenêtre** (§6) : 0 j en Free (402), 30 j en Pro,
//!    365 j en Team. Un `?days=3650` ne donne pas à un Pro l'historique d'un Team ;
//! 3. **`orgs.analytics_enabled` coupe tout** (§4.1) : l'org qui a refusé la mesure ne voit
//!    pas d'historique résiduel, elle voit un tableau vide et `enabled: false`.

use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    auth::OrgAccess,
    error::{AppError, Result},
    growth::Funnel,
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new().route("/growth", get(dashboard))
}

/// §11.1 : la phrase que le front doit afficher à côté des vues. Elle vit ici parce que
/// c'est le serveur qui sait que le chiffre est faux ; le front, lui, l'oublierait.
const VIEWS_NOTE: &str = "Les ouvertures sont indicatives : Apple Mail et le proxy d'images \
                          de Gmail préchargent les images, une ouverture peut n'être qu'une \
                          machine. Aucune décision ne se prend dessus.";

#[derive(Deserialize)]
struct Window {
    days: Option<i32>,
}

/// Fenêtre effective : ce que demande l'appelant, borné par la rétention du plan (§6).
///
/// Le plafond est ici et pas dans la query string : un `?days=3650` sur un compte Pro
/// donnerait l'historique d'un Team sans le payer.
fn window_days(asked: Option<i32>, retention: u32) -> i32 {
    let max = retention as i32;
    asked.unwrap_or(max).clamp(1, max)
}

/// Détail par signature. `installed` est un booléen et pas un compteur : l'index unique
/// partiel de la migration 0007 ne garde qu'une installation par signature.
#[derive(Serialize, sqlx::FromRow)]
struct SignatureFunnel {
    id: Uuid,
    name: String,
    slug: Option<String>,
    views: i64,
    badge_clicks: i64,
    signups: i64,
    /// Parmi `signups`, ceux qui appartiennent aujourd'hui à une org payante. Sans cette
    /// colonne, la ligne « payants » du détail resterait à zéro quoi qu'il arrive.
    paid: i64,
    installed: bool,
}

/// ponytail : quatre sous-requêtes corrélées par signature. Une org en compte quelques
/// dizaines et toutes les colonnes visées sont indexées (`events(signature_id, occurred_at)`,
/// `growth_events(org_id, occurred_at)`, `users(referred_by_signature_id)`). Le jour où une
/// org a mille signatures, passer à trois agrégats groupés joints sur `signatures`.
const PER_SIGNATURE_SQL: &str = "\
    SELECT s.id, s.name, s.public_slug::text AS slug, \
      (SELECT count(*) FROM events e \
        WHERE e.signature_id = s.id AND e.kind = 'open' \
          AND e.occurred_at >= now() - make_interval(days => $2::int)) AS views, \
      (SELECT count(*) FROM growth_events g \
        WHERE g.signature_id = s.id AND g.kind = 'powered_by_click' \
          AND g.occurred_at >= now() - make_interval(days => $2::int)) AS badge_clicks, \
      (SELECT count(*) FROM users u \
        WHERE u.referred_by_signature_id = s.id \
          AND u.created_at >= now() - make_interval(days => $2::int)) AS signups, \
      (SELECT count(*) FROM users u \
        WHERE u.referred_by_signature_id = s.id \
          AND u.created_at >= now() - make_interval(days => $2::int) \
          AND EXISTS (SELECT 1 FROM org_members m JOIN orgs o ON o.id = m.org_id \
                       WHERE m.user_id = u.id AND o.plan <> 'free')) AS paid, \
      EXISTS (SELECT 1 FROM growth_events gi \
               WHERE gi.signature_id = s.id AND gi.kind = 'signature_installed') AS installed \
    FROM signatures s \
    WHERE s.org_id = $1::uuid AND s.deleted_at IS NULL \
    ORDER BY badge_clicks DESC, views DESC, s.created_at \
    LIMIT 100";

/// `GET /api/growth?days=` — session requise (l'extracteur `OrgAccess` la porte).
async fn dashboard(
    State(st): State<AppState>,
    access: OrgAccess,
    Query(q): Query<Window>,
) -> Result<Json<Value>> {
    let retention = access.plan.limits.analytics_days;
    if retention == 0 {
        // Même verrou que `/api/signatures/{id}/analytics` : §6, l'analytics commence à Pro.
        return Err(AppError::QuotaExceeded(format!(
            "Le tableau de bord de croissance est inclus à partir du plan Pro ({prix}/mois, \
             30 jours d'historique, 12 mois en Team).",
            prix = crate::plans::PRO.price_label(),
        )));
    }
    let days = window_days(q.days, retention);

    if !access.analytics_enabled {
        // §4.1 : la mesure est coupée pour cette org, on ne montre pas d'historique résiduel.
        return Ok(Json(json!({
            "enabled": false,
            "days": days,
            "funnel": Funnel::default(),
            "rates": { "click": null, "signup": null, "paid": null },
            "signatures": [],
            "views_note": VIEWS_NOTE,
        })));
    }

    let funnel = crate::growth::funnel(&st.db, access.org_id, days).await?;
    let signatures: Vec<SignatureFunnel> = sqlx::query_as(PER_SIGNATURE_SQL)
        .bind(access.org_id)
        .bind(days)
        .fetch_all(&st.db)
        .await?;

    Ok(Json(json!({
        "enabled": true,
        "days": days,
        "funnel": funnel,
        // §11.4 : `None` quand il n'y a rien à diviser — « 0 % » et « pas encore de donnée »
        // ne se disent pas pareil à quelqu'un qui décide d'un budget.
        "rates": {
            "click": funnel.click_rate(),
            "signup": funnel.signup_rate(),
            "paid": funnel.paid_rate(),
        },
        "signatures": signatures,
        "views_note": VIEWS_NOTE,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plans::{FREE, PRO, TEAM};

    /// La seule logique de ce fichier qui puisse vendre à un client ce qu'il n'a pas payé.
    #[test]
    fn la_fenetre_ne_depasse_jamais_la_retention_du_plan() {
        // §6 : Pro 30 jours, Team 365.
        assert_eq!(window_days(None, PRO.limits.analytics_days), 30);
        assert_eq!(window_days(None, TEAM.limits.analytics_days), 365);
        // un Pro qui demande l'historique d'un Team reste à 30 jours
        assert_eq!(window_days(Some(3650), PRO.limits.analytics_days), 30);
        // une fenêtre plus courte est légitime
        assert_eq!(window_days(Some(7), PRO.limits.analytics_days), 7);
        // valeurs absurdes : jamais 0 ni négatif, `make_interval` accepterait les deux
        assert_eq!(window_days(Some(0), PRO.limits.analytics_days), 1);
        assert_eq!(window_days(Some(-30), TEAM.limits.analytics_days), 1);
        // Free n'atteint pas cette fonction : la route répond 402 avant.
        assert_eq!(FREE.limits.analytics_days, 0);
    }

    /// Le détail par signature ne doit jamais sortir de l'organisation appelante, et une
    /// signature supprimée ne réapparaît pas dans un tableau de bord.
    #[test]
    fn le_detail_par_signature_reste_dans_lorganisation() {
        assert!(PER_SIGNATURE_SQL.contains("s.org_id = $1::uuid"));
        assert!(PER_SIGNATURE_SQL.contains("s.deleted_at IS NULL"));
        // toutes les fenêtres sont bornées par le même paramètre, aucun compteur « à vie »
        // views, badge_clicks, signups, paid. `installed` seul n'est pas fenêtré : c'est un
        // booléen unique par signature, le faire disparaître au bout de 30 jours dirait
        // « plus installée » alors qu'elle l'est toujours.
        assert_eq!(
            PER_SIGNATURE_SQL
                .matches("make_interval(days => $2::int)")
                .count(),
            4
        );
        // chaque colonne rendue au front est calculée : une colonne toujours nulle est une
        // promesse vide à l'écran
        for column in [
            "AS views",
            "AS badge_clicks",
            "AS signups",
            "AS paid",
            "AS installed",
        ] {
            assert!(PER_SIGNATURE_SQL.contains(column), "{column}");
        }
    }
}
