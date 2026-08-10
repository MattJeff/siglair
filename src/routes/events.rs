//! `POST /api/events` — ingestion first-party pour la landing et l'application.
//!
//! La route accepte l'anonyme. Si le cookie de session HttpOnly est présent, `user_id` et
//! l'organisation courante sont dérivés côté serveur ; aucun identifiant de compte reçu du
//! navigateur n'est cru. Ni l'IP ni le User-Agent brut ne sont écrits.

use std::time::Duration;

use axum::{
    extract::{DefaultBodyLimit, State},
    http::{header, HeaderMap, StatusCode},
    routing::post,
    Json, Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64URL, Engine as _};
use serde::Serialize;
use sqlx::{Postgres, QueryBuilder};

use crate::{
    analytics::{self, country_code, IngestRequest, ValidatedBatch, MAX_REQUEST_BYTES},
    auth::{client_ip, current_org, OptionalUser},
    error::{AppError, Result},
    util::{hash_ip, rate_limit},
    AppState,
};

const RATE_WINDOW: Duration = Duration::from_secs(60);
const REQUESTS_PER_WINDOW: usize = 120;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/events", post(ingest))
        .layer(DefaultBodyLimit::max(MAX_REQUEST_BYTES))
}

#[derive(Serialize)]
struct IngestResponse {
    accepted: u64,
    duplicates: u64,
}

async fn ingest(
    State(st): State<AppState>,
    OptionalUser(user): OptionalUser,
    headers: HeaderMap,
    Json(request): Json<IngestRequest>,
) -> Result<(StatusCode, Json<IngestResponse>)> {
    let privacy_signal = headers
        .get("sec-gpc")
        .or_else(|| headers.get("dnt"))
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "yes"));
    if privacy_signal {
        return Ok((
            StatusCode::ACCEPTED,
            Json(IngestResponse {
                accepted: 0,
                duplicates: request.events.len() as u64,
            }),
        ));
    }

    let batch = ValidatedBatch::validate(request, chrono::Utc::now())
        .map_err(|error| AppError::validation(error.message()))?;

    // L'IP ne sert qu'à protéger ce point d'entrée public. Seule son empreinte salée vit
    // brièvement dans le compteur mémoire ; aucune colonne de `product_events` ne la reçoit.
    let rate_key = client_ip(&headers)
        .map(|ip| format!("events:ip:{}", B64URL.encode(hash_ip(&ip, &st.cfg.ip_salt))))
        .unwrap_or_else(|| format!("events:visitor:{}", batch.events[0].visitor_id));
    if !rate_limit(&rate_key, REQUESTS_PER_WINDOW, RATE_WINDOW) {
        return Err(AppError::RateLimited);
    }

    // Une session valide apporte l'utilisateur. L'absence d'organisation ne doit pas rendre
    // la collecte anonyme indisponible (compte en cours de création ou appartenance retirée).
    let (user_id, org_id, plan) = match user {
        Some(user) => {
            let org = current_org(&st.db, user.id).await.ok();
            (
                Some(user.id),
                org.as_ref().map(|access| access.org_id),
                org.as_ref().map(|access| access.plan.id),
            )
        }
        None => (None, None, None),
    };

    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|value| value.to_str().ok());
    let browser = analytics::browser_family(user_agent);
    let device = analytics::device_type(user_agent);
    let country = country_code(
        headers
            .get("cf-ipcountry")
            .and_then(|value| value.to_str().ok()),
    );

    let total = batch.events.len() as u64;
    let mut query = QueryBuilder::<Postgres>::new(
        "INSERT INTO product_events (event_id, name, visitor_id, analytics_session_id, \
         user_id, org_id, plan, properties, page_path, referrer_host, utm_source, utm_medium, \
         utm_campaign, utm_content, utm_term, browser_family, device_type, country_code, \
         language, occurred_at) ",
    );
    query.push_values(batch.events, |mut row, event| {
        row.push_bind(event.event_id)
            .push_bind(event.name.as_str())
            .push_bind(event.visitor_id)
            .push_bind(event.session_id)
            .push_bind(user_id)
            .push_bind(org_id)
            .push_bind(plan)
            .push_bind(event.properties)
            .push_bind(event.page_path)
            .push_bind(event.referrer_host)
            .push_bind(event.utm_source)
            .push_bind(event.utm_medium)
            .push_bind(event.utm_campaign)
            .push_bind(event.utm_content)
            .push_bind(event.utm_term)
            .push_bind(browser)
            .push_bind(device)
            .push_bind(country.clone())
            .push_bind(event.language)
            .push_bind(event.occurred_at);
    });
    query.push(" ON CONFLICT (event_id) DO NOTHING");

    let accepted = query.build().execute(&st.db).await?.rows_affected();
    Ok((
        StatusCode::ACCEPTED,
        Json(IngestResponse {
            accepted,
            duplicates: total.saturating_sub(accepted),
        }),
    ))
}
