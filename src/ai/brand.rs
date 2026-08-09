//! Extraction de marque depuis une URL — contrat §6bis.2.
//!
//! **Aucun appel de modèle ici.** C'est du parsing HTML et du traitement d'image : le
//! résultat doit être reproductible, testable hors ligne et gratuit. L'IA intervient un cran
//! plus loin (`generate.rs`), et uniquement sur ce que ce module a trouvé.
//!
//! C'est aussi le point le plus dangereux du produit : on récupère une URL fournie par un
//! inconnu, depuis notre serveur. Tout ce qui sort d'ici passe par [`fetch`], qui revalide
//! **chaque** saut avec [`check_remote_url`] et plafonne le corps et le temps.

use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::Duration;

use percent_encoding::percent_decode_str;
use reqwest::header::{ACCEPT, ACCEPT_ENCODING, ACCEPT_LANGUAGE, CONTENT_TYPE, LOCATION};
use url::Url;

use crate::{
    doc::check_remote_url,
    error::{AppError, Result},
};

// Les types partagés vivent dans `super` (src/ai/mod.rs) ; ils sont réexportés ici parce que
// `crate::ai::brand::Brand` est le chemin naturel pour qui lit « la marque extraite ».
pub use super::{ensure_contrast, Brand, Color, Logo};

/// 10 s pour l'extraction entière — page, manifeste et logo compris (§6bis.2).
const TOTAL_BUDGET: Duration = Duration::from_secs(10);
const MAX_HTML: usize = 5 * 1024 * 1024;
/// Un logo de plus de 2 Mo n'est pas un logo. Le plafond limite aussi le base64 renvoyé au front.
const MAX_ICON: usize = 2 * 1024 * 1024;
const MAX_HOPS: usize = 3;
/// On préfère un vrai logo confortable, mais certains sites protégés ne livrent qu'une icône
/// Apple/Favicon proprement carrée. En dessous de 24 px, l'étirement devient visible.
const PREFERRED_LOGO_PX: u32 = 64;
const USABLE_LOGO_PX: u32 = 48;
const MIN_LOGO_PX: u32 = 24;
const MAX_CANDIDATES: usize = 24;
const MAX_COLORS: usize = 6;

/// Analyse un site public et en tire sa marque.
///
/// Le paramètre `http` est le client partagé d'`AppState` ; il **n'est pas** utilisé pour
/// aller chercher la page : ce client suit les redirections tout seul (jusqu'à 10 sauts,
/// sans revalidation), ce qui rouvrirait exactement le trou SSRF que ce module ferme. On
/// utilise un client dédié en `redirect::Policy::none()`.
/// ponytail : le jour où `AppState::new` construit son client ainsi, supprimer [`fetcher`]
/// et se servir du paramètre — la signature n'a pas à changer d'ici là.
pub async fn extract(_http: &reqwest::Client, url: &str) -> Result<Brand> {
    tokio::time::timeout(TOTAL_BUDGET, analyze(url))
        .await
        .unwrap_or_else(|_| {
            Err(AppError::validation(
                "Ce site met trop de temps à répondre.",
            ))
        })
}

async fn analyze(url: &str) -> Result<Brand> {
    let page = match fetch(url, MAX_HTML).await {
        Ok(page) => page,
        Err(e) if can_fallback_to_domain(&e) => Fetched {
            url: guard(url.to_string()).await?,
            bytes: Vec::new(),
        },
        Err(e) => return Err(e),
    };

    let html = String::from_utf8_lossy(&page.bytes);
    analyze_page(page.url, &html).await
}

async fn analyze_page(base: Url, html: &str) -> Result<Brand> {
    let page = Page::new(html);
    let (logo, logo_colors) = match pick_logo(&page, &base).await {
        Some((l, c)) => (Some(l), c),
        None => (None, Vec::new()),
    };

    Ok(Brand {
        name: brand_name(&page, &base),
        tagline: tagline(&page),
        colors: colors(&page, logo_colors),
        socials: socials(&page, &base),
        contacts: contacts(&page, &base),
        font: font(&page),
        site: base.to_string(),
        logo,
        // rempli par la route, une fois le logo enregistré dans `assets`
        logo_asset_id: None,
    })
}

fn can_fallback_to_domain(e: &AppError) -> bool {
    match e {
        AppError::Validation(m) => [
            "injoignable",
            "trop de temps",
            "a répondu 401",
            "a répondu 403",
            "a répondu 406",
            "a répondu 408",
            "a répondu 409",
            "a répondu 425",
            "a répondu 429",
            "a répondu 500",
            "a répondu 502",
            "a répondu 503",
            "a répondu 504",
            "interrompue",
        ]
        .iter()
        .any(|needle| m.contains(needle)),
        _ => false,
    }
}

// ------------------------------------------------------------------ récupération réseau

/// Client dédié à l'extraction : aucune redirection suivie automatiquement, aucun cookie
/// (la feature n'est même pas compilée), aucun `Referer`.
fn fetcher() -> &'static reqwest::Client {
    static C: OnceLock<reqwest::Client> = OnceLock::new();
    C.get_or_init(|| {
        reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(TOTAL_BUDGET)
            .connect_timeout(Duration::from_secs(4))
            .referer(false)
            .user_agent(
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
                 AppleWebKit/537.36 (KHTML, like Gecko) \
                 Chrome/125.0.0.0 Safari/537.36",
            )
            .build()
            .expect("client HTTP d'extraction")
    })
}

struct Fetched {
    /// URL réellement servie, après redirections — base des URL relatives.
    url: Url,
    bytes: Vec<u8>,
}

/// `check_remote_url` fait une résolution DNS bloquante : l'appeler tel quel depuis une tâche
/// async immobiliserait l'exécuteur le temps du DNS d'un hôte choisi par l'attaquant.
async fn guard(raw: String) -> Result<Url> {
    tokio::task::spawn_blocking(move || check_remote_url(&raw))
        .await
        .map_err(|e| AppError::Internal(e.into()))?
}

/// Résout `location` contre l'URL courante **et revalide** : un 302 vers 169.254.169.254
/// traverse un garde qui ne vérifie que l'URL de départ.
async fn next_hop(current: &Url, location: &str) -> Result<Url> {
    let joined = current
        .join(location.trim())
        .map_err(|_| AppError::validation("Ce site renvoie une redirection invalide."))?;
    guard(joined.to_string()).await
}

/// GET plafonné : 3 redirections revalidées une par une, `cap` octets comptés au fil du flux.
async fn fetch(raw: &str, cap: usize) -> Result<Fetched> {
    // ponytail : rebinding DNS possible entre ce contrôle et la connexion de reqwest (la
    // politique du contrat, « valider à l'enregistrement et au rendu », l'accepte). Fermer
    // la fenêtre demanderait de résoudre une fois puis de connecter par IP avec un en-tête
    // Host — `ClientBuilder::resolve`, donc un client par hôte : à faire si ça devient réel.
    let mut target = guard(raw.to_string()).await?;

    for _ in 0..=MAX_HOPS {
        let resp = fetcher()
            .get(target.clone())
            .header(
                ACCEPT,
                "text/html,application/xhtml+xml,image/*,application/json;q=0.8,*/*;q=0.5",
            )
            .header(ACCEPT_LANGUAGE, "fr-FR,fr;q=0.9,en;q=0.7")
            // pas de gzip/br : une bombe de décompression coûte moins cher à l'attaquant
            // qu'à nous, et le plafond de 5 Mo ne porterait que sur le flux compressé.
            .header(ACCEPT_ENCODING, "identity")
            .send()
            .await
            .map_err(|_| AppError::validation("Ce site est injoignable."))?;

        if resp.status().is_redirection() {
            let loc = resp
                .headers()
                .get(LOCATION)
                .and_then(|v| v.to_str().ok())
                .ok_or_else(|| AppError::validation("Ce site renvoie une redirection vide."))?
                .to_string();
            target = next_hop(&target, &loc).await?;
            continue;
        }
        if !resp.status().is_success() {
            return Err(AppError::validation(format!(
                "Ce site a répondu {} — impossible de l'analyser.",
                resp.status().as_u16()
            )));
        }
        // le Content-Type déclaré n'est jamais cru (§8) ; il ne sert qu'à écarter tôt un
        // téléchargement manifestement hors sujet.
        if let Some(ct) = resp
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
        {
            let ct = ct.to_ascii_lowercase();
            if ct.starts_with("video/") || ct.starts_with("audio/") {
                return Err(AppError::validation(
                    "Cette URL ne pointe pas vers une page web.",
                ));
            }
        }
        return Ok(Fetched {
            url: target,
            bytes: read_capped(resp, cap).await?,
        });
    }
    Err(AppError::validation("Ce site fait trop de redirections."))
}

/// On compte les octets reçus : `Content-Length` est déclaratif, donc sans valeur.
async fn read_capped(mut resp: reqwest::Response, cap: usize) -> Result<Vec<u8>> {
    let mut buf = Vec::with_capacity(64 * 1024);
    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|_| AppError::validation("La réponse de ce site a été interrompue."))?
    {
        if buf.len() + chunk.len() > cap {
            return Err(AppError::validation(
                "Cette page est trop volumineuse pour être analysée.",
            ));
        }
        buf.extend_from_slice(&chunk);
    }
    Ok(buf)
}

// ------------------------------------------------------------------ balayage HTML
//
// Le HTML récupéré n'est jamais interprété ni réémis : on en extrait des valeurs, qui
// repartent ensuite par le même échappement que n'importe quel contenu utilisateur (§8).
// D'où ce balayeur de 60 lignes plutôt qu'un vrai parseur : on cherche des attributs dans
// une poignée de balises, pas un arbre DOM.

struct Page<'a> {
    raw: &'a str,
    /// Même disposition d'octets que `raw` (minuscule ASCII) : les index sont interchangeables.
    low: String,
}

impl<'a> Page<'a> {
    fn new(raw: &'a str) -> Self {
        Self {
            low: raw.to_ascii_lowercase(),
            raw,
        }
    }

    /// Zone d'attributs de chaque `<name ...>`, dans l'ordre du document.
    fn tags(&self, name: &str) -> Vec<&'a str> {
        let (raw, needle) = (self.raw, format!("<{name}"));
        let mut out = Vec::new();
        let mut i = 0;
        while let Some(p) = self.low[i..].find(&needle) {
            let s = i + p + needle.len();
            i = s;
            if !self.low[s..].starts_with(|c: char| c.is_ascii_whitespace() || c == '>' || c == '/')
            {
                continue;
            }
            let e = self.low[s..].find('>').map_or(self.low.len(), |k| s + k);
            out.push(&raw[s..e]);
            i = e;
        }
        out
    }

    fn meta(&self, key: &str) -> Option<String> {
        self.tags("meta")
            .into_iter()
            .find_map(|t| {
                let k = attr(t, "property")
                    .or_else(|| attr(t, "name"))
                    .or_else(|| attr(t, "itemprop"))?;
                k.trim()
                    .eq_ignore_ascii_case(key)
                    .then(|| attr(t, "content"))
                    .flatten()
            })
            .filter(|v| !v.trim().is_empty())
    }

    fn title(&self) -> Option<String> {
        let o = self.low.find("<title")?;
        let s = o + self.low[o..].find('>')? + 1;
        let e = s + self.low[s..].find("</title")?;
        Some(unescape(self.raw[s..e].trim())).filter(|t| !t.is_empty())
    }

    /// Contenu des `<script type="application/ld+json">` (schema.org).
    fn ld_json(&self) -> Vec<&'a str> {
        let raw = self.raw;
        let mut out = Vec::new();
        let mut i = 0;
        while let Some(p) = self.low[i..].find("<script") {
            let s = i + p;
            let Some(gt) = self.low[s..].find('>') else {
                break;
            };
            let open = s + gt + 1;
            let end = self.low[open..]
                .find("</script")
                .map_or(self.low.len(), |k| open + k);
            if attr(&raw[s..open], "type")
                .is_some_and(|t| t.to_ascii_lowercase().contains("ld+json"))
            {
                out.push(&raw[open..end]);
            }
            i = end;
        }
        out
    }
}

/// Valeur d'un attribut, guillemets simples, doubles ou aucun. Le nom doit être précédé d'un
/// blanc, sinon `data-href` répondrait pour `href`.
fn attr(tag: &str, name: &str) -> Option<String> {
    let low = tag.to_ascii_lowercase();
    let mut i = 0;
    while let Some(p) = low[i..].find(name) {
        let s = i + p;
        i = s + name.len();
        if s > 0 && !low.as_bytes()[s - 1].is_ascii_whitespace() {
            continue;
        }
        let rest = tag[i..].trim_start();
        let Some(v) = rest.strip_prefix('=').map(str::trim_start) else {
            continue;
        };
        let val = match v.as_bytes().first() {
            Some(b'"') => v[1..].split('"').next()?,
            Some(b'\'') => v[1..].split('\'').next()?,
            _ => v.split(|c: char| c.is_ascii_whitespace()).next()?,
        };
        return Some(unescape(val.trim()));
    }
    None
}

fn rel_has(tag: &str, pred: impl Fn(&str) -> bool) -> bool {
    attr(tag, "rel").is_some_and(|r| r.to_ascii_lowercase().split_whitespace().any(pred))
}

fn unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let head = match rest.char_indices().nth(12) {
            Some((k, _)) => &rest[..k],
            None => rest,
        };
        let ch = head.find(';').and_then(|semi| {
            let ent = &rest[1..semi];
            match ent {
                "amp" => Some(('&', semi)),
                "lt" => Some(('<', semi)),
                "gt" => Some(('>', semi)),
                "quot" => Some(('"', semi)),
                "apos" => Some(('\'', semi)),
                "nbsp" => Some((' ', semi)),
                _ => ent.strip_prefix('#').and_then(|n| {
                    match n.strip_prefix(['x', 'X']) {
                        Some(h) => u32::from_str_radix(h, 16).ok(),
                        None => n.parse().ok(),
                    }
                    .and_then(char::from_u32)
                    .map(|c| (c, semi))
                }),
            }
        });
        match ch {
            Some((c, semi)) => {
                out.push(c);
                rest = &rest[semi + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

// ------------------------------------------------------------------ nom, accroche, police

fn brand_name(page: &Page, base: &Url) -> String {
    let mut candidate = page
        .meta("og:site_name")
        .or_else(|| page.meta("application-name"))
        .or_else(|| page.meta("apple-mobile-web-app-title"))
        .or_else(|| schema_name(page))
        .or_else(|| page.title().map(|t| from_title(&t)))
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| from_host(base));

    if looks_like_domain_name(&candidate, base) {
        candidate = from_host(base);
    }

    candidate.trim().chars().take(60).collect()
}

fn looks_like_domain_name(name: &str, base: &Url) -> bool {
    let n = name
        .trim()
        .trim_end_matches('/')
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("www.")
        .to_ascii_lowercase();
    let host = base
        .host_str()
        .unwrap_or_default()
        .trim_start_matches("www.")
        .to_ascii_lowercase();
    let label = host.split('.').next().unwrap_or(&host);

    n == host
        || n == label
        || (n.contains('.')
            && n.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-')))
}

/// Premier `"name"` d'un bloc schema.org, à n'importe quelle profondeur (`@graph`, `publisher`…).
fn schema_name(page: &Page) -> Option<String> {
    fn find(v: &serde_json::Value) -> Option<String> {
        match v {
            serde_json::Value::Object(m) => m
                .get("name")
                .and_then(|n| n.as_str())
                .map(str::to_string)
                .or_else(|| m.values().find_map(find)),
            serde_json::Value::Array(a) => a.iter().find_map(find),
            _ => None,
        }
    }
    page.ld_json()
        .into_iter()
        .find_map(|b| {
            serde_json::from_str::<serde_json::Value>(b)
                .ok()
                .as_ref()
                .and_then(find)
        })
        .filter(|n| !n.trim().is_empty())
}

/// « Nos tarifs | Acme » → « Acme ». Le nom de marque est presque toujours le dernier segment.
fn from_title(title: &str) -> String {
    let normalized = title
        .replace(" - ", "|")
        .replace(" — ", "|")
        .replace(" :: ", "|");
    let parts: Vec<&str> = normalized
        .split(['|', '–', '—', '·', '•'])
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    match parts.as_slice() {
        [] => title.trim().to_string(),
        [only] => only.to_string(),
        // un dernier segment à rallonge est une accroche, pas un nom de marque
        parts => {
            let last = parts[parts.len() - 1];
            if last.chars().count() <= 40 {
                last
            } else {
                parts[0]
            }
            .to_string()
        }
    }
}

fn from_host(base: &Url) -> String {
    let host = base
        .host_str()
        .unwrap_or_default()
        .trim_start_matches("www.");
    let label = host.split('.').next().unwrap_or(host);
    let mut c = label.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => "Votre marque".to_string(),
    }
}

fn tagline(page: &Page) -> String {
    page.meta("og:description")
        .or_else(|| page.meta("description"))
        .or_else(|| page.meta("twitter:description"))
        .unwrap_or_default()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(240)
        .collect()
}

const GENERIC_FONTS: [&str; 8] = [
    "inherit",
    "initial",
    "sans-serif",
    "serif",
    "monospace",
    "cursive",
    "system-ui",
    "-apple-system",
];

/// Police déclarée : Google Fonts d'abord (c'est un choix explicite), sinon le premier
/// `font-family` non générique du CSS en ligne.
fn font(page: &Page) -> Option<String> {
    let google = page.tags("link").into_iter().find_map(|t| {
        let href = attr(t, "href")?;
        if !href.contains("fonts.googleapis.com") {
            return None;
        }
        let q = href.split("family=").nth(1)?;
        let fam = q.split(['&', ':', ';']).next()?.replace(['+', '%'], " ");
        Some(fam.trim().to_string()).filter(|f| !f.is_empty())
    });
    google.or_else(|| {
        let low = &page.low;
        let mut i = 0;
        while let Some(p) = low[i..].find("font-family") {
            let s = i + p + "font-family".len();
            i = s;
            let rest = low[s..].trim_start();
            let Some(list) = rest.strip_prefix(':') else {
                continue;
            };
            let first = list.split([',', ';', '}', '"', '\'']).find(|f| {
                let f = f.trim();
                !f.is_empty() && !GENERIC_FONTS.contains(&f)
            });
            if let Some(f) = first {
                return Some(title_case(f.trim()));
            }
        }
        None
    })
}

fn title_case(s: &str) -> String {
    s.split_whitespace()
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// ------------------------------------------------------------------ réseaux sociaux

const SOCIALS: [(&str, &str); 6] = [
    ("linkedin.com", "linkedin"),
    ("x.com", "x"),
    ("twitter.com", "x"),
    ("instagram.com", "instagram"),
    ("github.com", "github"),
    ("tiktok.com", "tiktok"),
];

fn socials(page: &Page, base: &Url) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for tag in page.tags("a") {
        let Some(href) = attr(tag, "href") else {
            continue;
        };
        let Ok(u) = base.join(href.trim()) else {
            continue;
        };
        add_social(&mut out, &u);
    }
    for raw in schema_same_as(page) {
        if let Ok(u) = base.join(raw.trim()) {
            add_social(&mut out, &u);
        }
    }
    out
}

fn add_social(out: &mut HashMap<String, String>, u: &Url) {
    if !matches!(u.scheme(), "http" | "https") {
        return;
    }
    // un bouton « partager » n'est pas le compte de la marque
    if u.path().contains("/share")
        || u.path().contains("/intent")
        || u.query().is_some_and(|q| q.contains("url="))
    {
        return;
    }
    if let Some(key) = social_key(u) {
        out.entry(key.to_string()).or_insert_with(|| u.to_string());
    }
}

fn social_key(u: &Url) -> Option<&'static str> {
    let host = u
        .host_str()
        .unwrap_or_default()
        .trim_start_matches("www.")
        .to_ascii_lowercase();
    SOCIALS
        .iter()
        .find(|(d, _)| host == *d || host.ends_with(&format!(".{d}")))
        .map(|(_, key)| *key)
}

fn schema_same_as(page: &Page) -> Vec<String> {
    fn strings(v: &serde_json::Value, out: &mut Vec<String>) {
        match v {
            serde_json::Value::String(s) => out.push(s.to_string()),
            serde_json::Value::Array(a) => a.iter().for_each(|v| strings(v, out)),
            _ => {}
        }
    }
    fn walk(v: &serde_json::Value, out: &mut Vec<String>) {
        match v {
            serde_json::Value::Object(m) => {
                for (k, v) in m {
                    if k.eq_ignore_ascii_case("sameAs") {
                        strings(v, out);
                    }
                    walk(v, out);
                }
            }
            serde_json::Value::Array(a) => a.iter().for_each(|v| walk(v, out)),
            _ => {}
        }
    }

    let mut out = Vec::new();
    for b in page.ld_json() {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(b) {
            walk(&v, &mut out);
        }
    }
    out
}

// ------------------------------------------------------------------ contacts publics

fn contacts(page: &Page, base: &Url) -> HashMap<String, String> {
    let mut out = HashMap::new();

    // Les liens explicites gagnent sur schema.org : ils sont généralement ceux que le visiteur
    // verrait dans le footer ou la page contact.
    for tag in page.tags("a") {
        let Some(href) = attr(tag, "href") else {
            continue;
        };
        insert_contact(&mut out, "email", email_href(&href));
        insert_contact(&mut out, "phone", phone_href(&href));
        insert_contact(&mut out, "whatsapp", whatsapp_href(&href, base));
    }

    for (key, value) in schema_contacts(page) {
        insert_contact(&mut out, &key, Some(value));
    }

    if !out.contains_key("email") {
        insert_contact(&mut out, "email", email_in_text(page.raw));
    }

    out
}

fn insert_contact(out: &mut HashMap<String, String>, key: &str, value: Option<String>) {
    let Some(value) = value.filter(|v| !v.trim().is_empty()) else {
        return;
    };
    out.entry(key.to_string()).or_insert(value);
}

fn email_href(href: &str) -> Option<String> {
    let raw = href.trim();
    let prefix = raw.get(..7)?;
    if !prefix.eq_ignore_ascii_case("mailto:") {
        return None;
    }
    clean_email(raw.get(7..)?)
}

fn phone_href(href: &str) -> Option<String> {
    let raw = href.trim();
    let prefix = raw.get(..4)?;
    if !prefix.eq_ignore_ascii_case("tel:") {
        return None;
    }
    clean_phone(raw.get(4..)?)
}

fn whatsapp_href(href: &str, base: &Url) -> Option<String> {
    let u = base.join(href.trim()).ok()?;
    let host = u
        .host_str()
        .unwrap_or_default()
        .trim_start_matches("www.")
        .to_ascii_lowercase();
    let whatsapp = host == "wa.me"
        || host.ends_with(".wa.me")
        || host == "whatsapp.com"
        || host.ends_with(".whatsapp.com");
    if !whatsapp {
        return None;
    }
    if host == "wa.me" || host.ends_with(".wa.me") {
        return clean_phone(u.path().trim_start_matches('/')).or_else(|| Some(u.to_string()));
    }
    u.query()
        .and_then(|q| {
            q.split('&').find_map(|pair| {
                let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
                k.eq_ignore_ascii_case("phone")
                    .then(|| clean_phone(v))
                    .flatten()
            })
        })
        .or_else(|| Some(u.to_string()))
}

fn schema_contacts(page: &Page) -> HashMap<String, String> {
    fn first(v: &serde_json::Value, parse: fn(&str) -> Option<String>) -> Option<String> {
        match v {
            serde_json::Value::String(s) => parse(s),
            serde_json::Value::Array(a) => a.iter().find_map(|v| first(v, parse)),
            serde_json::Value::Object(m) => m.values().find_map(|v| first(v, parse)),
            _ => None,
        }
    }
    fn walk(v: &serde_json::Value, out: &mut HashMap<String, String>) {
        match v {
            serde_json::Value::Object(m) => {
                for (k, v) in m {
                    if k.eq_ignore_ascii_case("email") {
                        insert_contact(out, "email", first(v, clean_email));
                    } else if k.eq_ignore_ascii_case("telephone") || k.eq_ignore_ascii_case("phone")
                    {
                        insert_contact(out, "phone", first(v, clean_phone));
                    }
                    walk(v, out);
                }
            }
            serde_json::Value::Array(a) => a.iter().for_each(|v| walk(v, out)),
            _ => {}
        }
    }

    let mut out = HashMap::new();
    for b in page.ld_json() {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(b) {
            walk(&v, &mut out);
        }
    }
    out
}

fn clean_email(raw: &str) -> Option<String> {
    let decoded = percent_decode_str(raw).decode_utf8_lossy();
    let email = decoded
        .split('?')
        .next()?
        .trim()
        .trim_matches(|c: char| "<>\"'(),;:[]{}".contains(c));
    if email.len() > 254
        || email.matches('@').count() != 1
        || email.chars().any(char::is_whitespace)
    {
        return None;
    }
    let (local, domain) = email.split_once('@')?;
    if local.is_empty()
        || domain.len() < 3
        || !domain.contains('.')
        || domain.starts_with('.')
        || domain.ends_with('.')
    {
        return None;
    }
    Some(email.to_ascii_lowercase())
}

fn clean_phone(raw: &str) -> Option<String> {
    let decoded = percent_decode_str(raw).decode_utf8_lossy();
    let value = decoded.split('?').next()?.trim();
    let digits: String = value.chars().filter(char::is_ascii_digit).collect();
    if !(6..=18).contains(&digits.len()) {
        return None;
    }
    Some(if value.starts_with('+') {
        format!("+{digits}")
    } else {
        digits
    })
}

fn email_in_text(raw: &str) -> Option<String> {
    raw.split(|c: char| {
        !(c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '%' | '+' | '-' | '@'))
    })
    .find_map(clean_email)
}

// ------------------------------------------------------------------ logo

/// Candidats par qualité décroissante, inspiré des règles `metascraper` : signaux explicites
/// de marque d'abord, icônes installables ensuite, images `logo` dans le HTML, chemins
/// standards par domaine, puis images sociales et favicons.
fn candidates(page: &Page, base: &Url, manifest: &[Url]) -> Vec<Url> {
    let mut apple: Vec<(u32, Url)> = page
        .tags("link")
        .into_iter()
        .filter(|t| rel_has(t, |r| r.starts_with("apple-touch-icon")))
        .filter_map(|t| {
            // une balise sans `sizes` est un 180×180 par convention Apple
            let size = attr(t, "sizes").map_or(180, |s| largest_size(&s));
            attr(t, "href")
                .and_then(|h| candidate_url(base, &h))
                .map(|u| (size, u))
        })
        .collect();
    apple.sort_by_key(|e| std::cmp::Reverse(e.0));

    let mut declared_icons: Vec<(u32, Url)> = page
        .tags("link")
        .into_iter()
        .filter(|t| rel_has(t, |r| matches!(r, "icon" | "shortcut" | "mask-icon")))
        .filter_map(|t| {
            let size = attr(t, "sizes").map_or(0, |s| largest_size(&s));
            attr(t, "href")
                .and_then(|h| candidate_url(base, &h))
                .map(|u| (size, u))
        })
        .collect();
    declared_icons.sort_by_key(|e| std::cmp::Reverse(e.0));

    let meta_images = meta_image_candidates(page, base);
    let html_logos = html_logo_candidates(page, base);
    let standard = standard_icon_paths(base);

    let mut favicons: Vec<Url> = page
        .tags("link")
        .into_iter()
        .filter(|t| rel_has(t, |r| r == "icon" || r == "shortcut"))
        .filter_map(|t| attr(t, "href").and_then(|h| candidate_url(base, &h)))
        .collect();
    favicons.extend(base.join("/favicon.ico").ok());

    let mut out: Vec<Url> = Vec::new();
    append_candidates(&mut out, schema_logos(page, base));
    append_candidates(&mut out, apple.into_iter().map(|(_, u)| u));
    append_candidates(&mut out, manifest.iter().cloned());
    append_candidates(&mut out, declared_icons.into_iter().map(|(_, u)| u));
    append_candidates(&mut out, html_logos);
    append_candidates(&mut out, standard);
    append_candidates(&mut out, meta_images);
    append_candidates(&mut out, favicons);
    out
}

fn append_candidates(out: &mut Vec<Url>, urls: impl IntoIterator<Item = Url>) {
    for u in urls {
        if out.len() >= MAX_CANDIDATES {
            break;
        }
        if !out.contains(&u) {
            out.push(u);
        }
    }
}

fn candidate_url(base: &Url, raw: &str) -> Option<Url> {
    let raw = raw.trim();
    if raw.is_empty()
        || raw.starts_with("data:")
        || raw.starts_with("javascript:")
        || raw.starts_with('#')
    {
        return None;
    }
    base.join(raw)
        .ok()
        .filter(|u| matches!(u.scheme(), "http" | "https"))
}

fn meta_image_candidates(page: &Page, base: &Url) -> Vec<Url> {
    [
        "og:logo",
        "og:image",
        "og:image:url",
        "og:image:secure_url",
        "twitter:image",
        "twitter:image:src",
        "image",
        "thumbnail",
        "thumbnailUrl",
        "msapplication-TileImage",
    ]
    .into_iter()
    .filter_map(|k| page.meta(k).and_then(|v| candidate_url(base, &v)))
    .fold(Vec::new(), |mut out, u| {
        if !out.contains(&u) {
            out.push(u);
        }
        out
    })
}

fn html_logo_candidates(page: &Page, base: &Url) -> Vec<Url> {
    let mut imgs = Vec::new();

    for tag in page.tags("img") {
        let marker = ["alt", "aria-label", "class", "id", "src", "data-src"]
            .into_iter()
            .filter_map(|name| attr(tag, name))
            .collect::<Vec<_>>()
            .join(" ");
        if !looks_like_logo_ref(&marker) {
            continue;
        }

        let declared_size = attr(tag, "width")
            .and_then(|v| parse_u32_attr(&v))
            .into_iter()
            .chain(attr(tag, "height").and_then(|v| parse_u32_attr(&v)))
            .max()
            .unwrap_or(0);

        if let Some((score, raw)) = attr(tag, "srcset").and_then(|v| best_srcset_url(&v)) {
            if let Some(u) = candidate_url(base, &raw) {
                imgs.push((score.max(declared_size), u));
            }
        }
        for name in [
            "src",
            "data-src",
            "data-lazy-src",
            "data-original",
            "data-actualsrc",
        ] {
            if let Some(raw) = attr(tag, name) {
                if let Some(u) = candidate_url(base, &raw) {
                    imgs.push((declared_size, u));
                }
            }
        }
    }

    imgs.sort_by_key(|e| std::cmp::Reverse(e.0));
    imgs.into_iter().map(|(_, u)| u).collect()
}

fn looks_like_logo_ref(s: &str) -> bool {
    let low = s.to_ascii_lowercase();
    ["logo", "logotype", "brand", "branding", "identity"]
        .into_iter()
        .any(|needle| low.contains(needle))
}

fn parse_u32_attr(raw: &str) -> Option<u32> {
    raw.trim().trim_end_matches("px").parse::<u32>().ok()
}

fn best_srcset_url(srcset: &str) -> Option<(u32, String)> {
    srcset
        .split(',')
        .filter_map(|item| {
            let mut parts = item.split_whitespace();
            let url = parts.next()?.to_string();
            let score = parts.filter_map(srcset_descriptor_score).max().unwrap_or(0);
            Some((score, url))
        })
        .max_by(|a, b| a.0.cmp(&b.0))
}

fn srcset_descriptor_score(desc: &str) -> Option<u32> {
    desc.strip_suffix('w')
        .and_then(|v| v.parse::<u32>().ok())
        .or_else(|| {
            desc.strip_suffix('x')
                .and_then(|v| v.parse::<f32>().ok())
                .map(|v| (v * 1000.0) as u32)
        })
}

fn standard_icon_paths(base: &Url) -> Vec<Url> {
    [
        "/apple-touch-icon.png",
        "/apple-touch-icon-precomposed.png",
        "/android-chrome-512x512.png",
        "/android-chrome-192x192.png",
        "/favicon-512x512.png",
        "/favicon-256x256.png",
        "/favicon-196x196.png",
        "/favicon-192x192.png",
        "/favicon-96x96.png",
        "/favicon-64x64.png",
        "/favicon-32x32.png",
        "/mstile-310x310.png",
        "/mstile-150x150.png",
        "/logo.png",
        "/logo.jpg",
        "/favicon.ico",
    ]
    .into_iter()
    .filter_map(|path| candidate_url(base, path))
    .collect()
}

fn schema_logos(page: &Page, base: &Url) -> Vec<Url> {
    fn collect(v: &serde_json::Value, out: &mut Vec<String>) {
        match v {
            serde_json::Value::String(s) => out.push(s.to_string()),
            serde_json::Value::Array(a) => a.iter().for_each(|v| collect(v, out)),
            serde_json::Value::Object(m) => {
                for key in ["url", "contentUrl"] {
                    if let Some(s) = m.get(key).and_then(|v| v.as_str()) {
                        out.push(s.to_string());
                    }
                }
            }
            _ => {}
        }
    }
    fn walk(v: &serde_json::Value, out: &mut Vec<String>) {
        match v {
            serde_json::Value::Object(m) => {
                for (k, v) in m {
                    if k.eq_ignore_ascii_case("logo") {
                        collect(v, out);
                    }
                    walk(v, out);
                }
            }
            serde_json::Value::Array(a) => a.iter().for_each(|v| walk(v, out)),
            _ => {}
        }
    }

    let mut raw = Vec::new();
    for b in page.ld_json() {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(b) {
            walk(&v, &mut raw);
        }
    }

    let mut out = Vec::new();
    for s in raw {
        let Ok(u) = base.join(s.trim()) else {
            continue;
        };
        if matches!(u.scheme(), "http" | "https") && !out.contains(&u) {
            out.push(u);
        }
    }
    out
}

fn manifest_urls(page: &Page, base: &Url) -> Vec<Url> {
    let mut out = Vec::new();
    for tag in page.tags("link") {
        if !rel_has(tag, |r| r == "manifest") {
            continue;
        }
        if let Some(u) = attr(tag, "href").and_then(|h| candidate_url(base, &h)) {
            append_candidates(&mut out, [u]);
        }
    }
    append_candidates(
        &mut out,
        [
            "/site.webmanifest",
            "/manifest.webmanifest",
            "/manifest.json",
        ]
        .into_iter()
        .filter_map(|path| candidate_url(base, path)),
    );
    out
}

/// Icônes du manifeste, la plus grande d'abord.
fn manifest_icons(json: &str, base: &Url) -> Vec<Url> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    let mut icons: Vec<(u32, Url)> = v["icons"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .filter_map(|i| {
            let src = i["src"].as_str()?;
            let size = i["sizes"].as_str().map_or(0, largest_size);
            base.join(src.trim()).ok().map(|u| (size, u))
        })
        .filter(|(_, u)| matches!(u.scheme(), "http" | "https"))
        .collect();
    icons.sort_by_key(|e| std::cmp::Reverse(e.0));
    icons.into_iter().map(|(_, u)| u).collect()
}

/// `"32x32 180x180 any"` → 180.
fn largest_size(sizes: &str) -> u32 {
    sizes
        .split_whitespace()
        .filter_map(|s| {
            s.split(['x', 'X'])
                .next()
                .and_then(|n| n.parse::<u32>().ok())
        })
        .max()
        .unwrap_or(0)
}

/// Un logo retenu, avec ses couleurs dominantes et sa surface en pixels.
/// Nommé plutôt qu'écrit en triplet : `Option<(Logo, Vec<(Color, f64)>, u64)>`
/// ne dit pas ce que chaque membre représente.
type LogoCandidate = (Logo, Vec<(Color, f64)>, u64);

async fn pick_logo(page: &Page<'_>, base: &Url) -> Option<(Logo, Vec<(Color, f64)>)> {
    let mut manifest = Vec::new();
    for u in manifest_urls(page, base) {
        if let Ok(f) = fetch(u.as_str(), MAX_ICON).await {
            append_candidates(
                &mut manifest,
                manifest_icons(&String::from_utf8_lossy(&f.bytes), &f.url),
            );
        }
    }

    let mut fallback: Option<LogoCandidate> = None;
    for url in candidates(page, base, &manifest) {
        let Ok(f) = fetch(url.as_str(), MAX_ICON).await else {
            continue;
        };
        let Some(content_type) = image_mime(&f.bytes).map(str::to_string) else {
            continue;
        };
        let source = f.url.to_string();
        let Some((bytes, width, height, colors)) = decode(f.bytes).await else {
            continue;
        };
        let logo = Logo {
            bytes,
            content_type,
            width,
            height,
            source,
        };
        if width >= PREFERRED_LOGO_PX && height >= PREFERRED_LOGO_PX {
            return Some((logo, colors));
        }
        if width >= USABLE_LOGO_PX && height >= USABLE_LOGO_PX {
            return Some((logo, colors));
        }

        let area = width as u64 * height as u64;
        if fallback
            .as_ref()
            .is_none_or(|(_, _, previous_area)| area > *previous_area)
        {
            fallback = Some((logo, colors, area));
        }
    }
    fallback.map(|(logo, colors, _)| (logo, colors))
}

/// Type déduit des octets, jamais du `Content-Type` du site (§8). Le SVG n'est pas reconnu
/// par `infer`, donc refusé de fait — et c'est très bien : c'est un vecteur de XSS.
fn image_mime(bytes: &[u8]) -> Option<&'static str> {
    let m = infer::get(bytes)?.mime_type();
    matches!(
        m,
        "image/png"
            | "image/jpeg"
            | "image/gif"
            | "image/webp"
            | "image/x-icon"
            | "image/vnd.microsoft.icon"
            | "image/bmp"
    )
    .then_some(m)
}

/// Décodage + quantification, hors de l'exécuteur : c'est du CPU, pas de l'attente.
async fn decode(bytes: Vec<u8>) -> Option<(Vec<u8>, u32, u32, Vec<(Color, f64)>)> {
    tokio::task::spawn_blocking(move || {
        let img = image::load_from_memory(&bytes).ok()?;
        let (w, h) = (img.width(), img.height());
        if w < MIN_LOGO_PX || h < MIN_LOGO_PX {
            return None;
        }
        Some((bytes, w, h, quantize(&img.to_rgba8())))
    })
    .await
    .ok()
    .flatten()
}

// ------------------------------------------------------------------ couleurs

/// Une couleur quasi blanche ou quasi noire n'est pas une couleur de marque : sans ce filtre,
/// toute marque ressort « blanche » — c'est la couleur du fond de son logo.
fn is_neutral_extreme(c: Color) -> bool {
    let (mx, mn) = (c.r.max(c.g).max(c.b), c.r.min(c.g).min(c.b));
    mn > 240 || mx < 24
}

/// Quantification 4 bits par canal (4096 casiers), représentant = moyenne réelle du casier.
/// Renvoie au plus 8 couleurs avec leur part de pixels retenus.
fn quantize(img: &image::RgbaImage) -> Vec<(Color, f64)> {
    // ponytail : échantillonnage à pas fixe plafonné à ~20 000 pixels. Un logo est plat, un
    // k-means n'y gagnerait rien de visible et coûterait vingt fois plus.
    let (w, h) = (img.width(), img.height());
    let step = (((w as f64 * h as f64) / 20_000.0).sqrt().ceil() as usize).max(1);

    #[derive(Default)]
    struct Sum {
        r: u64,
        g: u64,
        b: u64,
        n: u64,
    }

    let mut buckets: HashMap<(u8, u8, u8), Sum> = HashMap::new();
    let mut kept = 0u64;
    for y in (0..h as usize).step_by(step) {
        for x in (0..w as usize).step_by(step) {
            let p = img.get_pixel(x as u32, y as u32).0;
            if p[3] < 128 {
                continue; // pixel transparent
            }
            let c = Color::new(p[0], p[1], p[2]);
            if is_neutral_extreme(c) {
                continue;
            }
            let e = buckets.entry((c.r >> 4, c.g >> 4, c.b >> 4)).or_default();
            e.r += c.r as u64;
            e.g += c.g as u64;
            e.b += c.b as u64;
            e.n += 1;
            kept += 1;
        }
    }
    if kept == 0 {
        return Vec::new();
    }

    let mut out: Vec<(Color, f64)> = buckets
        .into_values()
        .map(|s| {
            (
                Color::new((s.r / s.n) as u8, (s.g / s.n) as u8, (s.b / s.n) as u8),
                s.n as f64 / kept as f64,
            )
        })
        .collect();
    out.sort_by(|a, b| b.1.total_cmp(&a.1));
    out.truncate(8);
    out
}

/// Palette de la marque, triée par saillance : `theme-color` (déclaration explicite), les
/// couleurs du logo, puis les hexadécimaux récurrents du CSS en ligne.
fn colors(page: &Page, logo_colors: Vec<(Color, f64)>) -> Vec<Color> {
    let mut cands: Vec<(Color, f64)> = Vec::new();
    if let Some(c) = page
        .meta("theme-color")
        .as_deref()
        .and_then(Color::parse_hex)
    {
        cands.push((c, 3.0));
    }
    cands.extend(logo_colors.into_iter().map(|(c, share)| (c, share * 2.0)));
    cands.extend(css_colors(page));

    // saillance : à fréquence égale, une couleur franche prime sur un gris.
    for (c, w) in cands.iter_mut() {
        *w *= 0.35 + 0.65 * c.saturation();
    }
    cands.sort_by(|a, b| b.1.total_cmp(&a.1));

    let mut out: Vec<Color> = Vec::new();
    for (c, _) in cands {
        if out.iter().all(|k| k.distance(c) > 48.0) {
            out.push(c);
        }
        if out.len() == MAX_COLORS {
            break;
        }
    }
    out
}

/// ponytail : on balaie tout le document plutôt que le seul CSS. Un `#rrggbb` hors couleur
/// est rare, la pondération par fréquence noie le bruit, et isoler le CSS demanderait de
/// suivre les `<style>`, les attributs `style` et les feuilles distantes (qu'on ne va pas
/// chercher). Plafond connu : les `#rgb` courts et `rgb()` ne sont pas comptés.
fn css_colors(page: &Page) -> Vec<(Color, f64)> {
    let raw = &page.low;
    let bytes = raw.as_bytes();
    let mut counts: HashMap<Color, u32> = HashMap::new();
    let mut total = 0u32;
    for (i, _) in raw.match_indices('#') {
        let Some(h) = raw.get(i + 1..i + 7) else {
            continue;
        };
        if !h.bytes().all(|b| b.is_ascii_hexdigit()) {
            continue;
        }
        // un 7e caractère alphanumérique : c'est un identifiant, pas une couleur
        if bytes
            .get(i + 7)
            .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'-' || *b == b'_')
        {
            continue;
        }
        let Some(c) = Color::parse_hex(h) else {
            continue;
        };
        if is_neutral_extreme(c) {
            continue;
        }
        *counts.entry(c).or_default() += 1;
        total += 1;
    }
    if total == 0 {
        return Vec::new();
    }
    let mut out: Vec<(Color, f64)> = counts
        .into_iter()
        .map(|(c, n)| (c, n as f64 / total as f64))
        .collect();
    out.sort_by(|a, b| b.1.total_cmp(&a.1));
    out.truncate(12);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = r##"
<!doctype html><html><head>
  <title>Nos tarifs — Acme Robotics</title>
  <meta property="og:site_name" content="Acme&nbsp;Robotics">
  <meta property="og:description" content="  Des bras   robotisés
       pour les PME.  ">
  <meta name="theme-color" content="#0a2540">
  <link rel="manifest" href="/site.webmanifest">
  <link rel="icon" href="/favicon.ico">
  <link rel="apple-touch-icon" sizes="120x120" href="/touch-120.png">
  <link rel="apple-touch-icon" sizes="180x180" href="/touch-180.png">
  <meta property="og:image" content="https://cdn.acme.test/og.png">
  <link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Space+Grotesk:wght@400;700">
  <style>.btn{background:#635bff;color:#ffffff}.hd{color:#635bff}.ft{color:#0a2540}</style>
  <script type="application/ld+json">{"@graph":[{"@type":"Organization","name":"Acme Robotics SAS","logo":"https://cdn.acme.test/logo.png","email":"hello@acme.test","telephone":"+33 9 87 65 43 21","sameAs":["https://www.instagram.com/acme","https://www.tiktok.com/@acme"]}]}</script>
</head><body>
  <a data-href="/leurre" href="https://twitter.com/intent/tweet?url=x">Partager</a>
  <a href="https://www.linkedin.com/company/acme">LinkedIn</a>
  <a href="//github.com/acme">GitHub</a>
  <a href="mailto:contact@acme.test?subject=Bonjour">Email</a>
  <a href="tel:+33 1 23 45 67 89">Téléphone</a>
  <a href="https://wa.me/33600000000">WhatsApp</a>
  <a href="/contact">Contact</a>
</body></html>"##;

    fn base() -> Url {
        Url::parse("https://acme.test/tarifs").unwrap()
    }

    // ---------------------------------------------------------------- SSRF

    #[test]
    fn refuse_les_adresses_internes_sans_toucher_au_reseau() {
        let http = reqwest::Client::new();
        for url in [
            "http://localhost/",
            "http://10.0.0.1/",
            "http://169.254.169.254/latest/meta-data/",
        ] {
            let err = tokio_test::block_on(extract(&http, url));
            assert!(err.is_err(), "{url} aurait dû être refusée");
        }
        // et ce qui n'est pas http(s) non plus
        assert!(tokio_test::block_on(extract(&http, "file:///etc/passwd")).is_err());
        assert!(tokio_test::block_on(extract(&http, "gopher://acme.test/")).is_err());
    }

    #[test]
    fn refuse_une_redirection_vers_une_ip_privee() {
        // hôte public littéral : aucun DNS, le test reste hors ligne
        let current = Url::parse("http://93.184.216.34/page").unwrap();

        for hop in [
            "http://169.254.169.254/latest/meta-data/iam/",
            "http://127.0.0.1:6379/",
            "http://10.0.0.1/",
            "http://[::1]/",
            "http://192.168.1.1/",
        ] {
            assert!(
                tokio_test::block_on(next_hop(&current, hop)).is_err(),
                "un 302 vers {hop} doit être refusé"
            );
        }

        // une redirection relative légitime passe, et reste sur l'hôte validé
        let ok = tokio_test::block_on(next_hop(&current, "/ailleurs")).unwrap();
        assert_eq!(ok.as_str(), "http://93.184.216.34/ailleurs");
    }

    // ---------------------------------------------------------------- logo

    #[test]
    fn choix_du_logo_par_ordre_de_priorite() {
        let page = Page::new(PAGE);
        let base = base();

        let manifest = manifest_icons(
            r#"{"icons":[{"src":"/icon-192.png","sizes":"192x192"},
                         {"src":"/icon-512.png","sizes":"512x512"},
                         {"src":"/icon-any.png"}]}"#,
            &base,
        );
        assert_eq!(
            manifest[0].as_str(),
            "https://acme.test/icon-512.png",
            "la plus grande d'abord"
        );
        assert_eq!(manifest[1].as_str(), "https://acme.test/icon-192.png");

        let got: Vec<String> = candidates(&page, &base, &manifest)
            .iter()
            .map(Url::to_string)
            .collect();
        assert_eq!(
            &got[..6],
            [
                "https://cdn.acme.test/logo.png",  // schema.org, le plus explicite
                "https://acme.test/touch-180.png", // apple-touch-icon, la plus grande
                "https://acme.test/touch-120.png",
                "https://acme.test/icon-512.png", // puis le manifeste
                "https://acme.test/icon-192.png",
                "https://acme.test/icon-any.png",
            ],
            "les signaux explicites doivent rester prioritaires"
        );
        assert!(
            got.contains(&"https://acme.test/apple-touch-icon.png".to_string()),
            "les chemins standards doivent servir de fallback"
        );
        assert!(
            got.iter().position(|u| u == "https://cdn.acme.test/og.png")
                > got.iter().position(|u| u == "https://acme.test/logo.jpg"),
            "og:image arrive après les icônes et logos probables"
        );

        // sans apple-touch-icon ni manifeste : og:image, puis le favicon déclaré, puis /favicon.ico
        let page = Page::new(
            r#"<link rel="icon" href="/f.png"><meta property="og:image" content="/og.jpg">"#,
        );
        let got: Vec<String> = candidates(&page, &base, &[])
            .iter()
            .map(Url::to_string)
            .collect();
        assert_eq!(got[0], "https://acme.test/f.png");
        assert!(
            got.iter().position(|u| u == "https://acme.test/og.jpg")
                > got.iter().position(|u| u == "https://acme.test/logo.jpg"),
            "une image sociale passe après les icônes/logos de domaine"
        );
    }

    #[test]
    fn le_type_du_logo_vient_des_octets() {
        assert_eq!(
            image_mime(b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR"),
            Some("image/png")
        );
        assert_eq!(
            image_mime(br#"<svg xmlns="http://www.w3.org/2000/svg"></svg>"#),
            None
        );
        assert_eq!(image_mime(b"<!doctype html>"), None);
    }

    #[test]
    fn page_type_orizn_donne_logo_contacts_et_reseaux() {
        let page = Page::new(
            r##"<meta name="theme-color" content="#0A0A0A">
                <meta property="og:site_name" content="Orizn">
                <meta property="og:image" content="https://orizn.app/landing/v2/orizn-landing-preview.png">
                <link rel="icon" href="/icon.png" type="image/png" sizes="512x512">
                <link rel="apple-touch-icon" href="/apple-icon.png" sizes="180x180" type="image/png">
                <script type="application/ld+json">{"@context":"https://schema.org","@type":["Organization","TravelAgency"],"name":"Orizn","url":"https://orizn.app","logo":"https://orizn.app/logo_orizn.png","sameAs":["https://x.com/orizn","https://instagram.com/orizn","https://linkedin.com/company/orizn","https://github.com/orizn","https://www.tiktok.com/@orizn"],"contactPoint":{"@type":"ContactPoint","contactType":"customer support","email":"contact@orizn.app"}}</script>"##,
        );
        let base = Url::parse("https://orizn.app/").unwrap();

        let got: Vec<String> = candidates(&page, &base, &[])
            .iter()
            .map(Url::to_string)
            .collect();
        assert_eq!(got[0], "https://orizn.app/logo_orizn.png");
        assert!(got.contains(&"https://orizn.app/apple-icon.png".to_string()));
        assert!(got.contains(&"https://orizn.app/icon.png".to_string()));

        let c = contacts(&page, &base);
        assert_eq!(
            c.get("email").map(String::as_str),
            Some("contact@orizn.app")
        );

        let s = socials(&page, &base);
        assert_eq!(
            s.get("linkedin").map(String::as_str),
            Some("https://linkedin.com/company/orizn")
        );
        assert_eq!(
            s.get("instagram").map(String::as_str),
            Some("https://instagram.com/orizn")
        );
        assert_eq!(
            s.get("tiktok").map(String::as_str),
            Some("https://www.tiktok.com/@orizn")
        );
    }

    // ---------------------------------------------------------------- balayage

    #[test]
    fn extrait_nom_accroche_police_et_reseaux() {
        let page = Page::new(PAGE);
        assert_eq!(brand_name(&page, &base()), "Acme Robotics");
        assert_eq!(tagline(&page), "Des bras robotisés pour les PME.");
        assert_eq!(font(&page).as_deref(), Some("Space Grotesk"));

        let s = socials(&page, &base());
        assert_eq!(
            s.get("linkedin").map(String::as_str),
            Some("https://www.linkedin.com/company/acme")
        );
        assert_eq!(
            s.get("github").map(String::as_str),
            Some("https://github.com/acme")
        );
        assert_eq!(
            s.get("instagram").map(String::as_str),
            Some("https://www.instagram.com/acme")
        );
        assert_eq!(
            s.get("tiktok").map(String::as_str),
            Some("https://www.tiktok.com/@acme")
        );
        assert!(
            !s.contains_key("x"),
            "un lien de partage n'est pas le compte de la marque"
        );

        // repli en cascade : titre, puis nom de domaine
        let t = Page::new("<title>Nos tarifs — Acme Robotics</title>");
        assert_eq!(brand_name(&t, &base()), "Acme Robotics");
        assert_eq!(brand_name(&Page::new(""), &base()), "Acme");
        let seloger_base = Url::parse("https://www.seloger.com/").unwrap();
        assert_eq!(
            brand_name(&Page::new("<title>seloger.com</title>"), &seloger_base),
            "Seloger",
            "un titre réduit à un domaine doit retomber sur le host analysé"
        );
        // schema.org quand og:site_name manque
        let ld = Page::new(
            r#"<script type="application/ld+json">{"@type":"Organization","name":"Zenith"}</script>"#,
        );
        assert_eq!(brand_name(&ld, &base()), "Zenith");
        // `data-href` ne doit pas répondre pour `href`
        assert_eq!(
            attr(r#" data-href="/a" href="/b""#, "href").as_deref(),
            Some("/b")
        );
    }

    #[test]
    fn extrait_le_logo_schema_org() {
        let page = Page::new(
            r#"<script type="application/ld+json">{"@type":"Organization","logo":{"@type":"ImageObject","url":"/logo.png"}}</script>"#,
        );
        let got: Vec<String> = schema_logos(&page, &base())
            .iter()
            .map(Url::to_string)
            .collect();
        assert_eq!(got, ["https://acme.test/logo.png"]);
    }

    #[test]
    fn extrait_un_logo_depuis_une_image_html() {
        let page = Page::new(
            r#"<img class="site-logo" src="/logo-small.png"
                    srcset="/logo-64.png 64w, /logo-256.png 256w" alt="Logo Acme">"#,
        );
        let got: Vec<String> = html_logo_candidates(&page, &base())
            .iter()
            .map(Url::to_string)
            .collect();

        assert_eq!(got[0], "https://acme.test/logo-256.png");
        assert!(got.contains(&"https://acme.test/logo-small.png".to_string()));
    }

    #[test]
    fn accepte_une_petite_icone_exploitable_en_fallback() {
        let mut bytes = Vec::new();
        let img = image::RgbaImage::from_pixel(57, 57, image::Rgba([0xf7, 0x4b, 0x32, 255]));
        image::DynamicImage::ImageRgba8(img)
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .unwrap();

        let got = tokio_test::block_on(decode(bytes)).expect("57×57 doit passer");
        assert_eq!((got.1, got.2), (57, 57));
    }

    #[test]
    fn extrait_contacts_publics() {
        let page = Page::new(PAGE);
        let c = contacts(&page, &base());
        assert_eq!(
            c.get("email").map(String::as_str),
            Some("contact@acme.test")
        );
        assert_eq!(c.get("phone").map(String::as_str), Some("+33123456789"));
        assert_eq!(c.get("whatsapp").map(String::as_str), Some("33600000000"));

        let schema = Page::new(
            r#"<script type="application/ld+json">{"@type":"Organization","email":"Sales@Example.COM","contactPoint":{"telephone":"+33 (0)6 11 22 33 44"}}</script>"#,
        );
        let c = contacts(&schema, &base());
        assert_eq!(
            c.get("email").map(String::as_str),
            Some("sales@example.com")
        );
        assert_eq!(c.get("phone").map(String::as_str), Some("+330611223344"));

        let visible = Page::new("<p>Écrivez à bonjour@acme.test pour parler à l'équipe.</p>");
        let c = contacts(&visible, &base());
        assert_eq!(
            c.get("email").map(String::as_str),
            Some("bonjour@acme.test")
        );
    }

    // ---------------------------------------------------------------- couleurs

    #[test]
    fn quantification_ignore_transparent_quasi_blanc_et_quasi_noir() {
        let mut img = image::RgbaImage::new(80, 80);
        for (x, y, p) in img.enumerate_pixels_mut() {
            *p = match (x < 40, y < 40) {
                (true, true) => image::Rgba([255, 255, 255, 255]), // quasi-blanc : ignoré
                (false, true) => image::Rgba([5, 5, 5, 255]),      // quasi-noir : ignoré
                (true, false) => image::Rgba([255, 0, 0, 0]),      // rouge transparent : ignoré
                (false, false) => image::Rgba([0x63, 0x5b, 0xff, 255]),
            };
        }
        let q = quantize(&img);
        assert_eq!(q.len(), 1, "seul le violet opaque doit rester : {q:?}");
        assert!(
            q[0].0.distance(Color::new(0x63, 0x5b, 0xff)) < 8.0,
            "{}",
            q[0].0
        );
        assert!((q[0].1 - 1.0).abs() < 1e-9);

        // un logo entièrement blanc ne donne aucune couleur, il ne « ressort » pas blanc
        let blanc = image::RgbaImage::from_pixel(64, 64, image::Rgba([255, 255, 255, 255]));
        assert!(quantize(&blanc).is_empty());
    }

    #[test]
    fn palette_triee_par_saillance() {
        let page = Page::new(PAGE);
        let cols = colors(&page, vec![(Color::new(0xff, 0x8a, 0x00), 0.5)]);
        // theme-color d'abord (déclaration explicite), puis les couleurs franches
        assert_eq!(cols[0], Color::new(0x0a, 0x25, 0x40));
        assert!(cols.contains(&Color::new(0xff, 0x8a, 0x00)), "{cols:?}");
        assert!(cols.contains(&Color::new(0x63, 0x5b, 0xff)), "{cols:?}");
        assert!(
            !cols.contains(&Color::WHITE),
            "le blanc n'est pas une couleur de marque"
        );
        assert!(cols.len() <= MAX_COLORS);
    }
}
