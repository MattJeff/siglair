//! Le document de signature — contrat §3. Stocké en `jsonb`, donc versionné dans le temps :
//! `#[serde(default)]` partout pour qu'un document écrit par une version antérieure se
//! désérialise toujours (un champ absent prend sa valeur par défaut, un champ inconnu est ignoré).

use std::collections::{BTreeMap, HashSet};
use std::net::{IpAddr, Ipv4Addr, ToSocketAddrs};

use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;

use crate::error::{AppError, Result};

/// Jetons `{{clé}}` : name, role, email, phone, website, linkedin, whatsapp, tagline, company.
pub type Profile = BTreeMap<String, String>;

pub const MAX_ELEMENTS: usize = 100;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Doc {
    pub v: u32,
    pub canvas: Canvas,
    /// L'ordre du tableau EST le z-index : index 0 = derrière.
    pub elements: Vec<Element>,
    pub timeline_duration: f64,
}

impl Default for Doc {
    fn default() -> Self {
        Self {
            v: 1,
            canvas: Canvas::default(),
            elements: Vec::new(),
            timeline_duration: 6.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Canvas {
    pub width: f64,
    pub height: f64,
    pub bg: String,
    /// URL http(s) ou "" — jamais de data: en base.
    pub bg_image: String,
    /// 0..1, voile noir au-dessus de `bg_image`.
    pub overlay: f64,
    pub radius: f64,
}

impl Default for Canvas {
    fn default() -> Self {
        Self {
            width: 620.0,
            height: 250.0,
            bg: "#07111f".into(),
            bg_image: String::new(),
            overlay: 0.8,
            radius: 18.0,
        }
    }
}

impl Canvas {
    /// Le viewport Chromium et les attributs `width`/`height` du `<img>` veulent des entiers.
    pub fn width_px(&self) -> u32 {
        self.width.round().clamp(1.0, 4000.0) as u32
    }

    pub fn height_px(&self) -> u32 {
        self.height.round().clamp(1.0, 4000.0) as u32
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Element {
    /// `[a-z0-9]{7}`, unique dans le document.
    pub id: String,
    #[serde(rename = "type")]
    pub kind: ElementType,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub rotation: f64,
    pub opacity: f64,
    /// Texte affiché ; supporte les jetons `{{...}}`.
    pub content: String,
    /// URL cible ; supporte les jetons ; "" = non cliquable.
    pub href: String,
    /// Média pour `image`/`video` (remplace l'ancien `src` en data-URL).
    pub asset_id: Option<Uuid>,
    pub font_size: f64,
    pub font_weight: String,
    pub color: String,
    pub background: String,
    pub radius: f64,
    pub align: String,
    pub locked: bool,
    pub hidden: bool,
    pub anim: Anim,
}

impl Default for Element {
    fn default() -> Self {
        Self {
            id: String::new(),
            kind: ElementType::Text,
            x: 0.0,
            y: 0.0,
            w: 160.0,
            h: 32.0,
            rotation: 0.0,
            opacity: 1.0,
            content: String::new(),
            href: String::new(),
            asset_id: None,
            font_size: 13.0,
            font_weight: "500".into(),
            color: "#ffffff".into(),
            background: "#2563eb".into(),
            radius: 0.0,
            align: "left".into(),
            locked: false,
            hidden: false,
            anim: Anim::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ElementType {
    #[default]
    Text,
    Button,
    Image,
    Video,
    Badge,
    Shape,
    Divider,
    Banner,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Anim {
    pub preset: AnimPreset,
    /// secondes, 0.1..20
    pub duration: f64,
    /// secondes, 0..20
    pub delay: f64,
    /// "infinite" | "1".."10"
    pub iterations: String,
    /// linear | ease | ease-in | ease-out | ease-in-out
    pub easing: String,
    /// 0.1..2 — multiplicateur d'amplitude
    pub intensity: f64,
    /// normal | reverse | alternate
    pub direction: String,
}

impl Default for Anim {
    fn default() -> Self {
        Self {
            preset: AnimPreset::None,
            duration: 2.4,
            delay: 0.0,
            iterations: "infinite".into(),
            easing: "ease-in-out".into(),
            intensity: 1.0,
            direction: "normal".into(),
        }
    }
}

/// Liste fermée — contrat §3.2. Le CSS de chaque preset vit dans `src/render/anim.css`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AnimPreset {
    #[default]
    None,
    Pulse,
    Glow,
    Float,
    Rotate,
    Bounce,
    Zoom,
    Fade,
    Reveal,
    Shimmer,
    Flicker,
    Swing,
    Slide,
    Draw,
}

impl AnimPreset {
    /// Suffixe de la classe CSS (`.anim-pulse`). `None` n'a pas de classe.
    pub fn css_name(self) -> Option<&'static str> {
        Some(match self {
            AnimPreset::None => return None,
            AnimPreset::Pulse => "pulse",
            AnimPreset::Glow => "glow",
            AnimPreset::Float => "float",
            AnimPreset::Rotate => "rotate",
            AnimPreset::Bounce => "bounce",
            AnimPreset::Zoom => "zoom",
            AnimPreset::Fade => "fade",
            AnimPreset::Reveal => "reveal",
            AnimPreset::Shimmer => "shimmer",
            AnimPreset::Flicker => "flicker",
            AnimPreset::Swing => "swing",
            AnimPreset::Slide => "slide",
            AnimPreset::Draw => "draw",
        })
    }
}

pub const EASINGS: [&str; 5] = ["linear", "ease", "ease-in", "ease-out", "ease-in-out"];
pub const DIRECTIONS: [&str; 3] = ["normal", "reverse", "alternate"];
pub const ALIGNS: [&str; 3] = ["left", "center", "right"];
pub const FONT_WEIGHTS: [&str; 11] = [
    "100", "200", "300", "400", "500", "600", "700", "800", "900", "normal", "bold",
];

// ---------------------------------------------------------------------- validation

impl Doc {
    /// Bornes du contrat §3. Appelé à chaque écriture d'un `doc` venu du client.
    pub fn validate(&self) -> Result<()> {
        range("canvas.width", self.canvas.width, 32.0, 2000.0)?;
        range("canvas.height", self.canvas.height, 32.0, 2000.0)?;
        range("canvas.overlay", self.canvas.overlay, 0.0, 1.0)?;
        range("canvas.radius", self.canvas.radius, 0.0, 200.0)?;
        range("timelineDuration", self.timeline_duration, 0.1, 60.0)?;
        paint("canvas.bg", &self.canvas.bg)?;
        if !self.canvas.bg_image.is_empty() {
            check_remote_url(&self.canvas.bg_image)?;
        }

        if self.elements.len() > MAX_ELEMENTS {
            return Err(AppError::validation(format!(
                "Une signature ne peut pas dépasser {MAX_ELEMENTS} éléments (elle en a {}).",
                self.elements.len()
            )));
        }

        let mut seen = HashSet::with_capacity(self.elements.len());
        for el in &self.elements {
            if el.id.len() != 7
                || !el
                    .id
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
            {
                return Err(AppError::validation(
                    "Identifiant d'élément invalide : 7 caractères, lettres minuscules et chiffres.",
                ));
            }
            if !seen.insert(el.id.as_str()) {
                return Err(AppError::validation(format!(
                    "Deux éléments portent le même identifiant ({}).",
                    el.id
                )));
            }
            el.validate()?;
        }
        Ok(())
    }
}

impl Element {
    fn validate(&self) -> Result<()> {
        range("x", self.x, -10_000.0, 10_000.0)?;
        range("y", self.y, -10_000.0, 10_000.0)?;
        range("w", self.w, 0.0, 10_000.0)?;
        range("h", self.h, 0.0, 10_000.0)?;
        range("rotation", self.rotation, -360.0, 360.0)?;
        range("opacity", self.opacity, 0.0, 1.0)?;
        range("fontSize", self.font_size, 1.0, 400.0)?;
        range("radius", self.radius, 0.0, 500.0)?;
        color("color", &self.color)?;
        paint("background", &self.background)?;
        one_of("align", &self.align, &ALIGNS)?;
        one_of("fontWeight", &self.font_weight, &FONT_WEIGHTS)?;

        if self.content.chars().count() > 2_000 {
            return Err(AppError::validation(
                "Le texte d'un élément est limité à 2 000 caractères.",
            ));
        }
        if self.href.len() > 2_000 {
            return Err(AppError::validation(
                "Le lien d'un élément est limité à 2 000 caractères.",
            ));
        }
        // Un href peut commencer par un jeton (`{{website}}`) : le schéma n'est connu qu'au
        // rendu, où `safe_href` re-vérifie la valeur résolue avant de produire le `<a>`.
        if !self.href.is_empty()
            && !self.href.trim_start().starts_with("{{")
            && safe_href(&self.href).is_none()
        {
            return Err(AppError::validation(format!(
                "Lien non autorisé sur « {} » : seuls http, https, mailto et tel sont acceptés.",
                if self.content.is_empty() {
                    self.id.as_str()
                } else {
                    self.content.as_str()
                }
            )));
        }

        range("anim.duration", self.anim.duration, 0.1, 20.0)?;
        range("anim.delay", self.anim.delay, 0.0, 20.0)?;
        range("anim.intensity", self.anim.intensity, 0.1, 2.0)?;
        one_of("anim.easing", &self.anim.easing, &EASINGS)?;
        one_of("anim.direction", &self.anim.direction, &DIRECTIONS)?;
        let it = &self.anim.iterations;
        if it != "infinite" && !matches!(it.parse::<u32>(), Ok(1..=10)) {
            return Err(AppError::validation(
                "Le nombre de répétitions doit être « infinite » ou un nombre entre 1 et 10.",
            ));
        }
        Ok(())
    }
}

fn range(field: &str, v: f64, min: f64, max: f64) -> Result<()> {
    if v.is_finite() && v >= min && v <= max {
        Ok(())
    } else {
        Err(AppError::validation(format!(
            "La valeur de « {field} » doit être comprise entre {min} et {max}."
        )))
    }
}

fn color(field: &str, v: &str) -> Result<()> {
    if is_hex_color(v) {
        Ok(())
    } else {
        Err(AppError::validation(format!(
            "La couleur « {field} » doit être au format #rrggbb."
        )))
    }
}

/// Remplissage fermé : couleur hex ou l'un des trois dégradés produits par l'éditeur.
/// Accepter du CSS arbitraire ici réintroduirait `url(...)` dans le HTML capturé par Chromium.
fn paint(field: &str, value: &str) -> Result<()> {
    if safe_paint(value).is_some() {
        Ok(())
    } else {
        Err(AppError::validation(format!(
            "Le remplissage « {field} » doit être une couleur #rrggbb ou un dégradé Siglair valide."
        )))
    }
}

pub fn safe_paint(value: &str) -> Option<&str> {
    let value = value.trim();
    if is_hex_color(value) {
        return Some(value);
    }

    if let Some(body) = value
        .strip_prefix("linear-gradient(")
        .and_then(|v| v.strip_suffix(')'))
    {
        let parts: Vec<_> = body.split(',').map(str::trim).collect();
        return (parts.len() == 3
            && angle(parts[0]).is_some()
            && color_stop(parts[1], "%", 0.0, 100.0)
            && color_stop(parts[2], "%", 0.0, 100.0))
        .then_some(value);
    }

    if let Some(body) = value
        .strip_prefix("radial-gradient(")
        .and_then(|v| v.strip_suffix(')'))
    {
        let parts: Vec<_> = body.split(',').map(str::trim).collect();
        return (parts.len() == 3
            && parts[0] == "circle at center"
            && color_stop(parts[1], "%", 0.0, 100.0)
            && color_stop(parts[2], "%", 0.0, 100.0))
        .then_some(value);
    }

    if let Some(body) = value
        .strip_prefix("conic-gradient(from ")
        .and_then(|v| v.strip_suffix(')'))
    {
        let parts: Vec<_> = body.split(',').map(str::trim).collect();
        let heading = parts.first()?.strip_suffix(" at center")?;
        return (parts.len() == 3
            && angle(heading).is_some()
            && color_stop(parts[1], "deg", 0.0, 360.0)
            && color_stop(parts[2], "deg", 0.0, 360.0))
        .then_some(value);
    }

    None
}

pub fn paint_fallback_color(value: &str) -> Option<&str> {
    let bytes = value.as_bytes();
    for index in 0..bytes.len().saturating_sub(6) {
        let Some(candidate) = value.get(index..index + 7) else {
            continue;
        };
        if is_hex_color(candidate) {
            return Some(candidate);
        }
    }
    None
}

fn is_hex_color(value: &str) -> bool {
    value.len() == 7 && value.starts_with('#') && value[1..].bytes().all(|b| b.is_ascii_hexdigit())
}

fn angle(value: &str) -> Option<f64> {
    let value = value.strip_suffix("deg")?.parse::<f64>().ok()?;
    (value.is_finite() && (0.0..=360.0).contains(&value)).then_some(value)
}

fn color_stop(value: &str, unit: &str, min: f64, max: f64) -> bool {
    let mut parts = value.split_whitespace();
    let Some(color) = parts.next() else {
        return false;
    };
    let Some(position) = parts.next() else {
        return false;
    };
    if parts.next().is_some() || !is_hex_color(color) {
        return false;
    }
    let Some(number) = position.strip_suffix(unit) else {
        return false;
    };
    number
        .parse::<f64>()
        .is_ok_and(|n| n.is_finite() && (min..=max).contains(&n))
}

fn one_of(field: &str, v: &str, allowed: &[&str]) -> Result<()> {
    if allowed.contains(&v) {
        Ok(())
    } else {
        Err(AppError::validation(format!(
            "Valeur invalide pour « {field} » : attendu {}.",
            allowed.join(", ")
        )))
    }
}

// ---------------------------------------------------------------------- jetons

/// Remplace `{{clé}}` par la valeur du profil ; un jeton inconnu devient "".
/// Garantie : la sortie ne contient jamais `{{` — un `{{website}}` visible dans un e-mail
/// envoyé à un client est pire qu'une valeur vide.
pub fn resolve_tokens(s: &str, profile: &Profile) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find("{{") {
        out.push_str(&rest[..i]);
        let after = &rest[i + 2..];
        match after.find("}}") {
            Some(j) => {
                if let Some(v) = profile.get(after[..j].trim()) {
                    out.push_str(&v.replace("{{", ""));
                }
                rest = &after[j + 2..];
            }
            // jeton non fermé : on jette la fin plutôt que de laisser fuir des accolades
            None => rest = "",
        }
    }
    out.push_str(rest);
    out
}

/// Schémas autorisés dans un `href` (contrat §8). `javascript:` dans une signature
/// d'entreprise, c'est du XSS stocké distribué par e-mail.
pub fn safe_href(s: &str) -> Option<String> {
    let t = s.trim();
    let low = t.to_ascii_lowercase();
    ["http://", "https://", "mailto:", "tel:"]
        .iter()
        .any(|p| low.starts_with(p))
        .then(|| t.to_string())
}

// ---------------------------------------------------------------------- garde SSRF

/// Contrat §8. Ces URL sont chargées par Chromium *depuis l'intérieur de notre réseau* :
/// tout ce qui n'est pas un hôte public est refusé. À rappeler au rendu, pas seulement à
/// l'enregistrement — un DNS peut changer de réponse entre les deux.
///
/// ponytail : la résolution DNS est bloquante (une écriture de doc, pas un chemin chaud) ;
/// passer par `spawn_blocking` si un jour ça se voit dans les latences.
pub fn check_remote_url(raw: &str) -> Result<Url> {
    let refuse = |why: &str| AppError::validation(format!("Image distante refusée : {why}."));

    let url = Url::parse(raw.trim()).map_err(|_| refuse("URL invalide"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(refuse("seules les URL http et https sont acceptées"));
    }
    let host = url.host_str().ok_or_else(|| refuse("hôte manquant"))?;
    let host_l = host.trim_end_matches('.').to_ascii_lowercase();
    if host_l == "localhost" || host_l.ends_with(".localhost") || host_l.ends_with(".internal") {
        return Err(refuse("cet hôte est interne"));
    }

    // Littéral IP : pas de DNS à faire, on tranche tout de suite.
    if let Ok(ip) = host_l
        .trim_matches(|c| c == '[' || c == ']')
        .parse::<IpAddr>()
    {
        return if is_blocked(ip) {
            Err(refuse("cette adresse est interne"))
        } else {
            Ok(url)
        };
    }

    let port = url.port_or_known_default().unwrap_or(80);
    let addrs = (host_l.as_str(), port)
        .to_socket_addrs()
        .map_err(|_| refuse("hôte introuvable"))?;
    let mut any = false;
    for a in addrs {
        any = true;
        if is_blocked(a.ip()) {
            return Err(refuse("cet hôte pointe vers une adresse interne"));
        }
    }
    if !any {
        return Err(refuse("hôte introuvable"));
    }
    Ok(url)
}

fn is_blocked(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_blocked_v4(v4),
        IpAddr::V6(v6) => {
            // ::ffff:127.0.0.1 est un contournement classique de ce genre de filtre.
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_blocked_v4(v4);
            }
            let s = v6.segments()[0];
            v6.is_loopback()
                || v6.is_unspecified()
                || (s & 0xfe00) == 0xfc00 // fc00::/7 — unique local
                || (s & 0xffc0) == 0xfe80 // fe80::/10 — link-local
        }
    }
}

fn is_blocked_v4(ip: Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();
    ip.is_loopback()          // 127/8
        || ip.is_private()    // 10/8, 172.16/12, 192.168/16
        || ip.is_link_local() // 169.254/16, dont 169.254.169.254 (metadata cloud)
        || ip.is_unspecified()
        || ip.is_broadcast()
        || ip.is_multicast()
        || a == 0
        || (a == 100 && (64..128).contains(&b)) // 100.64/10 — CGNAT
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> Profile {
        Profile::from([
            ("name".into(), "Ada".into()),
            ("website".into(), "https://ada.dev".into()),
        ])
    }

    #[test]
    fn tokens_resolve_and_never_leak_braces() {
        let p = profile();
        assert_eq!(resolve_tokens("Bonjour {{name}} !", &p), "Bonjour Ada !");
        assert_eq!(resolve_tokens("{{ name }}", &p), "Ada");
        assert_eq!(resolve_tokens("[{{unknown}}]", &p), "[]");
        assert_eq!(resolve_tokens("a {{oups", &p), "a ");
        for s in ["{{name}} {{x}}", "{{oups", "{{{{name}}}}"] {
            assert!(!resolve_tokens(s, &p).contains("{{"), "{s}");
        }
    }

    #[test]
    fn href_schemes() {
        assert!(safe_href("https://ok.dev").is_some());
        assert!(safe_href("mailto:a@b.c").is_some());
        assert!(safe_href("tel:+33600000000").is_some());
        for bad in [
            "javascript:alert(1)",
            "JavaScript:alert(1)",
            "data:text/html,x",
            "file:///etc",
            "",
            "/relatif",
        ] {
            assert!(safe_href(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn ssrf_guard() {
        for bad in [
            "http://localhost/x",
            "http://127.0.0.1/x",
            "http://127.1.2.3/x",
            "http://10.0.0.5/x",
            "http://172.16.4.4/x",
            "http://192.168.1.1/x",
            "http://169.254.169.254/latest/meta-data/",
            "http://[::1]/x",
            "http://[fc00::1]/x",
            "http://[::ffff:127.0.0.1]/x",
            "http://0.0.0.0/x",
            "http://100.64.1.1/x",
            "file:///etc/passwd",
            "javascript:alert(1)",
            "not a url",
        ] {
            assert!(check_remote_url(bad).is_err(), "aurait dû refuser {bad}");
        }
        assert!(check_remote_url("https://1.1.1.1/logo.png").is_ok());
    }

    #[test]
    fn validation_bounds() {
        let mut d = Doc::default();
        d.elements.push(Element {
            id: "abc1234".into(),
            ..Default::default()
        });
        assert!(d.validate().is_ok());

        let mut dup = d.clone();
        dup.elements.push(Element {
            id: "abc1234".into(),
            ..Default::default()
        });
        assert!(dup.validate().is_err());

        let mut bad_id = d.clone();
        bad_id.elements[0].id = "ABC".into();
        assert!(bad_id.validate().is_err());

        let mut bad_color = d.clone();
        bad_color.elements[0].color = "red".into();
        assert!(bad_color.validate().is_err());

        let mut bad_href = d.clone();
        bad_href.elements[0].href = "javascript:alert(1)".into();
        assert!(bad_href.validate().is_err());

        let mut token_href = d.clone();
        token_href.elements[0].href = "{{website}}".into();
        assert!(token_href.validate().is_ok());

        let mut bad_anim = d.clone();
        bad_anim.elements[0].anim.easing = "url(evil)".into();
        assert!(bad_anim.validate().is_err());

        let too_many = Doc {
            elements: (0..101)
                .map(|_| Element {
                    id: "abc1234".into(),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        };
        assert!(too_many.validate().is_err());
    }

    #[test]
    fn validates_editor_gradients_without_accepting_arbitrary_css() {
        for valid in [
            "#112233",
            "linear-gradient(135deg, #112233 0%, #445566 100%)",
            "radial-gradient(circle at center, #112233 0%, #445566 100%)",
            "conic-gradient(from 45deg at center, #112233 0deg, #445566 360deg)",
        ] {
            assert!(safe_paint(valid).is_some(), "aurait dû accepter {valid}");
            assert_eq!(paint_fallback_color(valid), Some("#112233"));
        }

        for invalid in [
            "red",
            "linear-gradient(90deg, #112233, url(https://evil.test/x))",
            "linear-gradient(999deg, #112233 0%, #445566 100%)",
            "radial-gradient(circle at top, #112233 0%, #445566 100%)",
            "conic-gradient(from 45deg at center, #112233 0%, var(--evil) 360deg)",
            "é linear-gradient(90deg, #112233 0%, #445566 100%)",
        ] {
            assert!(safe_paint(invalid).is_none(), "aurait dû refuser {invalid}");
        }
    }

    #[test]
    fn old_document_deserializes() {
        // document minimal, champs manquants + ancien champ `src` en trop
        let d: Doc = serde_json::from_str(
            r#"{"canvas":{"width":400},"elements":[{"id":"aaa1111","type":"image","src":"data:image/png;base64,xx"}]}"#,
        )
        .unwrap();
        assert_eq!(d.canvas.width, 400.0);
        assert_eq!(d.canvas.height, 250.0);
        assert_eq!(d.timeline_duration, 6.0);
        assert_eq!(d.elements[0].kind, ElementType::Image);
        assert_eq!(d.elements[0].anim.preset, AnimPreset::None);
    }
}
