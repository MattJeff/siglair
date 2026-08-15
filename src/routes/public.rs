//! Routes publiques — contrat §5.1. C'est ce que les clients mail appellent, des millions
//! de fois, sans session et sans indulgence.
//!
//! Deux règles tiennent tout le fichier :
//! 1. rien de lent dans le chemin de la réponse — l'écriture de l'événement part en tâche
//!    détachée, une image qui met 400 ms à s'afficher est une image cassée ;
//! 2. la cible d'un `/c/...` vient du document en base, jamais de la requête — sinon
//!    Siglair devient un redirecteur ouvert exploitable pour du phishing depuis son
//!    propre domaine.

use std::fmt::Write as _;
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
    util::{esc_html, hash_ip, is_machine, ua_family},
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
        .route("/v/{slug}", get(verify))
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

    // La redirection est servie à TOUT LE MONDE, robots compris : un scanner qui reçoit une
    // erreur classe le lien comme suspect, et c'est la signature entière qui devient douteuse.
    // On décide seulement s'il faut le COMPTER.
    let machine = is_machine(
        headers
            .get(header::USER_AGENT)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default(),
    );
    if row.analytics_enabled && !machine {
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

// ------------------------------------------------------------------ vérification

#[derive(sqlx::FromRow)]
struct VerifyRow {
    profile: Option<Value>,
    verify_link: bool,
    verify_notice: Option<String>,
    org_name: String,
    published_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// `GET /v/{slug}` — « cet e-mail vient-il vraiment de cette personne ? »
///
/// Une page publique, en HTML rendu par le serveur, SANS formulaire, sans champ, sans
/// javascript, sans cookie et sans rien à envoyer. Ce vide est la fonctionnalité : une page
/// de vérification qui demanderait quoi que ce soit serait indiscernable d'un hameçonnage,
/// et détruirait la confiance qu'elle prétend établir.
///
/// Les données viennent de l'INSTANTANÉ publié (`renders.profile`), jamais du brouillon en
/// cours d'édition — même règle que `/c/`. Sinon une modification non publiée changerait ce
/// que la page affirme au sujet d'e-mails déjà partis.
///
/// Ce qu'on N'AFFICHE PAS, volontairement : aucune adresse du destinataire, aucun contenu de
/// message, aucune date d'envoi. La page ne sait rien de qui la consulte et ne doit rien en
/// apprendre — elle ne fait que republier ce que l'employeur a lui-même publié.
async fn verify(State(st): State<AppState>, Path(slug): Path<String>) -> Response {
    if slug.is_empty() || slug.len() > 64 {
        return not_verifiable();
    }
    let row: Option<VerifyRow> = match sqlx::query_as(
        "SELECT r.profile, o.verify_link, o.verify_notice, o.name AS org_name, \
                r.created_at AS published_at \
         FROM signatures s \
         JOIN orgs o ON o.id = s.org_id \
         LEFT JOIN renders r ON r.id = s.published_render_id \
         WHERE s.public_slug = $1::citext AND s.deleted_at IS NULL",
    )
    .bind(&slug)
    .fetch_optional(&st.db)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = ?e, %slug, "page de vérification indisponible");
            return not_verifiable();
        }
    };

    // Org qui n'a pas activé le lien, signature inconnue, ou jamais publiée : la MÊME réponse
    // dans les trois cas. Distinguer « ce slug n'existe pas » de « cette org n'a pas activé
    // l'option » donnerait à un attaquant de quoi énumérer les clients de Siglair.
    let Some(row) = row.filter(|r| r.verify_link && r.profile.is_some()) else {
        return not_verifiable();
    };
    let profile: Profile = row
        .profile
        .and_then(|p| serde_json::from_value(p).ok())
        .unwrap_or_default();

    let champ = |k: &str| profile.get(k).map(String::as_str).unwrap_or("").trim();
    let nom = champ("name");
    if nom.is_empty() {
        return not_verifiable();
    }
    let structure = {
        let c = champ("company");
        if c.is_empty() { row.org_name.as_str() } else { c }
    };

    let mut contacts = String::new();
    for (etiquette, cle) in [("E-mail", "email"), ("Téléphone", "phone"), ("Site", "website")] {
        let v = champ(cle);
        if v.is_empty() {
            continue;
        }
        let _ = write!(
            contacts,
            "<div class=l><dt>{etiquette}</dt><dd>{}</dd></div>",
            esc_html(v)
        );
    }

    // La date de dernière publication est le seul élément qui distingue une page vivante
    // d'une page fabriquée une fois : elle dit que l'employeur tient encore cette fiche.
    let maj = row
        .published_at
        .map(|d| format!("Mise à jour par {} le {}.", esc_html(structure), d.format("%d/%m/%Y")))
        .unwrap_or_default();

    let avertissement = row
        .verify_notice
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(|n| format!("<p class=w><b>Ce que {} ne vous demandera jamais par e-mail</b><br>{}</p>", esc_html(structure), esc_html(n)))
        .unwrap_or_default();

    page(
        StatusCode::OK,
        &format!(
            "<h1>{}</h1><p class=s>{}</p><dl>{contacts}</dl><p class=d>{maj}</p>{avertissement}",
            esc_html(nom),
            esc_html(champ("role")),
        ),
    )
}

/// La réponse quand rien ne peut être confirmé. Elle ne dit PAS « ce compte n'existe pas » :
/// elle dit qu'on ne peut rien confirmer, ce qui est la seule chose vraie et la seule utile.
/// Un 404 nu laisserait croire à une page cassée, alors que l'absence de confirmation EST
/// l'information — c'est exactement ce que le lecteur doit retenir.
fn not_verifiable() -> Response {
    page(
        StatusCode::NOT_FOUND,
        "<h1>Impossible de confirmer</h1><p class=s>Aucune signature vérifiable ne correspond à \
         cette adresse.</p><p class=w>Cela ne prouve pas qu'il s'agit d'une fraude — mais si ce \
         message vous demande un virement, un changement de coordonnées bancaires ou un \
         identifiant, appelez votre correspondant à un numéro que vous connaissez déjà, jamais \
         à celui écrit dans le message.</p>",
    )
}

/// Le gabarit. Un seul fichier, aucun asset, aucune requête réseau supplémentaire : la page
/// doit s'afficher entièrement même derrière un pare-feu d'entreprise qui bloque tout le reste.
fn page(code: StatusCode, corps: &str) -> Response {
    let html = format!(
        "<!doctype html><html lang=fr><head><meta charset=utf-8>\
         <meta name=viewport content=\"width=device-width,initial-scale=1\">\
         <meta name=robots content=\"noindex\">\
         <title>Vérification — Siglair</title><style>\
         :root{{color-scheme:light dark}}\
         body{{margin:0;padding:48px 20px;font:16px/1.6 -apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Arial,sans-serif;background:#f5f6f8;color:#0a1220}}\
         main{{max-width:34rem;margin:auto;background:#fff;border:1px solid #dfe3ea;border-radius:14px;padding:32px}}\
         h1{{margin:0 0 4px;font-size:26px;line-height:1.2}}\
         .s{{margin:0 0 22px;color:#59657a}}\
         dl{{margin:0;display:grid;gap:10px}}.l{{display:flex;gap:12px}}\
         dt{{flex:0 0 88px;color:#59657a;font-size:14px}}dd{{margin:0;overflow-wrap:anywhere}}\
         .d{{margin:22px 0 0;font-size:14px;color:#59657a}}\
         .w{{margin:22px 0 0;padding:14px 16px;background:#fff7ed;border:1px solid #fed7aa;border-radius:10px;font-size:15px}}\
         footer{{max-width:34rem;margin:18px auto 0;font-size:13px;color:#59657a;text-align:center}}\
         footer a{{color:inherit}}\
         @media(prefers-color-scheme:dark){{body{{background:#07111f;color:#e7edf6}}\
         main{{background:#0c1727;border-color:#1d2b40}}.s,dt,.d,footer{{color:#93a1b5}}\
         .w{{background:#2a1c0b;border-color:#7c4a12}}}}\
         </style></head><body><main>{corps}</main>\
         <footer>Fiche publiée par l'employeur et vérifiée par \
         <a href=\"https://siglair.com/\">Siglair</a>. Cette page ne vous demande jamais rien.</footer>\
         </body></html>"
    );
    (
        code,
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            // Elle doit refléter une republication rapidement : c'est sa fraîcheur qui fait
            // sa valeur. 5 minutes, comme l'image.
            (header::CACHE_CONTROL, CACHE),
            // Une fiche de vérification n'a rien à faire dans un moteur de recherche :
            // elle ne se lit qu'en réponse à un e-mail qu'on tient déjà.
            (header::HeaderName::from_static("x-robots-tag"), "noindex"),
        ],
        html,
    )
        .into_response()
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
    let content_type = content_type_of(&key);
    let disposition = if content_type == "application/pdf" {
        "attachment"
    } else {
        "inline"
    };
    Ok((
        [
            (header::CONTENT_TYPE, content_type.to_string()),
            // la clé contient le sha256 du contenu : le média ne changera jamais
            (
                header::CACHE_CONTROL,
                "public, max-age=31536000, immutable".to_string(),
            ),
            (header::ETAG, etag),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_string()),
            (header::CONTENT_DISPOSITION, disposition.to_string()),
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
        Some("pdf") => "application/pdf",
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
        // Déduplication à 10 secondes, sur les CLICS UNIQUEMENT.
        //
        // Un scanner d'URL en ouvre plusieurs d'affilée sur le même bouton ; un humain qui
        // double-clique en produit deux. Dans les deux cas c'est un clic.
        //
        // Surtout : ne PAS l'appliquer aux ouvertures. Une ouverture passe par le proxy
        // d'images de Gmail, donc l'`ip_hash` enregistré est celui d'un serveur de Google, pas
        // du lecteur. Dédupliquer dessus fondrait DEUX DESTINATAIRES DIFFÉRENTS en un seul dès
        // qu'ils ouvrent le message à quelques secondes d'intervalle — une sous-estimation
        // silencieuse, et d'autant plus forte que la signature est diffusée largement.
        // La première version le faisait ; le parcours complet l'a attrapé en constatant une
        // ouverture au lieu de deux (le GIF et le PNG de repli, récupérés coup sur coup).
        //
        // `IS NOT DISTINCT FROM` et non `=` : `ip_hash` est NULL quand l'IP est absente, et
        // `NULL = NULL` vaut NULL — la clause serait toujours fausse et ne dédupliquerait rien.
        let res = sqlx::query(
            "INSERT INTO events (signature_id, kind, element_id, target_host, ip_hash, ua_family) \
             SELECT $1, $2, $3, $4, $5, $6 \
             WHERE $2 <> 'click' OR NOT EXISTS ( \
               SELECT 1 FROM events \
               WHERE signature_id = $1 AND kind = 'click' \
                 AND element_id IS NOT DISTINCT FROM $3 \
                 AND ip_hash IS NOT DISTINCT FROM $5 \
                 AND occurred_at > now() - interval '10 seconds')",
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
        assert_eq!(content_type_of("assets/x/cv.pdf"), "application/pdf");
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
