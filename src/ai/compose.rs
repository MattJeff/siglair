//! Composeur — contrat §6bis.1 : `VariantSpec + Brand + Profile → Doc`.
//!
//! L'IA ne produit **aucune géométrie** : elle renvoie une recette (modèle, palette, choix
//! d'animation, libellés de CTA) et c'est ce fichier — du Rust ordinaire — qui place chaque
//! élément. Conséquence directe, et tout l'intérêt de la règle : `compose` ne peut pas
//! échouer. Une recette absurde (`duration: 9999`, couleur « bleu », modèle inconnu) donne
//! le même genre de résultat qu'une bonne recette, en moins joli : toute valeur hors bornes
//! est ramenée, toute couleur illisible corrigée, tout champ absent omis. La sortie satisfait
//! **toujours** `Doc::validate()` (test `absurd_recipe_stays_valid_and_bounded`).
//!
//! Le profil n'est lu qu'**ici**, en local : le modèle, lui, n'a jamais vu autre chose que la
//! marque publique (§6bis.3). Aucune fonction de ce fichier ne sort sur le réseau — le fond
//! reste vide et le logo est un `assetId` déjà récupéré et validé par `brand.rs`. Il n'y a
//! donc pas de garde SSRF à rappeler ici : celui du §8 agit un cran plus tôt.

use super::{
    ensure_contrast, Brand, Color, Cta, CtaStyle, CtaTarget, LogoPlacement, LogoShape, Palette,
    Template, VariantSpec,
};
use crate::doc::{safe_href, Anim, AnimPreset, Canvas, Doc, Element, ElementType, Profile};

// ------------------------------------------------------------------ composition

pub fn compose(spec: &VariantSpec, brand: &Brand, profile: &Profile) -> Doc {
    let t = tpl(spec.template);
    let pal = Pal::new(&spec.palette, t.panel);
    let mut b = Build::default();

    // 1. carte de surface (neon-founder)
    if t.panel {
        b.push(Element {
            kind: ElementType::Shape,
            x: PANEL,
            y: PANEL,
            w: t.w - 2.0 * PANEL,
            h: t.h - 2.0 * PANEL,
            background: pal.surface.to_string(),
            radius: 18.0,
            opacity: 0.96,
            ..Default::default()
        });
    }

    // 2. le logo, toujours à son ratio d'aspect.
    let src = brand_logo(brand);
    // Une bannière horizontale n'a pas sa place dans une colonne carrée : elle y tomberait à
    // quelques pixels de haut. Elle passe au-dessus du texte, où la largeur est disponible.
    let top_logo =
        spec.logo.placement == LogoPlacement::Top || src.is_some_and(|(_, w, h)| w / h > 2.2);
    let (bw, bh) = if top_logo {
        ((t.w - 2.0 * t.pad).min(220.0), 44.0)
    } else {
        (t.logo_side * 1.4, t.logo_side)
    };
    let logo = src.map(|(id, iw, ih)| {
        let k = (bw / iw).min(bh / ih);
        let (w, h) = ((iw * k).max(8.0).round(), (ih * k).max(8.0).round());
        let radius = match spec.logo.shape {
            // Un cercle sur un logo qui n'est pas carré le rogne : on retombe sur l'arrondi.
            LogoShape::Circle if (0.72..1.4).contains(&(w / h)) => w.min(h) / 2.0,
            LogoShape::Circle | LogoShape::Rounded => (w.min(h) * 0.18).round(),
            LogoShape::Square => 0.0,
        };
        (id, w, h, radius)
    });

    // 3. colonne de texte : elle se resserre sur la gauche s'il n'y a pas de logo.
    let (mut text_x, mut text_top) = (t.pad, t.pad);
    match (&logo, top_logo) {
        (Some((_, _, h, _)), true) => text_top += h + 14.0,
        (Some((_, w, _, _)), false) => text_x += w + t.logo_gap,
        (None, _) => {}
    }
    let text_w = (t.w - text_x - t.pad).max(80.0);

    // 4. ce qui existe vraiment. Un champ absent du profil ne laisse ni jeton ni ligne vide.
    let head = pf(profile, "name").or_else(|| ne(brand.name.trim()));
    let sub = pf(profile, "role")
        .or_else(|| pf(profile, "company"))
        .or_else(|| ne(brand.name.trim()).filter(|n| Some(*n) != head));
    let tag = pf(profile, "tagline").or_else(|| ne(brand.tagline.trim()));
    let contact = [pf(profile, "email"), pf(profile, "phone")]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join("  ·  ");

    let mut lines = Vec::new();
    if let Some(s) = head {
        lines.push(Line::new(
            s,
            t.name_size,
            "700",
            pal.text,
            AnimPreset::Slide,
            0,
        ));
    }
    if let Some(s) = sub {
        lines.push(Line::new(
            s,
            t.role_size,
            "600",
            pal.role,
            AnimPreset::Reveal,
            1,
        ));
    }
    // La bannière EST l'accroche du modèle « sales » : elle porte la même phrase, il ne faut
    // donc pas l'écrire deux fois. Et elle ne se fait pas éjecter par le resserrement.
    match (t.banner, tag.or_else(|| ne(brand.name.trim()))) {
        (true, Some(s)) => lines.push(Line::banner(s, t.small_size, &pal)),
        (true, None) => {}
        (false, _) => {
            if let Some(s) = tag {
                lines.push(Line::new(
                    s,
                    t.small_size,
                    "400",
                    pal.muted,
                    AnimPreset::Fade,
                    3,
                ));
            }
        }
    }
    if !contact.is_empty() {
        lines.push(Line::new(
            &contact,
            t.small_size,
            "500",
            pal.muted,
            AnimPreset::Fade,
            2,
        ));
    }

    // 5. les CTA dont le profil contient réellement la cible. Les autres n'existent pas :
    //    pas de bouton à `href` vide, et pas de trou dans la rangée non plus.
    let mut ctas: Vec<(String, String, bool)> = Vec::new();
    for c in &spec.ctas {
        let (Some(href), Some(label)) = (cta_href(c.target, profile), ne(c.label.trim())) else {
            continue;
        };
        if ctas.len() == 3 || ctas.iter().any(|(_, h, _)| *h == href) {
            continue;
        }
        // Un seul bouton plein : trois aplats d'accent côte à côte n'ont plus de hiérarchie.
        let solid = c.style == CtaStyle::Solid && ctas.is_empty();
        ctas.push((fit(label, 130.0, t.small_size), href, solid));
    }

    // 6. bandes réservées en bas, puis centrage vertical du reste.
    let mut bottom = t.h - t.pad;
    if t.accent_bar {
        bottom -= 12.0;
    }
    let cta_y = (!ctas.is_empty()).then(|| {
        bottom -= t.cta_h + 16.0;
        bottom + 16.0
    });

    // Ça ne rentre pas : on retire la ligne la moins utile plutôt que de déborder du canvas.
    while stack_h(&lines) > bottom - text_top {
        let victim = lines
            .iter()
            .enumerate()
            .max_by_key(|(_, l)| l.drop)
            .filter(|(_, l)| l.drop > 0);
        match victim {
            Some((i, _)) => {
                lines.remove(i);
            }
            None => break,
        }
    }
    let mut y = text_top + ((bottom - text_top - stack_h(&lines)) / 2.0).max(0.0);

    if let Some((id, w, h, radius)) = logo {
        // Le logo occupe exactement sa propre largeur : la colonne s'est déjà réglée dessus.
        let ly = if top_logo {
            t.pad
        } else {
            text_top + ((bottom - text_top - h) / 2.0).max(0.0)
        };
        b.push(Element {
            kind: ElementType::Image,
            x: t.pad,
            y: ly,
            w,
            h,
            radius,
            asset_id: Some(id),
            content: fit(brand.name.trim(), 200.0, 10.0), // texte alternatif
            anim: anim(spec.logo.anim, spec.logo.duration, 0.0, 1.0),
            ..Default::default()
        });
        if t.divider && !top_logo {
            b.push(Element {
                kind: ElementType::Divider,
                x: text_x - t.logo_gap / 2.0,
                y: text_top,
                w: 1.0,
                h: bottom - text_top,
                background: pal.muted.to_string(),
                opacity: 0.35, // un filet, pas un trait
                ..Default::default()
            });
        }
    }

    // 7. la pile de texte
    for (i, l) in lines.iter().enumerate() {
        let delay = 0.2 + i as f64 * 0.18;
        let banner = l.kind == ElementType::Banner;
        b.push(Element {
            kind: l.kind,
            x: text_x,
            y,
            w: text_w,
            h: l.h,
            content: fit(&l.text, text_w, l.size),
            // La bannière est cliquable si — et seulement si — le profil porte un site.
            href: if banner {
                cta_href(CtaTarget::Website, profile).unwrap_or_default()
            } else {
                String::new()
            },
            font_size: l.size,
            font_weight: l.weight.into(),
            color: l.color.to_string(),
            background: l.background.to_string(),
            radius: if banner { 12.0 } else { 0.0 },
            align: if banner {
                "center".into()
            } else {
                "left".into()
            },
            anim: if banner {
                anim(spec.accent_anim, 3.0, delay, 0.8)
            } else {
                anim(l.preset, 0.9, delay, 1.0)
            },
            ..Default::default()
        });
        y += l.h + GAP;
    }

    // 8. la rangée de boutons
    if let Some(cy) = cta_y {
        let mut x = text_x;
        for (i, (label, href, solid)) in ctas.iter().enumerate() {
            let w = btn_w(label, t.small_size);
            if x + w > text_x + text_w {
                break; // ce qui ne rentre pas ne s'affiche pas — jamais un bouton coupé
            }
            let (bg, fg) = if *solid {
                (pal.accent, pal.on_accent)
            } else {
                (pal.surface, pal.accent_text)
            };
            b.push(Element {
                kind: ElementType::Button,
                x,
                y: cy,
                w,
                h: t.cta_h,
                content: label.clone(),
                href: href.clone(),
                font_size: t.small_size,
                font_weight: "700".into(),
                color: fg.to_string(),
                background: bg.to_string(),
                radius: t.cta_radius,
                align: "center".into(),
                anim: if i == 0 {
                    anim(spec.accent_anim, 2.6, 1.0, 0.7)
                } else {
                    anim(AnimPreset::Fade, 0.8, 1.0 + i as f64 * 0.15, 1.0)
                },
                ..Default::default()
            });
            x += w + 8.0;
        }
    }

    // 9. le filet d'accent (founder-motion)
    if t.accent_bar {
        b.push(Element {
            kind: ElementType::Shape,
            x: t.pad,
            y: t.h - t.pad - 8.0,
            w: t.w - 2.0 * t.pad,
            h: 2.0,
            background: pal.accent.to_string(),
            radius: 2.0,
            anim: anim(spec.accent_anim, 2.8, 0.6, 0.8),
            ..Default::default()
        });
    }

    // 6 s comme les modèles d'origine, allongé si une animation dépasse — sinon le GIF coupe
    // au milieu du mouvement. Le pipeline plafonne de toute façon à 10 s (§7.5).
    let longest = b
        .els
        .iter()
        .map(|e| e.anim.delay + e.anim.duration)
        .fold(6.0_f64, f64::max);
    Doc {
        v: 1,
        canvas: Canvas {
            width: t.w,
            height: t.h,
            bg: pal.bg.to_string(),
            bg_image: String::new(),
            overlay: 0.0,
            radius: t.radius,
        },
        elements: b.els,
        timeline_duration: longest.clamp(0.1, 10.0),
    }
}

// ------------------------------------------------------------------ repli sans clé (§6bis.5)

/// Trois directions déterministes bâties sur les couleurs extraites, sur trois modèles
/// différents. C'est ce que voient les utilisateurs quand le fournisseur d'IA est éteint :
/// moins finement adapté, mais présentable — l'inscription ne casse pas.
pub fn fallback_variants(brand: &Brand, profile: &Profile) -> Vec<VariantSpec> {
    let accent = brand
        .colors
        .first()
        .copied()
        .unwrap_or(Color::new(0x25, 0x63, 0xeb));
    let accent2 = brand.colors.get(1).copied().unwrap_or(accent);

    // Les cibles réellement disponibles, dans l'ordre d'utilité : trois propositions avec des
    // boutons morts feraient plus de mal que pas de bouton du tout.
    let avail: Vec<Cta> = [
        (CtaTarget::Website, "Site web"),
        (CtaTarget::Linkedin, "LinkedIn"),
        (CtaTarget::Whatsapp, "WhatsApp"),
        (CtaTarget::Email, "Écrire un e-mail"),
    ]
    .iter()
    .filter(|(t, _)| cta_href(*t, profile).is_some())
    .map(|(t, l)| Cta {
        label: (*l).into(),
        target: *t,
        style: CtaStyle::Solid,
    })
    .collect();
    let take = |n: usize| avail.iter().take(n).cloned().collect::<Vec<_>>();

    // ponytail : bases neutres + accents de la marque. Dériver aussi le fond de la couleur
    // dominante donnerait un aplat de marque plein écran, qui rate une fois sur deux ; le
    // jour où ça se voit, `Color::mix` vers le noir est déjà là.
    let grey = |v: u32| Color::new((v >> 16) as u8, (v >> 8) as u8, v as u8);
    vec![
        VariantSpec {
            name: "Corporate".into(),
            template: Template::FounderMotion,
            palette: Palette {
                bg: grey(0x0b1220),
                surface: grey(0x111c31),
                text: Color::WHITE,
                muted: grey(0x94a3b8),
                accent,
                accent2,
            },
            logo: super::LogoSpec {
                placement: LogoPlacement::Left,
                shape: LogoShape::Rounded,
                anim: AnimPreset::Fade,
                duration: 1.6,
            },
            accent_anim: AnimPreset::None,
            ctas: take(3),
            rationale: format!(
                "Bleu nuit sobre et accent {accent} repris de votre site : lisible en B2B, aucun mouvement inutile."
            ),
        },
        VariantSpec {
            name: "Premium".into(),
            template: Template::Minimal,
            palette: Palette {
                bg: Color::WHITE,
                surface: grey(0xf1f5f9),
                text: grey(0x0f172a),
                muted: grey(0x64748b),
                accent,
                accent2,
            },
            logo: super::LogoSpec {
                placement: LogoPlacement::Left,
                shape: LogoShape::Circle,
                anim: AnimPreset::None,
                duration: 2.4,
            },
            accent_anim: AnimPreset::Shimmer,
            ctas: take(1),
            rationale: "Fond clair, un seul bouton : la version qui passe partout, y compris chez les clients qui bloquent les images.".into(),
        },
        VariantSpec {
            name: "Animée".into(),
            template: Template::NeonFounder,
            palette: Palette {
                bg: grey(0x050814),
                surface: grey(0x0b1121),
                text: Color::WHITE,
                muted: grey(0x9db4cc),
                accent,
                accent2,
            },
            logo: super::LogoSpec {
                placement: LogoPlacement::Left,
                shape: LogoShape::Circle,
                anim: AnimPreset::Glow,
                duration: 2.4,
            },
            accent_anim: AnimPreset::Pulse,
            ctas: take(2),
            rationale: "Fond profond, halo sur le logo et bouton qui respire : la version qui se remarque dans une boîte de réception.".into(),
        },
    ]
}

// ------------------------------------------------------------------ modèles

const PANEL: f64 = 18.0;
const GAP: f64 = 7.0;

/// Géométrie reprise des 4 modèles de l'éditeur (`web/src/editor/templates.ts`) : marges,
/// tailles de police et boîte du logo. Les `y` ne sont pas repris tels quels — ils sont
/// recalculés par empilement, sinon un champ absent laisserait un trou.
struct Tpl {
    w: f64,
    h: f64,
    radius: f64,
    pad: f64,
    logo_side: f64,
    logo_gap: f64,
    name_size: f64,
    role_size: f64,
    small_size: f64,
    cta_h: f64,
    cta_radius: f64,
    panel: bool,
    divider: bool,
    banner: bool,
    accent_bar: bool,
}

fn tpl(id: Template) -> Tpl {
    let base = Tpl {
        w: 620.0,
        h: 250.0,
        radius: 18.0,
        pad: 24.0,
        logo_side: 92.0,
        logo_gap: 38.0,
        name_size: 21.0,
        role_size: 13.0,
        small_size: 10.0,
        cta_h: 32.0,
        cta_radius: 8.0,
        panel: false,
        divider: false,
        banner: false,
        accent_bar: false,
    };
    match id {
        Template::FounderMotion => Tpl {
            divider: true,
            accent_bar: true,
            ..base
        },
        Template::NeonFounder => Tpl {
            h: 240.0,
            radius: 20.0,
            pad: 42.0,
            logo_side: 82.0,
            logo_gap: 26.0,
            role_size: 11.0,
            cta_h: 34.0,
            cta_radius: 17.0,
            panel: true,
            ..base
        },
        Template::Sales => Tpl {
            radius: 10.0,
            pad: 26.0,
            logo_side: 56.0,
            logo_gap: 24.0,
            name_size: 20.0,
            role_size: 11.0,
            cta_h: 30.0,
            cta_radius: 7.0,
            banner: true,
            ..base
        },
        Template::Minimal => Tpl {
            w: 600.0,
            h: 210.0,
            radius: 12.0,
            logo_side: 72.0,
            logo_gap: 26.0,
            name_size: 19.0,
            role_size: 11.0,
            small_size: 9.0,
            cta_h: 30.0,
            cta_radius: 6.0,
            ..base
        },
    }
}

// ------------------------------------------------------------------ palette

/// La palette de la recette, rendue lisible. `brand::ensure_contrast` corrige le texte, jamais
/// le fond : le fond, c'est la marque.
struct Pal {
    bg: Color,
    surface: Color,
    text: Color,
    muted: Color,
    accent: Color,
    /// Ligne « fonction » : l'accent s'il est lisible en texte, sinon la sourdine.
    role: Color,
    /// Accent posé en texte sur `surface` (bouton secondaire).
    accent_text: Color,
    /// Libellé posé sur l'accent (bouton plein).
    on_accent: Color,
}

impl Pal {
    fn new(p: &Palette, panel: bool) -> Self {
        // Le texte repose sur la carte quand il y en a une : c'est ce fond-là qu'il faut
        // contraster, pas celui du canvas.
        let base = if panel { p.surface } else { p.bg };
        // La sourdine passe aussi par le contraste : un gris illisible reste illisible, même
        // s'il est « voulu ».
        let text = ensure_contrast(p.text, base);
        let muted = ensure_contrast(p.muted, base);
        Self {
            // Si l'accent doit être corrigé pour être lu, c'est qu'il n'est pas une couleur de
            // texte : on prend la sourdine plutôt qu'une teinte inventée.
            role: if ensure_contrast(p.accent, base) == p.accent {
                p.accent
            } else {
                muted
            },
            accent_text: ensure_contrast(p.accent, p.surface),
            on_accent: ensure_contrast(text, p.accent),
            bg: p.bg,
            surface: p.surface,
            text,
            muted,
            accent: p.accent,
        }
    }
}

// ------------------------------------------------------------------ pile de texte

struct Line {
    text: String,
    size: f64,
    h: f64,
    weight: &'static str,
    color: Color,
    background: Color,
    preset: AnimPreset,
    kind: ElementType,
    /// 0 = jamais retirée. Plus le nombre est grand, plus la ligne part tôt si ça déborde.
    drop: u8,
}

impl Line {
    fn new(
        text: &str,
        size: f64,
        weight: &'static str,
        color: Color,
        preset: AnimPreset,
        drop: u8,
    ) -> Self {
        Self {
            text: text.into(),
            size,
            h: (size * 1.5).round(),
            weight,
            color,
            background: color, // ignoré : un `text` ne peint pas son fond (html.rs)
            preset,
            kind: ElementType::Text,
            drop,
        }
    }

    fn banner(text: &str, size: f64, pal: &Pal) -> Self {
        Self {
            h: 48.0,
            background: pal.accent,
            kind: ElementType::Banner,
            ..Self::new(text, size, "600", pal.on_accent, AnimPreset::Shimmer, 0)
        }
    }
}

fn stack_h(lines: &[Line]) -> f64 {
    lines.iter().map(|l| l.h + GAP).sum::<f64>() - if lines.is_empty() { 0.0 } else { GAP }
}

// ------------------------------------------------------------------ profil → contenu

/// Valeur non vide du profil. Un champ absent ne produit pas d'élément : jamais de
/// `{{phone}}` ni de ligne vide dans un document rendu.
fn pf<'a>(p: &'a Profile, key: &str) -> Option<&'a str> {
    p.get(key).and_then(|v| ne(v.trim()))
}

fn ne(s: &str) -> Option<&str> {
    (!s.is_empty()).then_some(s)
}

/// Le logo et ses dimensions **intrinsèques** : sans elles on ne peut pas conserver son ratio.
/// L'`assetId` est rempli par la route d'onboarding ; tant qu'il n'existe pas, pas d'image —
/// un `Doc` ne porte ni octets ni URL distante (§3).
fn brand_logo(b: &Brand) -> Option<(uuid::Uuid, f64, f64)> {
    let id = b.logo_asset_id?;
    let (w, h) = b.logo.as_ref().map_or((0, 0), |l| (l.width, l.height));
    // Dimensions inconnues : on suppose carré plutôt que de renoncer au logo.
    Some(if w > 0 && h > 0 {
        (id, w as f64, h as f64)
    } else {
        (id, 1.0, 1.0)
    })
}

/// `target` (enum) → URL, depuis le profil **uniquement**. Une cible que le profil ne
/// renseigne pas est silencieusement omise, jamais rendue avec un `href` vide.
fn cta_href(target: CtaTarget, profile: &Profile) -> Option<String> {
    let raw = pf(
        profile,
        match target {
            CtaTarget::Website => "website",
            CtaTarget::Linkedin => "linkedin",
            CtaTarget::Whatsapp => "whatsapp",
            CtaTarget::Email => "email",
            // Pas de jeton `calendar` au contrat §3.1 : la clé n'existe le plus souvent pas,
            // et le bouton disparaît alors de lui-même.
            CtaTarget::Calendar => "calendar",
        },
    )?;
    let href = match target {
        CtaTarget::Email => raw.contains('@').then(|| format!("mailto:{raw}"))?,
        CtaTarget::Whatsapp if !raw.starts_with("http") => {
            let digits: String = raw.chars().filter(char::is_ascii_digit).collect();
            (digits.len() >= 6).then(|| format!("https://wa.me/{digits}"))?
        }
        _ => web_url(raw)?,
    };
    // Dernier filtre : les schémas du contrat §8, et rien d'autre.
    safe_href(&href)
}

fn web_url(v: &str) -> Option<String> {
    if v.starts_with("http://") || v.starts_with("https://") {
        Some(v.to_string())
    } else {
        // « acme.com » saisi sans schéma reste une intention claire ; « à remplir » non.
        (v.contains('.') && !v.contains(' ')).then(|| format!("https://{v}"))
    }
}

/// Tronque au lieu de déborder : on préfère « Jean-Baptiste de la Roche… » à un nom qui sort
/// du canvas.
fn fit(s: &str, w: f64, size: f64) -> String {
    let max = ((w / (size * 0.55)) as usize).clamp(6, 300);
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    while out.ends_with(' ') {
        out.pop();
    }
    out.push('…');
    out
}

fn btn_w(label: &str, size: f64) -> f64 {
    (label.chars().count() as f64 * size * 0.62 + 26.0)
        .clamp(70.0, 190.0)
        .round()
}

// ------------------------------------------------------------------ animations

/// Ramène en borne (§3) plutôt que de rejeter : le schéma JSON de l'API ne sait pas exprimer
/// un intervalle, donc `duration: 9999` est une réponse *plausible* du modèle, pas une faute.
fn anim(preset: AnimPreset, duration: f64, delay: f64, intensity: f64) -> Anim {
    let entrance = matches!(
        preset,
        AnimPreset::Slide
            | AnimPreset::Reveal
            | AnimPreset::Fade
            | AnimPreset::Draw
            | AnimPreset::Zoom
    );
    Anim {
        preset,
        duration: bound(duration, 0.1, 20.0, 2.4),
        delay: bound(delay, 0.0, 20.0, 0.0),
        intensity: bound(intensity, 0.1, 2.0, 1.0),
        iterations: if entrance {
            "1".into()
        } else {
            "infinite".into()
        },
        ..Anim::default()
    }
}

fn bound(v: f64, min: f64, max: f64, fallback: f64) -> f64 {
    if v.is_finite() {
        v.clamp(min, max)
    } else {
        fallback
    }
}

// ------------------------------------------------------------------ construction

#[derive(Default)]
struct Build {
    els: Vec<Element>,
}

impl Build {
    /// Identifiants déterministes : même recette = même signature (§6bis.1), donc testable
    /// sans appeler l'IA. `s000001` respecte `[a-z0-9]{7}`.
    ///
    /// Les coordonnées sont arrondies ici, une fois pour toutes : un `y` à 43,5 px donne un
    /// texte flou une frame sur deux dans le GIF.
    fn push(&mut self, el: Element) {
        self.els.push(Element {
            id: format!("s{:06}", self.els.len() + 1),
            x: el.x.round(),
            y: el.y.round(),
            w: el.w.max(1.0).round(),
            h: el.h.max(1.0).round(),
            ..el
        });
    }
}

// ------------------------------------------------------------------ tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::{Logo, LogoSpec};
    use uuid::Uuid;

    fn logo(w: u32, h: u32) -> Option<Logo> {
        Some(Logo {
            bytes: Vec::new(),
            content_type: "image/png".into(),
            width: w,
            height: h,
            source: "https://acme.com/logo.png".into(),
        })
    }

    fn brand() -> Brand {
        Brand {
            name: "Acme".into(),
            tagline: "L'outillage des équipes qui livrent".into(),
            colors: vec![Color::new(0x63, 0x5b, 0xff), Color::new(0x00, 0xd4, 0xff)],
            logo: logo(512, 512),
            logo_asset_id: Some(Uuid::nil()),
            ..Default::default()
        }
    }

    fn profile() -> Profile {
        Profile::from([
            ("name".into(), "Ada Lovelace".into()),
            ("role".into(), "CTO".into()),
            ("email".into(), "ada@acme.com".into()),
            ("phone".into(), "+33 6 00 00 00 00".into()),
            ("website".into(), "acme.com".into()),
            ("linkedin".into(), "https://linkedin.com/in/ada".into()),
        ])
    }

    fn spec() -> VariantSpec {
        VariantSpec {
            name: "Corporate".into(),
            template: Template::FounderMotion,
            ctas: vec![
                Cta {
                    label: "Voir le site".into(),
                    target: CtaTarget::Website,
                    style: CtaStyle::Solid,
                },
                Cta {
                    label: "LinkedIn".into(),
                    target: CtaTarget::Linkedin,
                    style: CtaStyle::Outline,
                },
                // Le profil n'a pas de `calendar` : ce bouton ne doit pas exister.
                Cta {
                    label: "Réserver".into(),
                    target: CtaTarget::Calendar,
                    style: CtaStyle::Outline,
                },
            ],
            ..Default::default()
        }
    }

    fn texts(doc: &Doc) -> String {
        doc.elements
            .iter()
            .map(|e| e.content.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn normal_recipe_composes_a_valid_doc() {
        let doc = compose(&spec(), &brand(), &profile());
        doc.validate().expect("un doc composé est toujours valide");

        assert!(texts(&doc).contains("Ada Lovelace"));
        assert!(texts(&doc).contains("ada@acme.com"));
        let btns: Vec<&Element> = doc
            .elements
            .iter()
            .filter(|e| e.kind == ElementType::Button)
            .collect();
        assert_eq!(
            btns.len(),
            2,
            "le CTA sans cible dans le profil doit être omis"
        );
        assert_eq!(
            btns[0].href, "https://acme.com",
            "le schéma manquant est complété"
        );
        // Reflow : le bouton omis ne laisse pas de trou dans la rangée.
        assert!(btns[1].x > btns[0].x && btns[1].x <= btns[0].x + btns[0].w + 12.0);
        assert!(doc
            .elements
            .iter()
            .all(|e| e.x + e.w <= doc.canvas.width + 1.0));
    }

    /// §6bis.3 : `duration: 9999` et `"bg": "bleu"` sont des réponses *plausibles* du modèle.
    /// Le chemin réel passe par serde (tolérance de `mod.rs`) puis par `compose` (bornes).
    #[test]
    fn absurd_recipe_stays_valid_and_bounded() {
        let s: VariantSpec = serde_json::from_str(
            r##"{"name":"???","template":"NEON_Founder",
                "palette":{"bg":"bleu nuit","text":"#GGG","accent":"#F0A","muted":""},
                "logo":{"placement":"nawak","shape":"blob","anim":"glow","duration":9999},
                "accent_anim":"pulse",
                "ctas":[{"label":"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx","target":"website","style":"???"}]}"##,
        )
        .expect("une recette bancale se désérialise quand même");
        let doc = compose(&s, &brand(), &profile());
        doc.validate()
            .expect("même une recette absurde donne un doc valide");

        assert_eq!(s.template, Template::NeonFounder);
        for e in &doc.elements {
            assert!(
                (0.1..=20.0).contains(&e.anim.duration),
                "durée {} hors borne",
                e.anim.duration
            );
            assert!((0.1..=2.0).contains(&e.anim.intensity));
            assert!((0.0..=20.0).contains(&e.anim.delay));
        }
        // « bleu nuit » n'existe pas : le fond retombe sur celui du défaut, pas sur du vide.
        assert_eq!(doc.canvas.bg, Palette::default().bg.to_string());
        // #F0A est une couleur valide, elle sert bien d'accent.
        assert!(doc.elements.iter().any(|e| e.background == "#ff00aa"));
        assert!(doc.timeline_duration <= 10.0);
    }

    #[test]
    fn empty_profile_leaves_no_token_and_no_ghost() {
        let doc = compose(&spec(), &brand(), &Profile::new());
        doc.validate().unwrap();

        assert!(!texts(&doc).contains("{{"), "aucun jeton résiduel");
        assert_eq!(
            doc.elements
                .iter()
                .filter(|e| e.kind == ElementType::Button)
                .count(),
            0
        );
        for e in &doc.elements {
            let bare = matches!(
                e.kind,
                ElementType::Shape | ElementType::Divider | ElementType::Image
            );
            assert!(
                bare || !e.content.trim().is_empty(),
                "élément fantôme : {e:?}"
            );
            assert!(e.href.is_empty() || safe_href(&e.href).is_some());
        }
        // La marque reste : le nom de l'entreprise n'est pas une donnée de profil.
        assert!(texts(&doc).contains("Acme"));
    }

    #[test]
    fn no_brand_no_profile_still_valid() {
        for t in [
            Template::FounderMotion,
            Template::NeonFounder,
            Template::Sales,
            Template::Minimal,
        ] {
            let s = VariantSpec {
                template: t,
                ..spec()
            };
            compose(&s, &Brand::default(), &Profile::new())
                .validate()
                .unwrap();
            compose(&s, &brand(), &profile()).validate().unwrap();
        }
    }

    #[test]
    fn a_wide_logo_is_not_crushed_into_a_circle() {
        let wide = Brand {
            logo: logo(600, 120),
            ..brand()
        };
        let s = VariantSpec {
            logo: LogoSpec {
                shape: LogoShape::Circle,
                ..Default::default()
            },
            ..spec()
        };
        let doc = compose(&s, &wide, &profile());
        let img = doc
            .elements
            .iter()
            .find(|e| e.kind == ElementType::Image)
            .unwrap();
        assert!(
            (img.w / img.h - 5.0).abs() < 0.2,
            "ratio perdu : {}×{}",
            img.w,
            img.h
        );
        assert!(
            img.radius < img.h / 2.0,
            "un logo large ne doit pas être rogné en cercle"
        );

        // Un logo carré, lui, garde bien son cercle.
        let round = compose(&s, &brand(), &profile());
        let img = round
            .elements
            .iter()
            .find(|e| e.kind == ElementType::Image)
            .unwrap();
        assert_eq!(img.radius, img.w / 2.0);
    }

    #[test]
    fn fallback_variants_are_three_distinct_valid_directions() {
        let (b, p) = (brand(), profile());
        let v = fallback_variants(&b, &p);
        assert_eq!(v.len(), 3);
        assert!(
            v[0].template != v[1].template
                && v[1].template != v[2].template
                && v[0].template != v[2].template
        );

        let mut bgs = Vec::new();
        for s in &v {
            let doc = compose(s, &b, &p);
            doc.validate().unwrap();
            assert!(!s.rationale.is_empty(), "chaque proposition s'explique");
            assert!(
                doc.elements.iter().any(|e| e.kind == ElementType::Button),
                "{} sans CTA",
                s.name
            );
            // La couleur de marque est bien passée dans le document.
            assert!(doc
                .elements
                .iter()
                .any(|e| e.background == "#635bff" || e.color == "#635bff"));
            bgs.push(doc.canvas.bg.clone());
        }
        bgs.dedup();
        assert_eq!(bgs.len(), 3, "trois directions visuellement distinctes");

        // Sans couleur extraite ni logo, le repli reste présentable.
        for s in fallback_variants(&Brand::default(), &Profile::new()) {
            compose(&s, &Brand::default(), &Profile::new())
                .validate()
                .unwrap();
        }
    }

    #[test]
    fn long_values_are_truncated_not_overflowing() {
        let mut p = profile();
        p.insert(
            "name".into(),
            "Jean-Baptiste Grenouille de la Roche-sur-Yon-et-Compagnie".into(),
        );
        p.insert("tagline".into(), "x".repeat(500));
        let doc = compose(&spec(), &brand(), &p);
        doc.validate().unwrap();
        assert!(texts(&doc).contains('…'));
        assert!(doc.elements.iter().all(|e| e.content.chars().count() < 300));
    }

    /// Le texte doit rester lisible même si le modèle propose une palette monochrome.
    #[test]
    fn unreadable_palette_is_corrected() {
        let white = Color::WHITE;
        let s = VariantSpec {
            palette: Palette {
                bg: white,
                surface: white,
                text: white,
                muted: white,
                accent: white,
                accent2: white,
            },
            ..spec()
        };
        let doc = compose(&s, &brand(), &profile());
        doc.validate().unwrap();
        assert_eq!(
            doc.canvas.bg, "#ffffff",
            "le fond, c'est la marque : on n'y touche pas"
        );
        for e in doc.elements.iter().filter(|e| e.kind == ElementType::Text) {
            assert_ne!(e.color, "#ffffff", "texte blanc sur blanc");
        }
    }
}

#[cfg(test)]
mod sweep {
    use super::*;
    use crate::ai::{Logo, LogoSpec};
    use uuid::Uuid;

    /// Balayage : `compose` doit satisfaire `Doc::validate()` pour TOUTE recette, y compris
    /// celles qu'aucun test nommé ne décrit. C'est la garantie du §6bis.1 — « le résultat est
    /// toujours un Doc valide » — vérifiée exhaustivement plutôt que sur trois exemples.
    #[test]
    fn toute_recette_donne_un_doc_valide() {
        let logo = |w: u32, h: u32| Logo {
            bytes: Vec::new(),
            content_type: "image/png".into(),
            width: w,
            height: h,
            source: String::new(),
        };
        let brands = [
            Brand::default(),
            Brand {
                name: "Acme".into(),
                logo: Some(logo(512, 512)),
                logo_asset_id: Some(Uuid::nil()),
                ..Default::default()
            },
            // logo dégénéré (1 px de haut, 0 px de large) et textes démesurés
            Brand {
                name: "É".repeat(400),
                tagline: "x ".repeat(600),
                logo: Some(logo(4000, 1)),
                logo_asset_id: Some(Uuid::nil()),
                colors: vec![Color::WHITE; 8],
                ..Default::default()
            },
            Brand {
                logo: Some(logo(0, 0)),
                logo_asset_id: Some(Uuid::nil()),
                ..Default::default()
            },
        ];
        let profiles = [
            Profile::new(),
            Profile::from([("name".into(), "Ada".into())]),
            Profile::from([
                ("name".into(), "Z".repeat(500)),
                ("role".into(), " ".repeat(50)),
                ("email".into(), "ada@acme.com".into()),
                ("phone".into(), "+33600000000".into()),
                ("website".into(), "acme.com".into()),
                ("linkedin".into(), "https://linkedin.com/in/ada".into()),
                ("whatsapp".into(), "0600000000".into()),
                ("calendar".into(), "cal.com/ada".into()),
                ("tagline".into(), "…".repeat(500)),
            ]),
        ];
        let palettes = [
            Palette::default(),
            Palette {
                bg: Color::WHITE,
                surface: Color::WHITE,
                text: Color::WHITE,
                muted: Color::WHITE,
                accent: Color::WHITE,
                accent2: Color::WHITE,
            },
            Palette {
                bg: Color::BLACK,
                surface: Color::BLACK,
                text: Color::BLACK,
                muted: Color::BLACK,
                accent: Color::BLACK,
                accent2: Color::BLACK,
            },
        ];
        let durations = [-1.0, 0.0, 1e9, f64::NAN, f64::INFINITY, 2.4];
        let mut n = 0;

        for template in [
            Template::FounderMotion,
            Template::NeonFounder,
            Template::Sales,
            Template::Minimal,
        ] {
            for shape in [LogoShape::Circle, LogoShape::Rounded, LogoShape::Square] {
                for placement in [LogoPlacement::Left, LogoPlacement::Top] {
                    for (i, palette) in palettes.iter().enumerate() {
                        for duration in durations {
                            // toutes les cibles, dont celles absentes du profil, plus un
                            // libellé vide et un libellé interminable
                            let ctas = [
                                CtaTarget::Website,
                                CtaTarget::Linkedin,
                                CtaTarget::Whatsapp,
                                CtaTarget::Email,
                                CtaTarget::Calendar,
                            ]
                            .iter()
                            .map(|t| Cta {
                                label: if i == 0 {
                                    String::new()
                                } else {
                                    "L".repeat(300)
                                },
                                target: *t,
                                style: CtaStyle::Solid,
                            })
                            .collect();
                            let spec = VariantSpec {
                                name: String::new(),
                                template,
                                palette: *palette,
                                logo: LogoSpec {
                                    placement,
                                    shape,
                                    anim: AnimPreset::Glow,
                                    duration,
                                },
                                accent_anim: AnimPreset::Shimmer,
                                ctas,
                                rationale: String::new(),
                            };
                            for b in &brands {
                                for p in &profiles {
                                    let doc = compose(&spec, b, p);
                                    doc.validate().unwrap_or_else(|e| {
                                        panic!("recette valide refusée par le validateur : {e:?}\n{spec:?}")
                                    });
                                    n += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        assert!(n > 3_000, "{n} combinaisons seulement");
    }
}
