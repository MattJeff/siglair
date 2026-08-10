//! « Colle ton site, récupère ta signature » — contrat §6bis.6.
//!
//! Trois routes, trois niveaux d'exigence :
//!   - `analyze`  : publique, sans compte (c'est l'accroche de la vitrine), donc plafonnée
//!     strictement par IP — elle déclenche une requête sortante depuis notre serveur.
//!   - `generate` : session requise, quota IA vérifié **et** décrémenté avant l'appel payant.
//!   - `pick`     : session requise, crée la signature et rattache le logo à l'organisation.
//!
//! Aucune donnée personnelle ne part chez le fournisseur d'IA (§6bis.3) : `generate` passe la
//! marque publique au modèle et garde le profil pour `compose`, qui s'exécute chez nous.

use std::time::Duration;

use axum::{
    extract::{DefaultBodyLimit, Multipart, State},
    http::{HeaderMap, StatusCode},
    routing::post,
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    ai::{brand, compose, generate, handoff, Brand, Logo, VariantSource},
    auth::{middleware::client_ip, OrgAccess},
    doc::{Doc, Profile},
    error::{AppError, Result},
    plans::{check_signature_quota, Plan},
    render::{html::render_document, RenderMode, RenderOpts},
    util::rate_limit,
    AppState,
};

const HOUR: Duration = Duration::from_secs(3600);
/// §6bis.6 : gratuite et sans compte, mais elle fait sortir une requête de notre réseau.
/// Sans ce plafond, la route est un amplificateur offert à qui la trouve.
const ANALYZE_PER_HOUR: usize = 10;
/// Une génération publique coûte un appel au modèle : plus stricte que l'analyse seule.
const DRAFT_PER_HOUR: usize = 3;
const JOB_DRAFT_PER_HOUR: usize = 5;
const JOB_PREVIEW_PER_HOUR: usize = 300;
const MAX_CV_BYTES: usize = 5 * 1024 * 1024;
/// Le logo d'un visiteur qui n'a pas encore de compte : aucune organisation, purgé après 24 h.
const TEMP_HOURS: i32 = 24;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/onboarding/analyze", post(analyze))
        .route("/onboarding/draft", post(create_draft))
        .route("/onboarding/job-preview", post(preview_job_draft))
        .route(
            "/onboarding/job-draft",
            post(create_job_draft).layer(DefaultBodyLimit::max(MAX_CV_BYTES + 128 * 1024)),
        )
        .route("/onboarding/claim", post(claim_draft))
        .route("/onboarding/generate", post(generate_variants))
        .route("/onboarding/pick", post(pick))
}

// ------------------------------------------------------------------ analyze

#[derive(Deserialize)]
struct AnalyzeReq {
    #[serde(default)]
    url: String,
}

/// `POST /api/onboarding/analyze` — publique.
async fn analyze(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<AnalyzeReq>,
) -> Result<Json<Value>> {
    let ip = client_ip(&headers);
    let key = format!("onboarding:{}", ip.as_deref().unwrap_or("?"));
    if !rate_limit(&key, ANALYZE_PER_HOUR, HOUR) {
        return Err(AppError::RateLimited);
    }

    let mut found = extract_brand(&st, &req.url).await?;

    // Pas de logo : ce n'est pas une erreur, c'est une porte de sortie (DESIGN.md). La marque
    // reste affichable et l'utilisateur ajoutera son logo dans l'éditeur.
    let notice = match found.logo.clone() {
        Some(logo) => match store_temp_logo(&st, &logo).await {
            Ok(id) => {
                found.logo_asset_id = id;
                found.logo_asset_id.is_none().then_some(
                    "Le fichier trouvé à la place du logo n'est pas une image exploitable. \
                         Continuez sans logo, vous l'ajouterez dans l'éditeur.",
                )
            }
            Err(e) => {
                tracing::warn!(error = ?e, site = %found.site, "logo détecté mais non enregistré");
                found.logo_asset_id = None;
                Some(
                    "Logo détecté, mais impossible de l'enregistrer pour l'instant. \
                         Continuez sans logo, vous l'ajouterez dans l'éditeur.",
                )
            }
        },
        None => Some(
            "Aucun logo trouvé sur ce site. Continuez sans logo, vous l'ajouterez dans l'éditeur.",
        ),
    };

    Ok(Json(json!({ "brand": found, "notice": notice })))
}

async fn extract_brand(st: &AppState, raw: &str) -> Result<Brand> {
    let urls = normalize_candidates(raw)?;
    let mut last = None;

    for (i, url) in urls.iter().enumerate() {
        match brand::extract(&st.http, url).await {
            Ok(found) => return Ok(found),
            Err(e) => {
                let e = readable(e);
                if i + 1 < urls.len() && retryable_analysis_error(&e) {
                    last = Some(e);
                    continue;
                }
                return Err(e);
            }
        }
    }

    Err(last.unwrap_or_else(|| {
        AppError::validation(
            "Ce site est injoignable. Vous pourrez continuer sans analyse automatique.",
        )
    }))
}

/// La barre d'accueil reçoit « votreentreprise.com » au moins aussi souvent que l'URL
/// complète. Refuser ça au premier geste demandé au visiteur serait absurde.
fn normalize(raw: &str) -> Result<String> {
    let u = raw.trim();
    if u.is_empty() || u.len() > 2_000 {
        return Err(AppError::validation(
            "Collez l'adresse de votre site, par exemple https://votreentreprise.com.",
        ));
    }
    Ok(if u.contains("://") {
        u.to_string()
    } else {
        format!("https://{u}")
    })
}

/// Quand l'utilisateur colle `acme.fr`, `https://acme.fr` est le bon premier essai. Mais de
/// vrais sites de PME répondent encore seulement en HTTP : on essaie donc les deux, tout en
/// gardant le garde SSRF sur chaque requête.
fn normalize_candidates(raw: &str) -> Result<Vec<String>> {
    let first = normalize(raw)?;
    Ok(if raw.trim().contains("://") {
        vec![first]
    } else {
        vec![first, format!("http://{}", raw.trim())]
    })
}

fn retryable_analysis_error(e: &AppError) -> bool {
    match e {
        AppError::Validation(m) => [
            "injoignable",
            "trop de temps",
            "a répondu",
            "redirections",
            "interrompue",
        ]
        .iter()
        .any(|needle| m.contains(needle)),
        _ => false,
    }
}

/// Le garde SSRF est partagé avec l'éditeur (`doc::check_remote_url`, §8) et ses messages
/// parlent d'« image distante » : exact dans l'éditeur, incompréhensible sous une barre d'URL.
/// On les reformule ici — les deux cas que le contrat veut distincts le restent.
fn readable(e: AppError) -> AppError {
    match e {
        AppError::Validation(m) if m.starts_with("Image distante refusée") => {
            AppError::validation(if m.contains("interne") {
                "Ce domaine est refusé : seuls les sites publics peuvent être analysés."
            } else {
                "Cette adresse n'est pas une URL valide. Attendu : https://votreentreprise.com."
            })
        }
        // « Ce site est injoignable. », « Ce site a répondu 404 »... : déjà écrits pour l'écran
        other => other,
    }
}

/// Le logo devient un asset sans organisation : le visiteur n'a pas encore de compte, et un
/// `Doc` ne porte ni octets ni URL distante (§3). `pick` le rattachera à l'org.
async fn store_temp_logo(st: &AppState, logo: &Logo) -> Result<Option<Uuid>> {
    purge_expired(st).await;

    // Type déduit des octets, jamais d'un en-tête (§8). Même liste fermée que
    // `assets::kind_for` — un SVG déguisé en logo, c'est du XSS distribué par e-mail.
    let Some(det) = infer::get(&logo.bytes).filter(|d| {
        matches!(
            d.mime_type(),
            "image/png" | "image/jpeg" | "image/gif" | "image/webp"
        )
    }) else {
        return Ok(None);
    };

    let id = Uuid::new_v4();
    let key = format!("assets/tmp/{id}.{}", det.extension());
    st.storage
        .put(&key, logo.bytes.clone(), det.mime_type())
        .await?;

    sqlx::query(
        "INSERT INTO assets (id, org_id, kind, filename, content_type, bytes, sha256, \
                             storage_key, width, height, expires_at) \
         VALUES ($1, NULL, 'image', $2, $3, $4, $5, $6, $7, $8, now() + make_interval(hours => $9))",
    )
    .bind(id)
    .bind(format!("logo.{}", det.extension()))
    .bind(det.mime_type())
    .bind(logo.bytes.len() as i64)
    .bind(Sha256::digest(&logo.bytes).to_vec())
    .bind(&key)
    .bind(logo.width as i32)
    .bind(logo.height as i32)
    .bind(TEMP_HOURS)
    .execute(&st.db)
    .await?;
    Ok(Some(id))
}

/// ponytail : purge opportuniste, au fil des analyses — pas de tâche de fond ni de cron pour
/// quelques logos par jour. Le jour où il y a un ordonnanceur, déplacer ces dix lignes dedans.
async fn purge_expired(st: &AppState) {
    let keys = sqlx::query_scalar::<_, String>(
        "DELETE FROM assets WHERE id IN ( \
             SELECT id FROM assets WHERE org_id IS NULL AND expires_at < now() LIMIT 50 \
         ) RETURNING storage_key",
    )
    .fetch_all(&st.db)
    .await;
    let Ok(keys) = keys else { return };
    for key in keys {
        // un objet orphelin sur le disque est moins grave qu'une ligne pointant dans le vide
        if let Err(e) = st.storage.delete(&key).await {
            tracing::warn!(error = ?e, key = %key, "fichier temporaire non supprimé du stockage");
        }
    }
}

// ------------------------------------------------------------------ relais avant connexion

#[derive(Deserialize)]
struct DraftReq {
    #[serde(default)]
    brand: Brand,
}

/// Prépare une vraie signature avant l'inscription. Le document reste 24 h dans le stockage ;
/// seul un jeton signé et court traversera ensuite le magic link.
async fn create_draft(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<DraftReq>,
) -> Result<Json<Value>> {
    let ip = client_ip(&headers);
    let key = format!("onboarding:draft:{}", ip.as_deref().unwrap_or("?"));
    if !rate_limit(&key, DRAFT_PER_HOUR, HOUR) {
        return Err(AppError::RateLimited);
    }

    let mut brand = req.brand;
    if let Some(asset_id) = brand.logo_asset_id {
        let usable: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM assets WHERE id = $1 AND org_id IS NULL \
             AND expires_at > now())",
        )
        .bind(asset_id)
        .fetch_one(&st.db)
        .await?;
        if !usable {
            brand.logo_asset_id = None;
        }
    }

    let mut profile = Profile::new();
    profile.insert("name".into(), "Votre nom".into());
    profile.insert("role".into(), "Votre fonction".into());
    let profile = enriched_profile(&brand, &profile);
    let fallback = compose::fallback_variants(&brand, &profile);
    let out = generate::generate(&st.http, &brand, fallback).await;
    let spec = out
        .variants
        .first()
        .ok_or_else(|| AppError::validation("Aucune direction n'a pu être composée."))?;
    let doc = compose::compose(spec, &brand, &profile);
    doc.validate()?;

    // Les octets du logo sont déjà dans l'asset temporaire. Les dupliquer dans le JSON ferait
    // grossir inutilement le brouillon de plusieurs mégaoctets.
    brand.logo = None;
    let token = handoff::store(
        &st,
        &handoff::Draft {
            signature_id: Uuid::new_v4(),
            brand,
            doc,
            profile,
            name: spec.name.chars().take(120).collect(),
            source: out.source,
            temporary_asset_ids: Vec::new(),
            preserve_profile: false,
        },
    )
    .await?;

    Ok(Json(json!({ "handoff": token, "source": out.source })))
}

#[derive(Deserialize)]
struct JobDraftReq {
    #[serde(default)]
    doc: Doc,
    #[serde(default)]
    profile: Profile,
    #[serde(default)]
    name: String,
}

#[derive(Deserialize)]
struct JobPreviewReq {
    #[serde(default)]
    doc: Doc,
    #[serde(default)]
    profile: Profile,
}

/// Aperçu public strictement limité aux modèles de candidature. Le branding Free est forcé et
/// aucun média arbitraire n'est résolu, ce qui garde `/api/preview` privé pour l'éditeur.
async fn preview_job_draft(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<JobPreviewReq>,
) -> Result<Json<Value>> {
    let ip = client_ip(&headers);
    let key = format!("onboarding:job-preview:{}", ip.as_deref().unwrap_or("?"));
    if !rate_limit(&key, JOB_PREVIEW_PER_HOUR, HOUR) {
        return Err(AppError::RateLimited);
    }

    req.doc.validate()?;
    validate_job_doc(&req.doc)?;
    let profile = validate_job_profile(req.profile)?;
    let opts = RenderOpts {
        public_url: st.cfg.public_url.clone(),
        branding: true,
        ..Default::default()
    };
    Ok(Json(json!({
        "html": render_document(&req.doc, &profile, RenderMode::Freeform, &opts)
    })))
}

/// Verticale candidature : le profil et le document exact sont préparés avant la connexion.
/// Aucun fournisseur IA ne reçoit le CV ou les coordonnées.
async fn create_job_draft(
    State(st): State<AppState>,
    headers: HeaderMap,
    mut form: Multipart,
) -> Result<Json<Value>> {
    let ip = client_ip(&headers);
    let key = format!("onboarding:job-draft:{}", ip.as_deref().unwrap_or("?"));
    if !rate_limit(&key, JOB_DRAFT_PER_HOUR, HOUR) {
        return Err(AppError::RateLimited);
    }

    let mut payload = None;
    let mut cv = None;
    while let Some(field) = form
        .next_field()
        .await
        .map_err(|_| AppError::validation("Le formulaire de candidature est illisible."))?
    {
        match field.name() {
            Some("payload") => {
                payload = Some(field.text().await.map_err(|_| {
                    AppError::validation("Les informations de candidature sont illisibles.")
                })?);
            }
            Some("cv") => {
                let filename = field.file_name().unwrap_or("cv.pdf").to_string();
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|_| AppError::validation("Le CV n'a pas pu être lu."))?
                    .to_vec();
                cv = Some((filename, bytes));
            }
            _ => {}
        }
    }

    let raw = payload.ok_or_else(|| {
        AppError::validation("Les informations de candidature sont obligatoires.")
    })?;
    let mut req: JobDraftReq = serde_json::from_str(&raw)
        .map_err(|_| AppError::validation("Les informations de candidature sont invalides."))?;
    req.doc.validate()?;
    validate_job_doc(&req.doc)?;
    req.profile = validate_job_profile(req.profile)?;

    let mut temporary_asset_ids = Vec::new();
    if let Some((filename, bytes)) = cv {
        let (id, url) = store_temp_cv(&st, &filename, bytes).await?;
        req.profile.insert("cv".into(), url);
        temporary_asset_ids.push(id);
    }

    let name = clean_job_draft_name(&req.name, &req.profile);
    let token = handoff::store(
        &st,
        &handoff::Draft {
            signature_id: Uuid::new_v4(),
            brand: Brand::default(),
            doc: req.doc,
            profile: req.profile,
            name,
            source: VariantSource::Fallback,
            temporary_asset_ids,
            preserve_profile: true,
        },
    )
    .await?;

    Ok(Json(json!({ "handoff": token, "source": "fallback" })))
}

fn clean_job_draft_name(raw: &str, profile: &Profile) -> String {
    let value = raw.trim();
    if !value.is_empty() && value.len() <= 120 && !value.chars().any(char::is_control) {
        return value.to_string();
    }
    let role = profile
        .get("role")
        .map(String::as_str)
        .unwrap_or("Recherche d'emploi");
    format!("Candidature - {role}").chars().take(120).collect()
}

fn validate_job_profile(profile: Profile) -> Result<Profile> {
    const ALLOWED: [&str; 16] = [
        "name",
        "role",
        "email",
        "phone",
        "website",
        "linkedin",
        "tagline",
        "company",
        "portfolio",
        "github",
        "cv",
        "availability",
        "location",
        "university",
        "graduation",
        "whatsapp",
    ];
    const URLS: [&str; 5] = ["website", "linkedin", "portfolio", "github", "cv"];

    if profile.len() > ALLOWED.len() {
        return Err(AppError::validation("Le profil contient trop de champs."));
    }
    let mut out = Profile::new();
    for (key, value) in profile {
        if !ALLOWED.contains(&key.as_str()) {
            return Err(AppError::validation(
                "Un champ du profil n'est pas autorisé.",
            ));
        }
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        if value.len() > 300 || value.chars().any(char::is_control) || value.contains("{{") {
            return Err(AppError::validation(
                "Une valeur du profil est invalide ou trop longue.",
            ));
        }
        if URLS.contains(&key.as_str()) {
            let url = url::Url::parse(value)
                .map_err(|_| AppError::validation("Un lien du profil n'est pas valide."))?;
            if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
                return Err(AppError::validation(
                    "Les liens du profil doivent utiliser HTTP ou HTTPS.",
                ));
            }
        }
        out.insert(key, value.to_string());
    }
    for required in ["name", "role", "email"] {
        if out
            .get(required)
            .is_none_or(|value| value.trim().is_empty())
        {
            return Err(AppError::validation(
                "Nom, métier recherché et email sont obligatoires.",
            ));
        }
    }
    let email = out.get("email").expect("email vérifié juste au-dessus");
    let valid_email = email.len() <= 254
        && !email.contains(char::is_whitespace)
        && matches!(email.split_once('@'), Some((local, domain)) if !local.is_empty() && domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.') && !domain.contains('@'));
    if !valid_email {
        return Err(AppError::validation("L'adresse email n'est pas valide."));
    }
    Ok(out)
}

fn validate_job_doc(doc: &Doc) -> Result<()> {
    const TOKENS: [&str; 16] = [
        "name",
        "role",
        "email",
        "phone",
        "website",
        "linkedin",
        "tagline",
        "company",
        "portfolio",
        "github",
        "cv",
        "availability",
        "location",
        "university",
        "graduation",
        "whatsapp",
    ];
    if !(500.0..=700.0).contains(&doc.canvas.width)
        || !(140.0..=320.0).contains(&doc.canvas.height)
        || !doc.canvas.bg_image.is_empty()
        || doc.elements.len() > 30
        || doc
            .elements
            .iter()
            .any(|element| element.asset_id.is_some())
    {
        return Err(AppError::validation(
            "Ce modèle de candidature n'est pas accepté.",
        ));
    }
    for element in &doc.elements {
        for value in [&element.content, &element.href] {
            let mut cleaned = value.clone();
            for token in TOKENS {
                cleaned = cleaned.replace(&format!("{{{{{token}}}}}"), "");
            }
            if cleaned.contains("{{") || cleaned.contains("}}") {
                return Err(AppError::validation("Le modèle contient un jeton inconnu."));
            }
        }
        let known_token_href = TOKENS
            .iter()
            .any(|token| element.href == format!("{{{{{token}}}}}"));
        if !element.href.is_empty() && !known_token_href && element.href != "mailto:{{email}}" {
            return Err(AppError::validation(
                "Un lien du modèle n'est pas autorisé.",
            ));
        }
    }
    Ok(())
}

async fn store_temp_cv(st: &AppState, filename: &str, bytes: Vec<u8>) -> Result<(Uuid, String)> {
    if bytes.is_empty() || bytes.len() > MAX_CV_BYTES {
        return Err(AppError::validation(
            "Le CV doit être un PDF de 5 Mo maximum.",
        ));
    }
    let detected = infer::get(&bytes)
        .filter(|kind| kind.mime_type() == "application/pdf")
        .ok_or_else(|| AppError::validation("Le CV doit être un véritable fichier PDF."))?;
    purge_expired(st).await;
    let id = Uuid::new_v4();
    let key = format!("assets/tmp/{id}.{}", detected.extension());
    st.storage
        .put(&key, bytes.clone(), "application/pdf")
        .await?;
    let clean_filename: String = filename
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("cv.pdf")
        .chars()
        .filter(|c| !c.is_control())
        .take(120)
        .collect();
    sqlx::query(
        "INSERT INTO assets (id, org_id, kind, filename, content_type, bytes, sha256, \
                             storage_key, expires_at) \
         VALUES ($1, NULL, 'document', $2, 'application/pdf', $3, $4, $5, \
                 now() + make_interval(hours => $6))",
    )
    .bind(id)
    .bind(if clean_filename.is_empty() {
        "cv.pdf"
    } else {
        &clean_filename
    })
    .bind(bytes.len() as i64)
    .bind(Sha256::digest(&bytes).to_vec())
    .bind(&key)
    .bind(TEMP_HOURS)
    .execute(&st.db)
    .await?;
    Ok((id, st.storage.url(&key)))
}

#[derive(Deserialize)]
struct ClaimReq {
    #[serde(default)]
    handoff: String,
}

/// Transforme le brouillon préparé sur la landing en signature ordinaire et ouvre directement
/// l'éditeur. L'identifiant de signature est fixé dans le brouillon : un double appel est donc
/// idempotent et ne consomme pas deux créations.
async fn claim_draft(
    State(st): State<AppState>,
    access: OrgAccess,
    Json(req): Json<ClaimReq>,
) -> Result<(StatusCode, Json<Value>)> {
    let (draft_id, mut draft) = handoff::load(&st, req.handoff.trim()).await?;

    if !draft.preserve_profile {
        if let Some((name,)) =
            sqlx::query_as::<_, (Option<String>,)>("SELECT name FROM users WHERE id = $1")
                .bind(access.user_id)
                .fetch_optional(&st.db)
                .await?
        {
            if let Some(name) = name.filter(|value| !value.trim().is_empty()) {
                draft.profile.insert("name".into(), name);
            }
        }
    }
    let email: String = sqlx::query_scalar("SELECT email::text FROM users WHERE id = $1")
        .bind(access.user_id)
        .fetch_one(&st.db)
        .await?;
    if !draft.preserve_profile {
        draft.profile.insert("email".into(), email.clone());
    } else {
        fill_profile(&mut draft.profile, "email", &email);
    }
    if draft
        .profile
        .get("name")
        .is_none_or(|name| name == "Votre nom" || name.trim().is_empty())
    {
        draft.profile.insert(
            "name".into(),
            email.split('@').next().unwrap_or("Votre nom").to_string(),
        );
    }

    // Un brouillon déjà réclamé renvoie la même signature, même après un double clic.
    if let Some(id) = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM signatures WHERE id = $1 AND org_id = $2 AND owner_user_id = $3 \
         AND deleted_at IS NULL",
    )
    .bind(draft.signature_id)
    .bind(access.org_id)
    .bind(access.user_id)
    .fetch_optional(&st.db)
    .await?
    {
        handoff::delete(&st, draft_id).await;
        return Ok((
            StatusCode::OK,
            Json(json!({ "id": id, "name": draft.name })),
        ));
    }

    let live: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM signatures WHERE org_id = $1 AND deleted_at IS NULL",
    )
    .bind(access.org_id)
    .fetch_one(&st.db)
    .await?;
    check_signature_quota(&access.plan, live.max(0) as u32)?;

    if draft.source == VariantSource::Model {
        consume_ai_generation(&st, &access).await?;
    }
    if let Err(error) = attach_temp_assets(&st, access.org_id, &mut draft.doc).await {
        if draft.source == VariantSource::Model {
            let _ = refund_ai_generation(&st, &access).await;
        }
        return Err(error);
    }
    attach_temporary_documents(&st, access.org_id, &mut draft).await?;

    let inserted = sqlx::query(
        "INSERT INTO signatures (id, org_id, owner_user_id, name, kind, doc, profile) \
         VALUES ($1, $2, $3, $4, 'personal', $5, $6) ON CONFLICT (id) DO NOTHING",
    )
    .bind(draft.signature_id)
    .bind(access.org_id)
    .bind(access.user_id)
    .bind(&draft.name)
    .bind(serde_json::to_value(&draft.doc)?)
    .bind(serde_json::to_value(&draft.profile)?)
    .execute(&st.db)
    .await?
    .rows_affected();

    if inserted == 0 {
        if draft.source == VariantSource::Model {
            let _ = refund_ai_generation(&st, &access).await;
        }
        return Err(AppError::conflict(
            "Cette création a déjà été utilisée par un autre compte.",
        ));
    }

    handoff::delete(&st, draft_id).await;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "id": draft.signature_id, "name": draft.name })),
    ))
}

async fn attach_temporary_documents(
    st: &AppState,
    org_id: Uuid,
    draft: &mut handoff::Draft,
) -> Result<()> {
    for id in draft.temporary_asset_ids.clone() {
        let temp = sqlx::query_as::<_, (Vec<u8>, String)>(
            "SELECT sha256, storage_key FROM assets \
             WHERE id = $1 AND org_id IS NULL AND expires_at > now()",
        )
        .bind(id)
        .fetch_optional(&st.db)
        .await?
        .ok_or_else(|| AppError::validation("Le CV temporaire a expiré. Importez-le à nouveau."))?;

        if let Some(existing_key) = sqlx::query_scalar::<_, String>(
            "SELECT storage_key FROM assets WHERE org_id = $1 AND sha256 = $2",
        )
        .bind(org_id)
        .bind(&temp.0)
        .fetch_optional(&st.db)
        .await?
        {
            draft
                .profile
                .insert("cv".into(), st.storage.url(&existing_key));
            sqlx::query("DELETE FROM assets WHERE id = $1 AND org_id IS NULL")
                .bind(id)
                .execute(&st.db)
                .await?;
            let _ = st.storage.delete(&temp.1).await;
        } else {
            let moved = sqlx::query(
                "UPDATE assets SET org_id = $1, expires_at = NULL \
                 WHERE id = $2 AND org_id IS NULL AND expires_at > now()",
            )
            .bind(org_id)
            .bind(id)
            .execute(&st.db)
            .await?
            .rows_affected();
            if moved != 1 {
                return Err(AppError::validation(
                    "Le CV temporaire n'est plus disponible.",
                ));
            }
        }
    }
    Ok(())
}

// ------------------------------------------------------------------ generate

#[derive(Deserialize)]
struct GenerateReq {
    #[serde(default)]
    brand: Brand,
    /// Reste **ici** : il sert à `compose`, jamais au prompt (§6bis.3).
    #[serde(default)]
    profile: Profile,
}

/// `POST /api/onboarding/generate` — session requise.
async fn generate_variants(
    State(st): State<AppState>,
    access: OrgAccess,
    Json(req): Json<GenerateReq>,
) -> Result<Json<Value>> {
    // Avant l'appel : un appel payé puis refusé pour quota est une perte sèche.
    consume_ai_generation(&st, &access).await?;

    // Le repli (§6bis.5) est calculé ici parce qu'il a besoin du profil pour ne proposer que
    // des CTA réellement disponibles — et que `generate` ne doit surtout pas le recevoir.
    let profile = enriched_profile(&req.brand, &req.profile);
    let fallback = compose::fallback_variants(&req.brand, &profile);
    let out = generate::generate(&st.http, &req.brand, fallback).await;

    // Un repli local n'est pas une génération IA. Il reste utile, mais il ne consomme
    // jamais le quota vendu aux plans payants (ni l'unique essai du plan Free).
    if out.source == VariantSource::Fallback {
        if let Err(error) = refund_ai_generation(&st, &access).await {
            tracing::error!(?error, org_id = %access.org_id, "quota IA non remboursé après repli");
        }
    }

    let mut variants = Vec::with_capacity(out.variants.len());
    for (i, spec) in out.variants.iter().enumerate() {
        let mut doc = compose::compose(spec, &req.brand, &profile);
        // Dès maintenant, pas seulement au `pick` : `/api/preview` ne sert que les assets de
        // l'organisation, et trois propositions sans logo ne montrent plus la marque.
        attach_temp_assets(&st, access.org_id, &mut doc).await?;
        variants.push(json!({
            "index": i,
            "spec": spec, // name, template, palette, ctas, rationale...
            "doc": doc,
        }));
    }

    Ok(Json(json!({ "variants": variants, "source": out.source })))
}

pub(crate) fn enriched_profile(brand: &Brand, profile: &Profile) -> Profile {
    let mut out = profile.clone();
    fill_profile(&mut out, "website", &brand.site);
    fill_profile(&mut out, "company", &brand.name);
    fill_profile(&mut out, "tagline", &brand.tagline);
    if let Some(v) = brand.contacts.get("email") {
        fill_profile(&mut out, "email", v);
    }
    if let Some(v) = brand.contacts.get("phone") {
        fill_profile(&mut out, "phone", v);
    }
    if let Some(v) = brand.contacts.get("whatsapp") {
        fill_profile(&mut out, "whatsapp", v);
    }
    if let Some(v) = brand.socials.get("linkedin") {
        fill_profile(&mut out, "linkedin", v);
    }
    out
}

fn fill_profile(profile: &mut Profile, key: &str, value: &str) {
    if value.trim().is_empty() || profile.get(key).is_some_and(|v| !v.trim().is_empty()) {
        return;
    }
    profile.insert(key.to_string(), value.trim().to_string());
}

/// Quota IA (§6bis.6) : contrôle **et** décrément dans un seul `UPDATE`. En deux requêtes,
/// deux onglets consomment la même génération gratuite — et c'est exactement ce que fait un
/// double-clic sur « Générer ».
///
/// La remise à zéro mensuelle des plans payants tient dans le même ordre : pas de tâche
/// planifiée pour un compteur qu'on lit une fois par génération.
pub(crate) async fn consume_ai_generation(st: &AppState, access: &OrgAccess) -> Result<()> {
    let Some(max) = access.plan.ai_generations else {
        return Ok(());
    };

    // « le compteur repart » = plan mensuel ET échéance passée (ou jamais posée).
    let rolled = "$2::bool AND (ai_quota_reset_at IS NULL OR ai_quota_reset_at <= now())";
    let n = sqlx::query(&format!(
        "UPDATE orgs SET \
           ai_generations_used = CASE WHEN {rolled} THEN 1 ELSE ai_generations_used + 1 END, \
           ai_quota_reset_at = CASE WHEN {rolled} \
               THEN date_trunc('month', now()) + interval '1 month' ELSE ai_quota_reset_at END \
         WHERE id = $1 \
           AND (CASE WHEN {rolled} THEN 0 ELSE ai_generations_used END) < $3::int"
    ))
    .bind(access.org_id)
    .bind(access.plan.ai_generations_monthly)
    .bind(max as i32)
    .execute(&st.db)
    .await?
    .rows_affected();

    if n == 0 {
        return Err(AppError::QuotaExceeded(quota_message(&access.plan)));
    }
    Ok(())
}

pub(crate) async fn refund_ai_generation(st: &AppState, access: &OrgAccess) -> Result<()> {
    if access.plan.ai_generations.is_none() {
        return Ok(());
    }
    sqlx::query(
        "UPDATE orgs SET ai_generations_used = greatest(ai_generations_used - 1, 0) WHERE id = $1",
    )
    .bind(access.org_id)
    .execute(&st.db)
    .await?;
    Ok(())
}

/// §6bis.6 : « Envie d'une autre direction ? » — jamais « quota dépassé ». On fait payer
/// celui qui a déjà vu ce que le système sait faire de sa marque, on ne le punit pas.
fn quota_message(plan: &Plan) -> String {
    if plan.ai_generations_monthly {
        "Vous avez utilisé toutes vos générations de ce mois-ci ; elles reviennent le 1er. \
         D'ici là, l'éditeur reste entièrement à vous."
            .to_string()
    } else {
        format!(
            "Envie d'une autre direction ? Passez à Pro ({prix}/mois) : {n} générations par \
             mois, le GIF hébergé à votre URL et le suivi des ouvertures.",
            prix = crate::plans::PRO.price_label(),
            n = crate::plans::PRO.ai_generations.unwrap_or(0),
        )
    }
}

// ------------------------------------------------------------------ pick

#[derive(Deserialize)]
struct PickReq {
    #[serde(default)]
    variant_index: usize,
    #[serde(default)]
    doc: Doc,
    #[serde(default)]
    profile: Profile,
    name: Option<String>,
}

/// `POST /api/onboarding/pick` — session requise. Renvoie l'id pour que le front ouvre
/// l'éditeur : à partir d'ici, la proposition est une signature ordinaire (§6bis).
async fn pick(
    State(st): State<AppState>,
    access: OrgAccess,
    Json(req): Json<PickReq>,
) -> Result<(StatusCode, Json<Value>)> {
    if req.variant_index > 2 {
        return Err(AppError::validation(
            "Proposition inconnue : choisissez l'une des trois.",
        ));
    }
    let mut doc = req.doc;
    doc.validate()?;

    let live: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM signatures WHERE org_id = $1 AND deleted_at IS NULL",
    )
    .bind(access.org_id)
    .fetch_one(&st.db)
    .await?;
    check_signature_quota(&access.plan, live.max(0) as u32)?;

    attach_temp_assets(&st, access.org_id, &mut doc).await?;

    let name = match req.name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
        Some(n) => n.chars().take(120).collect(),
        None => format!("Proposition {}", req.variant_index + 1),
    };

    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO signatures (id, org_id, owner_user_id, name, kind, doc, profile) \
         VALUES ($1, $2, $3, $4, 'personal', $5, $6)",
    )
    .bind(id)
    .bind(access.org_id)
    .bind(access.user_id)
    .bind(&name)
    .bind(serde_json::to_value(&doc)?)
    .bind(serde_json::to_value(&req.profile)?)
    .execute(&st.db)
    .await?;

    Ok((StatusCode::CREATED, Json(json!({ "id": id, "name": name }))))
}

/// Le logo enregistré par `analyze` n'appartenait à personne : le visiteur n'avait pas encore
/// de compte. Il rejoint l'organisation et cesse d'expirer. Idempotent — un asset déjà
/// rattaché (le sien ou celui d'une autre org) n'est pas touché.
pub(crate) async fn attach_temp_assets(st: &AppState, org_id: Uuid, doc: &mut Doc) -> Result<()> {
    for el in &mut doc.elements {
        let Some(id) = el.asset_id else { continue };
        let res = sqlx::query(
            "UPDATE assets SET org_id = $1, expires_at = NULL WHERE id = $2 AND org_id IS NULL",
        )
        .bind(org_id)
        .bind(id)
        .execute(&st.db)
        .await;

        match res {
            Ok(_) => {}
            // `unique (org_id, sha256)` : l'organisation possède déjà ce logo — deuxième
            // passage dans l'onboarding sur le même site. On pointe l'élément vers l'asset
            // existant et on laisse le temporaire expirer de lui-même.
            Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
                el.asset_id = sqlx::query_scalar(
                    "SELECT a.id FROM assets a JOIN assets t ON t.sha256 = a.sha256 \
                     WHERE t.id = $1 AND a.org_id = $2",
                )
                .bind(id)
                .bind(org_id)
                .fetch_optional(&st.db)
                .await?
                .or(el.asset_id);
            }
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_url_sans_schema_reste_utilisable() {
        assert_eq!(
            normalize(" votreentreprise.com ").unwrap(),
            "https://votreentreprise.com"
        );
        assert_eq!(normalize("http://acme.fr").unwrap(), "http://acme.fr");
        assert!(normalize("  ").is_err());
        // pas de repli silencieux sur https:// pour un schéma exotique : le garde SSRF tranche
        assert_eq!(
            normalize("file:///etc/passwd").unwrap(),
            "file:///etc/passwd"
        );
    }

    #[test]
    fn une_url_sans_schema_tente_https_puis_http() {
        assert_eq!(
            normalize_candidates(" acme.fr ").unwrap(),
            ["https://acme.fr", "http://acme.fr"],
        );
        assert_eq!(
            normalize_candidates("https://acme.fr").unwrap(),
            ["https://acme.fr"]
        );
        assert_eq!(
            normalize_candidates("http://acme.fr").unwrap(),
            ["http://acme.fr"]
        );
    }

    #[test]
    fn seuls_les_echecs_reseau_declenchent_le_repli_http() {
        assert!(retryable_analysis_error(&AppError::validation(
            "Ce site est injoignable."
        )));
        assert!(retryable_analysis_error(&AppError::validation(
            "Ce site a répondu 403 — impossible de l'analyser."
        )));
        assert!(!retryable_analysis_error(&AppError::validation(
            "Ce domaine est refusé : seuls les sites publics peuvent être analysés."
        )));
        assert!(!retryable_analysis_error(&AppError::RateLimited));
    }

    /// Les quatre échecs du contrat §6bis.6 doivent rester distincts et lisibles sous une
    /// barre d'URL — pas « Image distante refusée », qui est le vocabulaire de l'éditeur.
    #[test]
    fn les_echecs_sont_distincts_et_en_francais() {
        let msg = |e: AppError| match e {
            AppError::Validation(m) => m,
            other => panic!("attendu une validation, reçu {other:?}"),
        };
        let refuse = |why: &str| {
            msg(readable(AppError::validation(format!(
                "Image distante refusée : {why}."
            ))))
        };

        let interne = refuse("cette adresse est interne");
        let invalide = refuse("URL invalide");
        assert!(interne.contains("domaine est refusé"), "{interne}");
        assert!(invalide.contains("URL valide"), "{invalide}");
        assert_ne!(interne, invalide);

        // ce qui vient de brand.rs est déjà écrit pour l'utilisateur : on n'y touche pas
        let tel_quel = AppError::validation("Ce site est injoignable.");
        assert_eq!(msg(readable(tel_quel)), "Ce site est injoignable.");
    }

    #[test]
    fn le_message_de_quota_vend_au_lieu_de_punir() {
        let free = quota_message(&crate::plans::FREE);
        assert!(free.contains("Envie d'une autre direction"), "{free}");
        assert!(!free.to_lowercase().contains("dépassé"), "{free}");
        // un plan payant n'a rien à acheter de plus : on lui dit quand ça revient
        assert!(quota_message(&crate::plans::PRO).contains("le 1er"));
    }

    #[test]
    fn le_profil_reutilise_les_signaux_publics_sans_ecraser_l_utilisateur() {
        let brand = Brand {
            site: "https://acme.test/".into(),
            name: "Acme".into(),
            tagline: "Robots utiles".into(),
            contacts: [
                ("email".into(), "contact@acme.test".into()),
                ("phone".into(), "+33123456789".into()),
                ("whatsapp".into(), "33600000000".into()),
            ]
            .into(),
            socials: [(
                "linkedin".into(),
                "https://linkedin.com/company/acme".into(),
            )]
            .into(),
            ..Default::default()
        };
        let profile = Profile::from([("email".into(), "moi@example.com".into())]);

        let out = enriched_profile(&brand, &profile);
        assert_eq!(
            out.get("email").map(String::as_str),
            Some("moi@example.com")
        );
        assert_eq!(out.get("phone").map(String::as_str), Some("+33123456789"));
        assert_eq!(
            out.get("website").map(String::as_str),
            Some("https://acme.test/")
        );
        assert_eq!(
            out.get("linkedin").map(String::as_str),
            Some("https://linkedin.com/company/acme")
        );
    }

    #[test]
    fn le_profil_candidat_n_accepte_que_les_champs_et_urls_prevus() {
        let valid = Profile::from([
            ("name".into(), "Lucas Martin".into()),
            ("role".into(), "Développeur React".into()),
            ("email".into(), "lucas@example.com".into()),
            ("github".into(), "https://github.com/lucas".into()),
            ("availability".into(), "Dès septembre".into()),
        ]);
        assert!(validate_job_profile(valid).is_ok());

        let unknown = Profile::from([
            ("name".into(), "Lucas".into()),
            ("role".into(), "Designer".into()),
            ("email".into(), "lucas@example.com".into()),
            ("admin".into(), "true".into()),
        ]);
        assert!(validate_job_profile(unknown).is_err());

        let unsafe_url = Profile::from([
            ("name".into(), "Lucas".into()),
            ("role".into(), "Designer".into()),
            ("email".into(), "lucas@example.com".into()),
            ("portfolio".into(), "javascript:alert(1)".into()),
        ]);
        assert!(validate_job_profile(unsafe_url).is_err());

        let invalid_email = Profile::from([
            ("name".into(), "Lucas".into()),
            ("role".into(), "Designer".into()),
            ("email".into(), "lucas@localhost".into()),
        ]);
        assert!(validate_job_profile(invalid_email).is_err());
    }

    #[test]
    fn le_modele_candidat_refuse_les_jetons_et_liens_arbitraires() {
        let mut valid = Doc::default();
        valid.elements.push(crate::doc::Element {
            id: "profil1".into(),
            content: "{{name}} · {{role}}".into(),
            href: "{{portfolio}}".into(),
            ..Default::default()
        });
        assert!(validate_job_doc(&valid).is_ok());

        let mut unknown_token = valid.clone();
        unknown_token.elements[0].content = "{{secret}}".into();
        assert!(validate_job_doc(&unknown_token).is_err());

        let mut arbitrary_link = valid;
        arbitrary_link.elements[0].href = "https://attacker.example".into();
        assert!(validate_job_doc(&arbitrary_link).is_err());
    }

    #[test]
    fn le_nom_du_brouillon_candidat_est_borne_et_a_un_repli() {
        let profile = Profile::from([("role".into(), "Product Designer".into())]);
        assert_eq!(
            clean_job_draft_name("  Candidature design  ", &profile),
            "Candidature design"
        );
        assert_eq!(
            clean_job_draft_name("", &profile),
            "Candidature - Product Designer"
        );
        assert!(clean_job_draft_name(&"x".repeat(500), &profile).len() <= 120);
    }
}
