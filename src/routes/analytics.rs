//! Statistiques d'une signature — contrat §5.3 et §6.
//!
//! Une requête agrégée par graphique, jamais une boucle : PostgreSQL sait grouper par jour
//! bien mieux qu'une boucle Rust qui rouvre une connexion à chaque itération.

use axum::Json;
use chrono::NaiveDate;
use serde::Serialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    auth::OrgAccess,
    error::{AppError, Result},
    AppState,
};

#[derive(Serialize, sqlx::FromRow)]
struct DayRow {
    day: NaiveDate,
    opens: i64,
    clicks: i64,
}

#[derive(Serialize, sqlx::FromRow)]
struct ElementRow {
    element_id: String,
    clicks: i64,
}

#[derive(Serialize, sqlx::FromRow)]
struct ClientRow {
    family: String,
    opens: i64,
    clicks: i64,
}

pub async fn for_signature(
    st: &AppState,
    access: &OrgAccess,
    signature_id: Uuid,
) -> Result<Json<Value>> {
    let (plan, analytics_enabled) = (access.plan, access.analytics_enabled);
    if plan.limits.analytics_days == 0 {
        return Err(AppError::QuotaExceeded(
            "Le suivi des ouvertures et des clics est inclus à partir du plan Pro (30 jours \
             d'historique, 12 mois en Team)."
                .into(),
        ));
    }

    let owned: bool = sqlx::query_scalar(
        "SELECT exists(SELECT 1 FROM signatures \
         WHERE id = $1 AND org_id = $2 AND deleted_at IS NULL)",
    )
    .bind(signature_id)
    .bind(access.org_id)
    .fetch_one(&st.db)
    .await?;
    if !owned {
        return Err(AppError::NotFound);
    }

    let days = plan.limits.analytics_days as i32;
    if !analytics_enabled {
        // contrat §4.1 : l'org a coupé la mesure, on ne montre pas d'historique résiduel
        return Ok(Json(json!({
            "enabled": false, "days": days,
            "totals": { "opens": 0, "clicks": 0 },
            "series": [], "top_elements": [], "clients": [],
        })));
    }

    let series: Vec<DayRow> = sqlx::query_as(
        "SELECT (occurred_at AT TIME ZONE 'UTC')::date AS day, \
                count(*) FILTER (WHERE kind = 'open')  AS opens, \
                count(*) FILTER (WHERE kind = 'click') AS clicks \
         FROM events \
         WHERE signature_id = $1 AND occurred_at >= now() - make_interval(days => $2) \
         GROUP BY 1 ORDER BY 1",
    )
    .bind(signature_id)
    .bind(days)
    .fetch_all(&st.db)
    .await?;

    let top_elements: Vec<ElementRow> = sqlx::query_as(
        "SELECT element_id, count(*) AS clicks \
         FROM events \
         WHERE signature_id = $1 AND kind = 'click' AND element_id IS NOT NULL \
           AND occurred_at >= now() - make_interval(days => $2) \
         GROUP BY 1 ORDER BY 2 DESC, 1 LIMIT 10",
    )
    .bind(signature_id)
    .bind(days)
    .fetch_all(&st.db)
    .await?;

    let clients: Vec<ClientRow> = sqlx::query_as(
        "SELECT coalesce(ua_family, 'other') AS family, \
                count(*) FILTER (WHERE kind = 'open')  AS opens, \
                count(*) FILTER (WHERE kind = 'click') AS clicks \
         FROM events \
         WHERE signature_id = $1 AND occurred_at >= now() - make_interval(days => $2) \
         GROUP BY 1 ORDER BY 2 DESC, 1",
    )
    .bind(signature_id)
    .bind(days)
    .fetch_all(&st.db)
    .await?;

    // les totaux se déduisent de la série : pas de quatrième requête pour deux additions
    let opens: i64 = series.iter().map(|d| d.opens).sum();
    let clicks: i64 = series.iter().map(|d| d.clicks).sum();

    Ok(Json(json!({
        "enabled": true,
        "days": days,
        "totals": { "opens": opens, "clicks": clicks },
        "series": series,
        "top_elements": top_elements,
        "clients": clients,
    })))
}
