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
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    ai::{brand, compose, generate, Brand, Logo},
    auth::{middleware::client_ip, OrgAccess},
    doc::{Doc, Profile},
    error::{AppError, Result},
    plans::{check_signature_quota, Plan},
    util::rate_limit,
    AppState,
};

const HOUR: Duration = Duration::from_secs(3600);
/// §6bis.6 : gratuite et sans compte, mais elle fait sortir une requête de notre réseau.
/// Sans ce plafond, la route est un amplificateur offert à qui la trouve.
const ANALYZE_PER_HOUR: usize = 10;
/// Le logo d'un visiteur qui n'a pas encore de compte : aucune organisation, purgé après 24 h.
const TEMP_HOURS: i32 = 24;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/onboarding/analyze", post(analyze))
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
            tracing::warn!(error = ?e, key = %key, "logo temporaire non supprimé du stockage");
        }
    }
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

fn enriched_profile(brand: &Brand, profile: &Profile) -> Profile {
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
async fn consume_ai_generation(st: &AppState, access: &OrgAccess) -> Result<()> {
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

/// §6bis.6 : « Envie d'une autre direction ? » — jamais « quota dépassé ». On fait payer
/// celui qui a déjà vu ce que le système sait faire de sa marque, on ne le punit pas.
fn quota_message(plan: &Plan) -> String {
    if plan.ai_generations_monthly {
        "Vous avez utilisé toutes vos générations de ce mois-ci ; elles reviennent le 1er. \
         D'ici là, l'éditeur reste entièrement à vous."
            .to_string()
    } else {
        "Envie d'une autre direction ? Passez à Pro (7,90 €/mois) : 30 générations par mois, \
         le GIF hébergé à votre URL et le suivi des ouvertures."
            .to_string()
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
async fn attach_temp_assets(st: &AppState, org_id: Uuid, doc: &mut Doc) -> Result<()> {
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
}
