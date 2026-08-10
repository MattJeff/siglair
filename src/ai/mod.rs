//! Onboarding « colle ton site, récupère ta signature » — contrat §6bis.
//!
//! Trois étages, volontairement séparés :
//!   - [`brand`]    : extraction déterministe depuis une URL. Aucun appel de modèle (§6bis.2).
//!   - [`generate`] : le seul appel réseau vers le fournisseur d'IA (§6bis.3).
//!   - [`compose`]  : recette → [`crate::doc::Doc`], du Rust ordinaire (§6bis.1).
//!
//! La règle qui rend la fonctionnalité sûre est ici, dans les types : le modèle ne renvoie
//! jamais de géométrie, il renvoie un [`VariantSpec`] — des enums fermés, une palette et des
//! libellés. Un modèle ne peut donc pas produire du HTML cassé dans l'Outlook d'un client.

pub mod brand;
pub mod compose;
pub mod generate;
pub mod handoff;

use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::doc::AnimPreset;

// ---------------------------------------------------------------------- couleur

/// Une couleur de marque. Sérialisée en `"#rrggbb"` : elle se pose telle quelle dans
/// `Element.color` / `Canvas.bg`, qui sont des `String` validées au format `#rrggbb` (§3).
/// C'est le seul type de couleur du projet — n'en ajouter aucun autre à côté.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// Seuil WCAG AA pour du texte normal. Une signature illisible n'est pas une signature.
pub const WCAG_AA: f64 = 4.5;

impl Color {
    pub const WHITE: Color = Color::new(255, 255, 255);
    pub const BLACK: Color = Color::new(0, 0, 0);

    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// `#abc`, `#aabbcc`, `#aabbccdd` (l'alpha est ignoré). Rien d'autre : les couleurs
    /// nommées et `rgb()` n'apparaissent quasiment jamais dans le CSS d'une charte.
    pub fn parse_hex(s: &str) -> Option<Self> {
        let h = s.trim().trim_start_matches('#');
        let d = |i: usize| u8::from_str_radix(&h[i..i + 1], 16).ok();
        let p = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
        if !h.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        match h.len() {
            3 => Some(Self::new(d(0)? * 17, d(1)? * 17, d(2)? * 17)),
            6 | 8 => Some(Self::new(p(0)?, p(2)?, p(4)?)),
            _ => None,
        }
    }

    /// Luminance relative WCAG.
    pub fn luminance(self) -> f64 {
        fn chan(c: u8) -> f64 {
            let s = c as f64 / 255.0;
            if s <= 0.039_28 {
                s / 12.92
            } else {
                ((s + 0.055) / 1.055).powf(2.4)
            }
        }
        0.2126 * chan(self.r) + 0.7152 * chan(self.g) + 0.0722 * chan(self.b)
    }

    /// Ratio de contraste WCAG, entre 1.0 et 21.0.
    pub fn contrast(self, other: Self) -> f64 {
        let (a, b) = (self.luminance(), other.luminance());
        let (hi, lo) = if a > b { (a, b) } else { (b, a) };
        (hi + 0.05) / (lo + 0.05)
    }

    /// Saturation HSV, 0.0 (gris) à 1.0 (couleur pure) — sert à trier par saillance.
    pub fn saturation(self) -> f64 {
        let (mx, mn) = self.extremes();
        if mx == 0 {
            0.0
        } else {
            (mx - mn) as f64 / mx as f64
        }
    }

    /// Mélange linéaire vers `other`, `t` dans 0..1.
    pub fn mix(self, other: Self, t: f64) -> Self {
        let t = t.clamp(0.0, 1.0);
        let m = |a: u8, b: u8| (a as f64 + (b as f64 - a as f64) * t).round() as u8;
        Self::new(m(self.r, other.r), m(self.g, other.g), m(self.b, other.b))
    }

    /// Distance euclidienne en RGB — suffisant pour dédoublonner deux teintes voisines.
    pub fn distance(self, other: Self) -> f64 {
        let d = |a: u8, b: u8| (a as f64 - b as f64).powi(2);
        (d(self.r, other.r) + d(self.g, other.g) + d(self.b, other.b)).sqrt()
    }

    fn extremes(self) -> (u8, u8) {
        (
            self.r.max(self.g).max(self.b),
            self.r.min(self.g).min(self.b),
        )
    }
}

/// Corrige `fg` jusqu'à atteindre 4.5:1 sur `bg`, en conservant la teinte (mélange vers le
/// blanc ou le noir, selon celui des deux qui contraste le mieux avec le fond).
///
/// Une palette extraite d'un logo n'a aucune raison d'être lisible : du orange de marque sur
/// du blanc de marque donne 2:1. On corrige le texte, jamais le fond — le fond, c'est la marque.
pub fn ensure_contrast(fg: Color, bg: Color) -> Color {
    if fg.contrast(bg) >= WCAG_AA {
        return fg;
    }
    let target = if Color::WHITE.contrast(bg) >= Color::BLACK.contrast(bg) {
        Color::WHITE
    } else {
        Color::BLACK
    };
    for i in 1..=20 {
        let c = fg.mix(target, i as f64 / 20.0);
        if c.contrast(bg) >= WCAG_AA {
            return c;
        }
    }
    // Inatteignable : à la 20e étape le mélange *est* `target`, et pour tout fond le blanc
    // ou le noir dépasse 4.5:1 (les deux ne peuvent pas échouer ensemble).
    target
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

impl FromStr for Color {
    type Err = &'static str;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Self::parse_hex(s).ok_or("couleur attendue au format #rrggbb")
    }
}

impl From<Color> for String {
    fn from(c: Color) -> String {
        c.to_string()
    }
}

impl TryFrom<String> for Color {
    type Error = &'static str;
    fn try_from(s: String) -> std::result::Result<Self, Self::Error> {
        s.parse()
    }
}

// ---------------------------------------------------------------------- marque extraite

/// Ce que [`brand::extract`] tire d'un site public — contrat §6bis.2.
///
/// **Aucune donnée saisie par l'utilisateur ici, par construction** (§6bis.3) : uniquement
/// des informations publiques tirées du site analysé. Le profil de l'utilisateur entre dans
/// `compose.rs` localement, après la réponse du modèle.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Brand {
    /// URL réellement analysée (après redirections revalidées).
    pub site: String,
    pub name: String,
    /// Accroche publique (`og:description`).
    pub tagline: String,
    pub logo: Option<Logo>,
    /// Rempli **après** l'extraction, par la route qui enregistre `logo` dans `assets` :
    /// un `Doc` ne porte jamais d'octets ni d'URL distante, seulement un `assetId` (§3).
    /// `brand.rs` n'a ni base ni stockage, il ne peut donc pas le remplir lui-même.
    pub logo_asset_id: Option<Uuid>,
    /// Triées par saillance décroissante. Peut être vide.
    pub colors: Vec<Color>,
    /// `linkedin` | `x` | `instagram` | `github` → URL absolue.
    pub socials: HashMap<String, String>,
    /// `email` | `phone` | `whatsapp` → coordonnée publique détectée sur le site.
    pub contacts: HashMap<String, String>,
    pub font: Option<String>,
}

/// Le logo récupéré, en octets. Sérialisé en base64 : la vitrine l'affiche en `data:` avant
/// même l'inscription, et la route d'onboarding l'enregistre ensuite comme `asset`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Logo {
    #[serde(with = "b64")]
    pub bytes: Vec<u8>,
    /// Déduit des octets (`infer`), jamais du `Content-Type` du site (§8).
    pub content_type: String,
    pub width: u32,
    pub height: u32,
    /// URL d'origine — utile en journal quand un site sert un logo inattendu.
    pub source: String,
}

mod b64 {
    use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &[u8], s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&B64.encode(v))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> std::result::Result<Vec<u8>, D::Error> {
        let s = String::deserialize(d)?;
        B64.decode(s.as_bytes()).map_err(serde::de::Error::custom)
    }
}

// ---------------------------------------------------------------------- la recette

/// Enum fermé, désérialisation tolérante : une valeur inconnue retombe sur le défaut plutôt
/// que de faire échouer toute la réponse du modèle (§6bis.3 — c'est `compose.rs` qui ramène
/// en borne, jamais le validateur qui rejette).
macro_rules! closed_enum {
    ($(#[$doc:meta])* $name:ident, $def:ident, { $($v:ident => $s:literal),+ $(,)? }) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name { $($v),+ }

        impl Default for $name {
            fn default() -> Self { Self::$def }
        }

        impl $name {
            pub fn as_str(self) -> &'static str {
                match self { $(Self::$v => $s),+ }
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
                s.serialize_str(self.as_str())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
                let raw = serde_json::Value::deserialize(d)?;
                let s = raw.as_str().unwrap_or_default().trim().to_ascii_lowercase().replace('_', "-");
                Ok(match s.as_str() { $($s => Self::$v,)+ _ => Self::$def })
            }
        }
    };
}

closed_enum!(
    /// Nos 4 modèles. Les identifiants sont ceux de `web/src/editor/templates.ts`.
    Template, FounderMotion, {
        FounderMotion => "founder-motion",
        NeonFounder   => "neon-founder",
        Sales         => "sales",
        Minimal       => "minimal",
    }
);

closed_enum!(LogoPlacement, Left, { Left => "left", Top => "top" });

closed_enum!(LogoShape, Circle, {
    Circle  => "circle",
    Rounded => "rounded",
    Square  => "square",
});

closed_enum!(CtaTarget, Website, {
    Website  => "website",
    Linkedin => "linkedin",
    Whatsapp => "whatsapp",
    Email    => "email",
    Calendar => "calendar",
});

closed_enum!(CtaStyle, Solid, {
    Solid   => "solid",
    Outline => "outline",
    Ghost   => "ghost",
});

closed_enum!(
    /// D'où viennent les propositions. Va dans les journaux, pas dans l'interface :
    /// l'utilisateur n'a pas à savoir que le fournisseur était en panne.
    VariantSource, Fallback, {
        Model    => "model",
        Fallback => "fallback",
    }
);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", from = "RawPalette")]
pub struct Palette {
    pub bg: Color,
    pub surface: Color,
    pub text: Color,
    pub muted: Color,
    pub accent: Color,
    pub accent2: Color,
}

/// Étage de tolérance (§6bis.3) : une couleur absente ou illisible (`"bleu"`, `"rgb(...)"`)
/// retombe sur celle du défaut, au lieu de faire échouer toute la réponse du modèle.
#[derive(Default, Deserialize)]
#[serde(default)]
struct RawPalette {
    bg: MaybeColor,
    surface: MaybeColor,
    text: MaybeColor,
    muted: MaybeColor,
    accent: MaybeColor,
    accent2: MaybeColor,
}

#[derive(Default)]
struct MaybeColor(Option<Color>);

impl<'de> Deserialize<'de> for MaybeColor {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let raw = serde_json::Value::deserialize(d)?;
        Ok(Self(raw.as_str().and_then(Color::parse_hex)))
    }
}

impl From<RawPalette> for Palette {
    fn from(r: RawPalette) -> Self {
        let d = Palette::default();
        Self {
            bg: r.bg.0.unwrap_or(d.bg),
            surface: r.surface.0.unwrap_or(d.surface),
            text: r.text.0.unwrap_or(d.text),
            muted: r.muted.0.unwrap_or(d.muted),
            accent: r.accent.0.unwrap_or(d.accent),
            accent2: r.accent2.0.unwrap_or(d.accent2),
        }
    }
}

impl Default for Palette {
    fn default() -> Self {
        Self {
            bg: Color::new(0x07, 0x11, 0x1f),
            surface: Color::new(0x0f, 0x1d, 0x33),
            text: Color::WHITE,
            muted: Color::new(0x94, 0xa3, 0xb8),
            accent: Color::new(0x25, 0x63, 0xeb),
            accent2: Color::new(0x38, 0xbd, 0xf8),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct LogoSpec {
    pub placement: LogoPlacement,
    pub shape: LogoShape,
    #[serde(deserialize_with = "anim_lenient")]
    pub anim: AnimPreset,
    pub duration: f64,
}

impl Default for LogoSpec {
    fn default() -> Self {
        Self {
            placement: LogoPlacement::Left,
            shape: LogoShape::Circle,
            anim: AnimPreset::None,
            duration: 2.4,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct Cta {
    pub label: String,
    pub target: CtaTarget,
    pub style: CtaStyle,
}

/// Une proposition — contrat §6bis.4. Aucune géométrie, aucun HTML, aucune coordonnée :
/// c'est `compose.rs` qui en fait un `Doc`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct VariantSpec {
    pub name: String,
    pub template: Template,
    pub palette: Palette,
    pub logo: LogoSpec,
    #[serde(deserialize_with = "anim_lenient")]
    pub accent_anim: AnimPreset,
    pub ctas: Vec<Cta>,
    /// Affiché sous la proposition : c'est ce qui montre que le système a lu la marque.
    pub rationale: String,
}

/// Les 3 propositions, plus leur provenance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateOutcome {
    /// Exactement 3 entrées — `compose::fallback_variants` complète si le modèle en rend moins.
    pub variants: Vec<VariantSpec>,
    pub source: VariantSource,
}

/// `AnimPreset` vient de `doc.rs` et refuse une valeur inconnue ; ici on la tolère.
fn anim_lenient<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<AnimPreset, D::Error> {
    let raw = serde_json::Value::deserialize(d)?;
    Ok(serde_json::from_value(raw).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_aller_retour() {
        assert_eq!(
            Color::parse_hex("#2563EB"),
            Some(Color::new(0x25, 0x63, 0xeb))
        );
        assert_eq!(Color::parse_hex("#abc"), Some(Color::new(0xaa, 0xbb, 0xcc)));
        assert_eq!(
            Color::parse_hex("#2563ebff"),
            Some(Color::new(0x25, 0x63, 0xeb))
        );
        assert_eq!(Color::parse_hex("bleu"), None);
        assert_eq!(Color::parse_hex("#12345"), None);
        assert_eq!(Color::new(0x07, 0x11, 0x1f).to_string(), "#07111f");
        // sérialisé comme une chaîne : se pose tel quel dans `Element.color` (§3)
        assert_eq!(serde_json::to_string(&Color::WHITE).unwrap(), "\"#ffffff\"");
        assert_eq!(
            serde_json::from_str::<Color>("\"#000000\"").unwrap(),
            Color::BLACK
        );
    }

    #[test]
    fn contraste_wcag() {
        assert!((Color::WHITE.contrast(Color::BLACK) - 21.0).abs() < 0.01);
        assert!((Color::WHITE.contrast(Color::WHITE) - 1.0).abs() < 0.001);
        // valeur de référence WCAG : #767676 sur blanc vaut tout juste 4.54:1
        let gris = Color::new(0x76, 0x76, 0x76);
        assert!(
            (gris.contrast(Color::WHITE) - 4.54).abs() < 0.02,
            "{}",
            gris.contrast(Color::WHITE)
        );
    }

    #[test]
    fn ensure_contrast_corrige_ce_qui_est_illisible() {
        let fond = Color::new(0x07, 0x11, 0x1f);
        // déjà lisible : la couleur n'est pas touchée
        assert_eq!(ensure_contrast(Color::WHITE, fond), Color::WHITE);

        // orange de marque sur blanc : 2:1, doit être assombri jusqu'à 4.5:1
        let orange = Color::new(0xff, 0x8a, 0x00);
        let fixe = ensure_contrast(orange, Color::WHITE);
        assert!(
            fixe.contrast(Color::WHITE) >= WCAG_AA,
            "{fixe} = {}",
            fixe.contrast(Color::WHITE)
        );
        assert_ne!(fixe, orange);
        // teinte conservée : ça reste plus rouge que bleu
        assert!(fixe.r > fixe.b);

        // fond sombre : on éclaircit au lieu d'assombrir
        let bleu = Color::new(0x10, 0x2a, 0x5a);
        let fixe = ensure_contrast(bleu, fond);
        assert!(fixe.contrast(fond) >= WCAG_AA);
        assert!(fixe.luminance() > bleu.luminance());

        // le pire cas — gris sur gris — reste corrigé : aucun fond ne peut échouer à la fois
        // vers le blanc et vers le noir.
        let moyen = Color::new(0x80, 0x80, 0x80);
        let fixe = ensure_contrast(Color::new(0x77, 0x77, 0x77), moyen);
        assert!(
            fixe.contrast(moyen) >= WCAG_AA,
            "{fixe} = {}",
            fixe.contrast(moyen)
        );

        // invariant général : quelle que soit la paire, la sortie est lisible
        for bg in (0..=255).step_by(17).map(|v| Color::new(v, v / 2, 255 - v)) {
            for fg in [
                Color::new(0x88, 0x88, 0x88),
                Color::new(0xff, 0x8a, 0x00),
                bg,
            ] {
                assert!(
                    ensure_contrast(fg, bg).contrast(bg) >= WCAG_AA,
                    "{fg} sur {bg}"
                );
            }
        }
    }

    /// §6bis.3 : une sortie de modèle bancale ne doit pas casser l'inscription.
    #[test]
    fn desserialisation_tolerante_de_la_recette() {
        let v: VariantSpec = serde_json::from_str(
            r#"{"name":"Corporate","template":"Founder_Motion","logo":{"anim":"sparkle","shape":"blob"},
                "accent_anim":null,"ctas":[{"label":"Réserver","target":"calendar","style":"nawak"}]}"#,
        )
        .unwrap();
        assert_eq!(v.template, Template::FounderMotion);
        assert_eq!(v.logo.shape, LogoShape::Circle); // inconnu → défaut
        assert_eq!(v.logo.anim, AnimPreset::None); // preset inconnu → aucune animation
        assert_eq!(v.accent_anim, AnimPreset::None);
        assert_eq!(v.ctas[0].target, CtaTarget::Calendar);
        assert_eq!(v.ctas[0].style, CtaStyle::Solid);
        assert_eq!(v.palette.accent, Palette::default().accent);

        // et une valeur juste reste juste
        let v: VariantSpec =
            serde_json::from_str(r#"{"template":"minimal","logo":{"anim":"glow"}}"#).unwrap();
        assert_eq!(v.template, Template::Minimal);
        assert_eq!(v.logo.anim, AnimPreset::Glow);
        // l'identifiant sérialisé est celui de templates.ts
        assert_eq!(v.template.as_str(), "minimal");
    }
}
