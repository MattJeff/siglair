//! Routes publiques — contrat §5.1. C'est ce que les clients mail appellent, des millions
//! de fois, sans session et sans indulgence.
//!
//! Deux règles tiennent tout le fichier :
//! 1. rien de lent dans le chemin de la réponse — l'écriture de l'événement part en tâche
//!    détachée, une image qui met 400 ms à s'afficher est une image cassée ;
//! 2. la cible d'un `/c/...` vient du document en base, jamais de la requête — sinon
//!    Siglair devient un redirecteur ouvert exploitable pour du phishing depuis son
//!    propre domaine.

use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde_json::{json, Value};
use url::Url;
use uuid::Uuid;

use crate::{
    db,
    doc::{resolve_tokens, safe_href, Doc, Profile},
    error::{AppError, Result},
    growth::{self, GrowthEvent, Visitor},
    util::{hash_ip, ua_family},
    AppState,
};

/// Les proxys de Gmail rappellent l'URL très souvent : 5 minutes de cache absorbent la
/// rafale sans figer une republication.
const CACHE: &str = "public, max-age=300";

/// GIF 1×1 transparent. Un client mail affiche une image, pas un code HTTP : mieux vaut un
/// pixel invisible qu'un cadre « image cassée » dans la signature d'un client.
const BLANK_GIF: &[u8] = &[
    0x47, 0x49, 0x46, 0x38, 0x39, 0x61, 0x01, 0x00, 0x01, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00,
    0xff, 0xff, 0xff, 0x21, 0xf9, 0x04, 0x01, 0x00, 0x00, 0x00, 0x00, 0x2c, 0x00, 0x00, 0x00, 0x00,
    0x01, 0x00, 0x01, 0x00, 0x00, 0x02, 0x02, 0x44, 0x01, 0x00, 0x3b,
];

/// PNG 1×1 transparent.
const BLANK_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae,
    0x42, 0x60, 0x82,
];

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .route("/s/{file}", get(image))
        .route("/c/{slug}/{element_id}", get(click))
        .route("/f/{*key}", get(media))
}

// ------------------------------------------------------------------ sondes

async fn health() -> Json<Value> {
    Json(json!({ "ok": true }))
}

async fn ready(State(st): State<AppState>) -> Response {
    if db::ping(&st.db).await {
        (StatusCode::OK, Json(json!({ "ok": true }))).into_response()
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "ok": false })),
        )
            .into_response()
    }
}

// ------------------------------------------------------------------ image publiée

#[derive(sqlx::FromRow)]
struct ImageRow {
    id: Uuid,
    /// Nécessaire au seul `growth_events.org_id` : la table est lue par organisation (§11.4).
    org_id: Uuid,
    plan: String,
    subscription_status: Option<String>,
    grace_until: Option<chrono::DateTime<chrono::Utc>>,
    /// Nombre de signatures de l'org plus anciennes que celle-ci. 0 = la plus ancienne.
    /// C'est le rang qui décide quelles signatures survivent à une bascule vers Free.
    older: i64,
    analytics_enabled: bool,
    render_id: Option<Uuid>,
    gif_key: Option<String>,
    png_key: Option<String>,
}

// ponytail : la sous-requête `older` est corrélée, mais elle porte sur `signatures(org_id)`
// qui est indexé et sur des orgs à quelques dizaines de lignes. Si le plan d'exécution
// devient un problème, matérialiser le rang dans une colonne au moment de la publication.
const IMAGE_SQL: &str = "SELECT s.id, s.org_id, o.plan, o.subscription_status, o.grace_until, \
                                o.analytics_enabled, \
                                (SELECT count(*) FROM signatures s2 \
                                  WHERE s2.org_id = s.org_id AND s2.deleted_at IS NULL \
                                    AND (s2.created_at, s2.id) < (s.created_at, s.id)) AS older, \
                                r.id AS render_id, r.gif_key, r.png_key \
                         FROM signatures s \
                         JOIN orgs o ON o.id = s.org_id \
                         LEFT JOIN renders r ON r.id = s.published_render_id \
                         WHERE s.public_slug = $1::citext AND s.deleted_at IS NULL";

/// Cette signature doit-elle encore être servie ?
///
/// `older` est son rang d'ancienneté dans l'org (0 = la plus ancienne). Après une bascule
/// vers Free, les signatures au-delà du quota cessent d'être servies : c'est précisément
/// ce que les e-mails de relance annoncent, et sans ça la menace était vide — un ex-Pro
/// aux douze signatures continuait d'être servi sur les douze, gratuitement.
fn servable(plan: &crate::plans::Plan, older: i64) -> bool {
    plan.limits.hosted_gif
        && plan
            .limits
            .signatures
            .is_none_or(|max| older < i64::from(max))
}

async fn image(
    State(st): State<AppState>,
    Path(file): Path<String>,
    headers: HeaderMap,
) -> Response {
    let Some((slug, ext)) = file.rsplit_once('.') else {
        return AppError::NotFound.into_response();
    };
    let content_type = match ext {
        "gif" => "image/gif",
        "png" => "image/png",
        _ => return AppError::NotFound.into_response(),
    };
    if slug.is_empty() || slug.len() > 64 {
        return AppError::NotFound.into_response();
    }

    let row: Option<ImageRow> = match sqlx::query_as(IMAGE_SQL)
        .bind(slug)
        .fetch_optional(&st.db)
        .await
    {
        Ok(r) => r,
        Err(e) => return AppError::from(e).into_response(),
    };
    let Some(row) = row else {
        return AppError::NotFound.into_response();
    };

    // Le droit de servir se lit au moment de la requête : une org en grâce garde ses GIF,
    // une org basculée les perd sans attendre qu'un webhook réécrive une colonne.
    let plan = crate::billing::lifecycle::effective_plan(&crate::billing::lifecycle::OrgBilling {
        plan: row.plan.clone(),
        subscription_status: row.subscription_status.clone(),
        grace_until: row.grace_until,
    });

    if !servable(&plan, row.older) {
        // 402 exact pour nos propres outils, pixel neutre pour le client mail
        return blank(StatusCode::PAYMENT_REQUIRED, ext);
    }

    let key = match (row.render_id, ext) {
        (Some(_), "gif") => row.gif_key.clone(),
        (Some(_), _) => row.png_key.clone(),
        (None, _) => None,
    };
    let (Some(render_id), Some(key)) = (row.render_id, key) else {
        // rendu pas encore prêt : pixel non mis en cache, le proxy repassera dans un instant
        return blank(StatusCode::OK, ext);
    };

    // l'identifiant du rendu change à chaque republication : c'est exactement un ETag
    let etag = format!("\"{render_id}\"");
    let fresh = matches_etag(&headers, &etag);

    // même sur un 304, l'e-mail a bien été ouvert : l'événement part avant de répondre
    if row.analytics_enabled {
        record(&st, row.id, "open", None, None, &headers);
        // §11.1 `signature_installed` : la première ouverture qui ne vient pas d'une IP
        // connue du propriétaire. Toute la condition est dans le SQL de `growth::record` —
        // ici on ne fait que lui donner l'occasion de se poser.
        record_install(&st, row.org_id, row.id, &headers);
    }
    if fresh {
        return (
            StatusCode::NOT_MODIFIED,
            [
                (header::ETAG, etag),
                (header::CACHE_CONTROL, CACHE.to_string()),
            ],
        )
            .into_response();
    }

    match st.storage.get(&key).await {
        Ok(bytes) => (
            [
                (header::CONTENT_TYPE, content_type.to_string()),
                (header::CACHE_CONTROL, CACHE.to_string()),
                (header::ETAG, etag),
            ],
            bytes,
        )
            .into_response(),
        Err(e) => {
            tracing::warn!(error = ?e, %key, "rendu introuvable dans le stockage");
            blank(StatusCode::OK, ext)
        }
    }
}

fn blank(status: StatusCode, ext: &str) -> Response {
    let (ct, body) = if ext == "png" {
        ("image/png", BLANK_PNG)
    } else {
        ("image/gif", BLANK_GIF)
    };
    (
        status,
        [
            (header::CONTENT_TYPE, ct.to_string()),
            // jamais en cache : le plan peut changer, le rendu peut arriver
            (header::CACHE_CONTROL, "no-store".to_string()),
        ],
        body,
    )
        .into_response()
}

fn matches_etag(headers: &HeaderMap, etag: &str) -> bool {
    headers
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| {
            v.split(',').any(|t| {
                let t = t.trim();
                t == "*" || t.trim_start_matches("W/") == etag
            })
        })
}

// ------------------------------------------------------------------ clic

#[derive(sqlx::FromRow)]
struct ClickRow {
    id: Uuid,
    render_id: Option<Uuid>,
    doc: Option<Value>,
    profile: Option<Value>,
    analytics_enabled: bool,
}

/// Cible d'un clic : jetons résolus puis schéma re-vérifié (contrat §8). Un élément inconnu,
/// masqué ou sans lien valide donne `None` — 404, jamais de redirection « par défaut ».
fn click_target(doc: &Doc, profile: &Profile, element_id: &str) -> Option<String> {
    let element = doc
        .elements
        .iter()
        .find(|e| e.id == element_id && !e.hidden)?;
    safe_href(&resolve_tokens(&element.href, profile))
}

async fn click(
    State(st): State<AppState>,
    Path((slug, element_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response> {
    // Contrat §5.1 : la cible vient de l'instantané figé au rendu, jamais de `signatures.doc`
    // qui est le brouillon en cours d'édition — sinon éditer un brouillon change la
    // destination de liens déjà partis dans des e-mails, sans republication ni trace.
    let row: ClickRow = sqlx::query_as(
        "SELECT s.id, r.id AS render_id, r.doc, r.profile, o.analytics_enabled \
         FROM signatures s JOIN orgs o ON o.id = s.org_id \
         LEFT JOIN renders r ON r.id = s.published_render_id \
         WHERE s.public_slug = $1::citext AND s.deleted_at IS NULL",
    )
    .bind(&slug)
    .fetch_optional(&st.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let Some(doc) = row.doc else {
        // Rendu antérieur à la migration 0002 : pas d'instantané. Un lien mort se répare en
        // republiant ; une redirection vers une cible non vérifiée, non.
        if let Some(render_id) = row.render_id {
            tracing::warn!(%slug, %render_id, "rendu publié sans instantané : clic refusé, republier");
        }
        return Err(AppError::NotFound);
    };
    let doc: Doc = serde_json::from_value(doc)?;
    let profile: Profile = row
        .profile
        .and_then(|p| serde_json::from_value(p).ok())
        .unwrap_or_default();

    let target = click_target(&doc, &profile, &element_id).ok_or(AppError::NotFound)?;

    if row.analytics_enabled {
        let host = Url::parse(&target)
            .ok()
            .and_then(|u| u.host_str().map(str::to_string));
        record(&st, row.id, "click", Some(element_id), host, &headers);
    }

    Ok((
        StatusCode::FOUND,
        [
            (header::LOCATION, target),
            // chaque clic doit être compté : aucun cache ne s'intercale
            (header::CACHE_CONTROL, "no-store".to_string()),
            // la destination n'a pas à apprendre le slug de la signature
            (header::REFERRER_POLICY, "no-referrer".to_string()),
        ],
    )
        .into_response())
}

// ------------------------------------------------------------------ médias

async fn media(
    State(st): State<AppState>,
    Path(key): Path<String>,
    headers: HeaderMap,
) -> Result<Response> {
    let etag = format!("\"{}\"", key.replace('"', ""));
    if matches_etag(&headers, &etag) {
        return Ok((StatusCode::NOT_MODIFIED, [(header::ETAG, etag)]).into_response());
    }
    // `Storage::get` refuse les clés qui sortent de la racine ; rien à revalider ici
    let bytes = st.storage.get(&key).await?;
    Ok((
        [
            (header::CONTENT_TYPE, content_type_of(&key).to_string()),
            // la clé contient le sha256 du contenu : le média ne changera jamais
            (
                header::CACHE_CONTROL,
                "public, max-age=31536000, immutable".to_string(),
            ),
            (header::ETAG, etag),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_string()),
            (header::CONTENT_DISPOSITION, "inline".to_string()),
        ],
        bytes,
    )
        .into_response())
}

/// Liste fermée. Servir un type deviné depuis un nom de fichier, c'est laisser un
/// `.html` téléversé s'exécuter sur notre origine (contrat §8).
fn content_type_of(key: &str) -> &'static str {
    match key
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("mp4") => "video/mp4",
        Some("webm") => "video/webm",
        _ => "application/octet-stream",
    }
}

// ------------------------------------------------------------------ mesure

/// Contrat §5.1 : jamais dans le chemin de la réponse. Contrat §4.1 : ni IP en clair, ni
/// User-Agent brut en base.
fn record(
    st: &AppState,
    signature_id: Uuid,
    kind: &'static str,
    element_id: Option<String>,
    target_host: Option<String>,
    headers: &HeaderMap,
) {
    let db = st.db.clone();
    let salt = st.cfg.ip_salt.clone();
    let ip = client_ip(headers);
    let family = ua_family(
        headers
            .get(header::USER_AGENT)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default(),
    );

    tokio::spawn(async move {
        let ip_hash = ip.map(|ip| hash_ip(&ip, &salt));
        let res = sqlx::query(
            "INSERT INTO events (signature_id, kind, element_id, target_host, ip_hash, ua_family) \
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(signature_id)
        .bind(kind)
        .bind(element_id)
        .bind(target_host)
        .bind(ip_hash)
        .bind(family)
        .execute(&db)
        .await;
        if let Err(e) = res {
            tracing::warn!(error = ?e, "événement non enregistré");
        }
    });
}

/// `signature_installed` (§11.1) : « combien l'ont vraiment collée dans leur client mail ».
///
/// Rien n'est décidé ici. L'IP du propriétaire, l'unicité par signature et le respect de
/// `orgs.analytics_enabled` sont dans le SQL de `growth::record` : un chemin qui refarait ce
/// raisonnement à la main finirait par en oublier un morceau.
///
/// ponytail : une tentative d'insertion par ouverture. Après la première, l'index unique
/// partiel de la migration 0007 la transforme en `ON CONFLICT DO NOTHING`, soit une sonde
/// d'index dans une tâche détachée. Si le volume d'ouvertures rend ce coût mesurable, garder
/// en mémoire les signatures déjà installées et sauter l'appel — pas avant de l'avoir mesuré.
fn record_install(st: &AppState, org_id: Uuid, signature_id: Uuid, headers: &HeaderMap) {
    let (db, salt) = (st.db.clone(), st.cfg.ip_salt.clone());
    // L'IP brute et l'UA brut ne franchissent pas ce point : `record` les hache et les
    // réduit à une famille (§4.1).
    let ip = client_ip(headers);
    let ua = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);

    tokio::spawn(async move {
        growth::record(
            &db,
            org_id,
            None, // un destinataire d'e-mail n'est pas un de nos utilisateurs (§11.3)
            Some(signature_id),
            GrowthEvent::SignatureInstalled,
            json!({}),
            Visitor {
                ip: ip.as_deref(),
                ua: ua.as_deref(),
                salt: &salt,
            },
        )
        .await;
    });
}

/// ponytail : pas de `ConnectInfo`. En production la socket est toujours celle du proxy ;
/// la seule IP utile arrive par en-tête, et à défaut l'événement est simplement anonyme.
fn client_ip(headers: &HeaderMap) -> Option<String> {
    let raw = headers
        .get("x-forwarded-for")
        .or_else(|| headers.get("x-real-ip"))?
        .to_str()
        .ok()?;
    let first = raw.split(',').next()?.trim();
    (!first.is_empty() && first.len() <= 64).then(|| first.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ce qui est servi, et à qui. Faux dans un sens, on héberge gratuitement un client
    /// qui ne paie plus ; faux dans l'autre, on casse les signatures d'un client à jour.
    #[test]
    fn le_quota_du_plan_effectif_decide_de_ce_qui_est_servi() {
        use crate::plans::{FREE, PRO};
        // Free : la plus ancienne signature reste en ligne, les suivantes non. C'est la
        // conséquence annoncée par les e-mails de relance.
        assert!(servable(&FREE, 0));
        assert!(!servable(&FREE, 1));
        assert!(!servable(&FREE, 11), "ex-Pro basculé, 12e signature");
        // Payant : illimité, aucun rang ne coupe.
        assert!(servable(&PRO, 0));
        assert!(servable(&PRO, 999));
    }

    /// Le droit de servir se lit à la requête : la grâce protège, la bascule coupe, sans
    /// dépendre d'une tâche de fond qui aurait ou non réécrit `orgs.plan`.
    #[test]
    fn la_grace_garde_le_gif_en_ligne_la_bascule_le_coupe() {
        use crate::billing::lifecycle::{effective_plan_at, OrgBilling};
        let now = chrono::Utc::now();
        let org = |grace: Option<chrono::DateTime<chrono::Utc>>| OrgBilling {
            plan: "pro".into(),
            subscription_status: Some("past_due".into()),
            grace_until: grace,
        };
        // en grâce : 3e signature d'un Pro impayé, toujours servie
        let en_grace = effective_plan_at(&org(Some(now + chrono::Duration::days(2))), now);
        assert!(servable(&en_grace, 2));
        // grâce échue : Free, seule la plus ancienne survit
        let echue = effective_plan_at(&org(Some(now - chrono::Duration::seconds(1))), now);
        assert!(servable(&echue, 0));
        assert!(!servable(&echue, 2));
    }

    #[test]
    fn etag_matching_handles_lists_and_weak_tags() {
        let mut h = HeaderMap::new();
        h.insert(header::IF_NONE_MATCH, "W/\"abc\", \"def\"".parse().unwrap());
        assert!(matches_etag(&h, "\"def\""));
        assert!(matches_etag(&h, "\"abc\""));
        assert!(!matches_etag(&h, "\"ghi\""));
        assert!(!matches_etag(&HeaderMap::new(), "\"abc\""));
    }

    #[test]
    fn only_known_extensions_get_a_real_content_type() {
        assert_eq!(content_type_of("assets/x/ab.png"), "image/png");
        assert_eq!(content_type_of("assets/x/ab.JPEG"), "image/jpeg");
        for bad in [
            "assets/x/ab.svg",
            "assets/x/ab.html",
            "assets/x/ab",
            "assets/x/ab.exe",
        ] {
            assert_eq!(content_type_of(bad), "application/octet-stream", "{bad}");
        }
    }

    #[test]
    fn click_target_resolves_tokens_and_refuses_everything_else() {
        use crate::doc::Element;
        let el = |id: &str, href: &str| Element {
            id: id.into(),
            href: href.into(),
            ..Default::default()
        };
        let doc = Doc {
            elements: vec![
                el("aaa1111", "https://{{website}}/x"),
                el("bbb2222", "javascript:alert(1)"),
                el("ccc3333", ""),
                Element {
                    hidden: true,
                    ..el("ddd4444", "https://exemple.test")
                },
            ],
            ..Default::default()
        };
        let profile = Profile::from([("website".into(), "ada.dev".into())]);

        assert_eq!(
            click_target(&doc, &profile, "aaa1111").as_deref(),
            Some("https://ada.dev/x")
        );
        for refused in ["bbb2222", "ccc3333", "ddd4444", "inconnu"] {
            assert_eq!(click_target(&doc, &profile, refused), None, "{refused}");
        }
        // jeton absent du profil : l'URL devient "https:///x", pas de redirection devinée
        assert_eq!(
            click_target(&doc, &Profile::new(), "aaa1111").as_deref(),
            Some("https:///x")
        );
    }

    #[test]
    fn client_ip_takes_the_first_hop_only() {
        let mut h = HeaderMap::new();
        h.insert(
            "x-forwarded-for",
            "203.0.113.7, 70.41.3.18".parse().unwrap(),
        );
        assert_eq!(client_ip(&h).as_deref(), Some("203.0.113.7"));
        assert_eq!(client_ip(&HeaderMap::new()), None);
    }
}
