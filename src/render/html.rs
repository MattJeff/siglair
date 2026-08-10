//! Document → HTML. **Seule** implémentation du rendu dans tout le produit (contrat §2) :
//! export, page capturée par Chromium et aperçu de l'éditeur passent tous par ici.
//!
//! Deux invariants tenus à chaque écriture dans la sortie :
//! tout ce qui vient du document est échappé, et tout `href` est re-vérifié après
//! résolution des jetons (contrat §8).

use std::fmt::Write as _;

use super::{RenderMode, RenderOpts};
use crate::doc::{
    paint_fallback_color, resolve_tokens, safe_href, safe_paint, Doc, Element, ElementType,
    Profile, ALIGNS, DIRECTIONS, EASINGS, FONT_WEIGHTS,
};
use crate::util::esc_html as esc;

const ANIM_CSS: &str = include_str!("anim.css");
/// Polices système uniquement : Chromium n'a pas de webfont et Outlook les ignore.
const FONT: &str = "-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Arial,Helvetica,sans-serif";

/// Les premières versions ajoutaient l'attribution comme un élément texte du document.
/// Le badge est désormais rendu par le serveur : ignorer ces deux libellés exacts évite
/// un doublon sur Free et laisse les anciens documents propres après un passage à Pro.
fn is_legacy_branding_element(element: &Element) -> bool {
    element.kind == ElementType::Text
        && matches!(
            element.content.trim().to_ascii_lowercase().as_str(),
            "power by siglair.com" | "powered by siglair.com"
        )
}

pub fn render_document(
    doc: &Doc,
    profile: &Profile,
    mode: RenderMode,
    opts: &RenderOpts,
) -> String {
    match mode {
        RenderMode::Hosted => hosted(doc, profile, opts),
        RenderMode::Freeform => freeform(doc, profile, opts),
        RenderMode::Safe => safe(doc, profile, opts),
    }
}

// ------------------------------------------------------------------ hosted

/// Un seul `<img>` vers le GIF hébergé : le produit qu'on facture. Les zones cliquables
/// passent par une image map quand il y a plusieurs cibles ; les clients qui la rejettent
/// (Gmail) suivent le lien global qui enveloppe l'image.
fn hosted(doc: &Doc, profile: &Profile, opts: &RenderOpts) -> String {
    // Sans slug il n'y a pas de GIF publié : mieux vaut du HTML lisible qu'une image morte.
    let Some(slug) = opts.slug.as_deref() else {
        return safe(doc, profile, opts);
    };

    let (w, h) = (doc.canvas.width_px(), doc.canvas.height_px());
    let links: Vec<(&Element, String)> = doc
        .elements
        .iter()
        .filter(|e| !is_legacy_branding_element(e))
        .filter(|e| !e.hidden)
        .filter_map(|e| link(e, profile, opts).map(|href| (e, href)))
        .collect();

    let alt = doc
        .elements
        .iter()
        .find(|e| {
            !e.hidden
                && !is_legacy_branding_element(e)
                && e.kind == ElementType::Text
                && !e.content.is_empty()
        })
        .map(|e| esc(&resolve_tokens(&e.content, profile)))
        .unwrap_or_else(|| "Signature".into());

    let map_name = format!("sg-{}", esc(slug));
    let use_map = links.len() > 1;

    let mut img = format!(
        "<img src=\"{}/s/{}.gif\" width=\"{w}\" height=\"{h}\" alt=\"{alt}\"{} \
         style=\"display:block;border:0;outline:none;text-decoration:none;width:{w}px;height:{h}px;max-width:100%;\">",
        esc(&opts.public_url),
        esc(slug),
        if use_map { format!(" usemap=\"#{map_name}\"") } else { String::new() }
    );
    if let Some((_, href)) = links.first() {
        img = format!(
            "<a href=\"{}\" target=\"_blank\" style=\"text-decoration:none;\">{img}</a>",
            esc(href)
        );
    }
    if use_map {
        let mut map = format!("<map name=\"{map_name}\">");
        for (el, href) in &links {
            let _ = write!(
                map,
                "<area shape=\"rect\" coords=\"{},{},{},{}\" href=\"{}\" target=\"_blank\" alt=\"{}\">",
                num(el.x),
                num(el.y),
                num(el.x + el.w),
                num(el.y + el.h),
                esc(href),
                esc(&resolve_tokens(&el.content, profile)),
            );
        }
        map.push_str("</map>");
        img.push_str(&map);
    }

    format!(
        "<table role=\"presentation\" border=\"0\" cellpadding=\"0\" cellspacing=\"0\" style=\"border-collapse:collapse;\">\
         <tr><td style=\"padding:0;\">{img}</td></tr>{}</table>",
        branding_row(opts)
    )
}

// ------------------------------------------------------------------ freeform

/// Positionnement absolu fidèle + animations CSS. `for_capture` produit en plus le
/// squelette de page que Chromium ouvre : taille exacte, pas de scrollbar, fond opaque.
fn freeform(doc: &Doc, profile: &Profile, opts: &RenderOpts) -> String {
    let c = &doc.canvas;
    let mut out = format!(
        "<div class=\"sg-canvas\" style=\"position:relative;overflow:hidden;box-sizing:border-box;\
         width:{}px;height:{}px;border-radius:{}px;background:{};{}\
         background-size:cover;background-position:center;font-family:{FONT};\">",
        num(c.width),
        num(c.height),
        num(c.radius),
        safe_paint(&c.bg).unwrap_or("#000000"),
        background_image(doc),
    );

    for (i, el) in doc.elements.iter().enumerate() {
        if el.hidden || is_legacy_branding_element(el) {
            continue;
        }
        let href = link(el, profile, opts);
        let mut style = format!(
            "position:absolute;box-sizing:border-box;display:flex;align-items:center;overflow:hidden;\
             white-space:pre-wrap;text-decoration:none;line-height:1.3;\
             left:{}px;top:{}px;width:{}px;height:{}px;z-index:{};opacity:{};color:{};font-size:{}px;\
             font-weight:{};text-align:{};justify-content:{};border-radius:{}px;background:{};",
            num(el.x),
            num(el.y),
            num(el.w),
            num(el.h),
            i + 1,
            num(el.opacity.clamp(0.0, 1.0)),
            css_color(&el.color, "#ffffff"),
            num(el.font_size),
            one_of(&el.font_weight, &FONT_WEIGHTS, "500"),
            one_of(&el.align, &ALIGNS, "left"),
            justify(&el.align),
            num(el.radius),
            element_background(el),
        );
        if el.rotation.abs() > f64::EPSILON {
            let _ = write!(style, "transform:rotate({}deg);", num(el.rotation));
        }
        let class = match el.anim.preset.css_name() {
            Some(name) => {
                let a = &el.anim;
                let _ = write!(
                    style,
                    "--duration:{}s;--delay:{}s;--iterations:{};--easing:{};--direction:{};--intensity:{};",
                    num(a.duration.clamp(0.1, 20.0)),
                    num(a.delay.clamp(0.0, 20.0)),
                    css_iterations(&a.iterations),
                    one_of(&a.easing, &EASINGS, "ease-in-out"),
                    one_of(&a.direction, &DIRECTIONS, "normal"),
                    num(a.intensity.clamp(0.1, 2.0)),
                );
                format!(" class=\"sg-el sg-anim anim-{name}\"")
            }
            None => " class=\"sg-el\"".to_string(),
        };

        match &href {
            Some(h) => {
                let _ = write!(
                    out,
                    "<a href=\"{}\" target=\"_blank\"{class} style=\"{style}\">",
                    esc(h)
                );
            }
            None => {
                let _ = write!(out, "<div{class} style=\"{style}\">");
            }
        }
        out.push_str(&inner(el, profile, opts, true));
        out.push_str(if href.is_some() { "</a>" } else { "</div>" });
    }
    out.push_str("</div>");
    if opts.branding {
        let _ = write!(
            out,
            "<div style=\"font-family:{FONT};font-size:10px;color:#8a94a6;padding-top:6px;\">{}</div>",
            branding_link(opts)
        );
    }

    if !opts.for_capture {
        return format!("<style>{ANIM_CSS}</style>{out}");
    }
    format!(
        "<!doctype html><html lang=\"fr\"><head><meta charset=\"utf-8\">\
         <style>html,body{{margin:0;padding:0;width:{w}px;height:{h}px;overflow:hidden;\
         background-color:{bg};}}{ANIM_CSS}</style></head><body>{out}</body></html>",
        w = c.width_px(),
        h = c.height_px(),
        // fond transparent interdit à la capture : le GIF hériterait de coins noirs
        bg = paint_fallback_color(&c.bg).unwrap_or("#ffffff"),
    )
}

// ------------------------------------------------------------------ safe

/// Tables imbriquées, empilement vertical trié par y, aucune animation, styles inline.
fn safe(doc: &Doc, profile: &Profile, opts: &RenderOpts) -> String {
    let mut els: Vec<&Element> = doc
        .elements
        .iter()
        .filter(|e| !e.hidden && !is_legacy_branding_element(e))
        .collect();
    els.sort_by(|a, b| {
        a.y.partial_cmp(&b.y)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.x.total_cmp(&b.x))
    });

    let mut rows = String::new();
    for el in els {
        let align = one_of(&el.align, &ALIGNS, "left");
        let color = css_color(&el.color, "#ffffff");
        let content = inner(el, profile, opts, false);
        if content.is_empty() && !matches!(el.kind, ElementType::Divider | ElementType::Shape) {
            continue;
        }
        let cell = match el.kind {
            ElementType::Divider | ElementType::Shape => format!(
                "<div style=\"height:{}px;line-height:0;font-size:0;background-color:{};border-radius:{}px;\">&nbsp;</div>",
                num(el.h.max(1.0)),
                element_fallback_color(el),
                num(el.radius),
            ),
            ElementType::Button | ElementType::Badge => format!(
                "<table role=\"presentation\" border=\"0\" cellpadding=\"0\" cellspacing=\"0\" style=\"border-collapse:collapse;display:inline-table;\">\
                 <tr><td style=\"background-color:{};border-radius:{}px;padding:7px 14px;color:{};font-size:{}px;font-weight:{};\">{}</td></tr></table>",
                element_fallback_color(el),
                num(el.radius),
                color,
                num(el.font_size),
                one_of(&el.font_weight, &FONT_WEIGHTS, "500"),
                wrap_link(&content, el, profile, opts, color),
            ),
            _ => wrap_link(&content, el, profile, opts, color),
        };
        let _ = write!(
            rows,
            "<tr><td style=\"padding:3px 0;font-family:{FONT};font-size:{}px;font-weight:{};color:{};text-align:{};\">{cell}</td></tr>",
            num(el.font_size),
            one_of(&el.font_weight, &FONT_WEIGHTS, "500"),
            color,
            align,
        );
    }

    format!(
        "<table role=\"presentation\" border=\"0\" cellpadding=\"0\" cellspacing=\"0\" \
         style=\"border-collapse:collapse;background-color:{};width:{}px;max-width:100%;\">\
         <tr><td style=\"padding:14px 18px;\">\
         <table role=\"presentation\" border=\"0\" cellpadding=\"0\" cellspacing=\"0\" style=\"border-collapse:collapse;width:100%;\">{rows}</table>\
         </td></tr>{}</table>",
        paint_fallback_color(&doc.canvas.bg).unwrap_or("#ffffff"),
        doc.canvas.width_px(),
        branding_row(opts),
    )
}

// ------------------------------------------------------------------ briques

/// Contenu interne d'un élément, déjà échappé. `rich` = mode freeform (vidéo autorisée).
fn inner(el: &Element, profile: &Profile, opts: &RenderOpts, rich: bool) -> String {
    match el.kind {
        ElementType::Image => match asset_url(el, opts) {
            Some(url) => format!(
                "<img src=\"{}\" alt=\"{}\" style=\"width:100%;height:100%;object-fit:contain;border:0;display:block;border-radius:{}px;\">",
                esc(url),
                esc(&resolve_tokens(&el.content, profile)),
                num(el.radius),
            ),
            None => String::new(),
        },
        ElementType::Video if rich => match asset_url(el, opts) {
            Some(url) => format!(
                "<video src=\"{}\" autoplay loop muted playsinline style=\"width:100%;height:100%;object-fit:contain;border-radius:{}px;\"></video>",
                esc(url),
                num(el.radius),
            ),
            None => String::new(),
        },
        // pas de vidéo dans un e-mail : le mode safe la saute plutôt que d'afficher un trou
        ElementType::Video | ElementType::Shape | ElementType::Divider => String::new(),
        _ => esc(&resolve_tokens(&el.content, profile)),
    }
}

/// `href` final d'un élément : jetons résolus **puis** schéma re-vérifié, et réécriture
/// vers `/c/{slug}/{id}` dès qu'un slug existe pour que le clic soit compté.
fn link(el: &Element, profile: &Profile, opts: &RenderOpts) -> Option<String> {
    let direct = safe_href(&resolve_tokens(&el.href, profile))?;
    Some(match &opts.slug {
        Some(slug) => format!("{}/c/{}/{}", opts.public_url, slug, el.id),
        None => direct,
    })
}

fn wrap_link(
    content: &str,
    el: &Element,
    profile: &Profile,
    opts: &RenderOpts,
    color: &str,
) -> String {
    match link(el, profile, opts) {
        Some(h) => format!(
            "<a href=\"{}\" target=\"_blank\" style=\"color:{color};text-decoration:none;\">{content}</a>",
            esc(&h)
        ),
        None => content.to_string(),
    }
}

fn asset_url<'a>(el: &Element, opts: &'a RenderOpts) -> Option<&'a str> {
    opts.assets.get(&el.asset_id?).map(String::as_str)
}

/// Reprend la règle de l'éditeur : seul `divider` peint son fond parmi les types « nus ».
fn element_background(el: &Element) -> &str {
    match el.kind {
        ElementType::Text | ElementType::Image | ElementType::Video => "transparent",
        _ => safe_paint(&el.background).unwrap_or("transparent"),
    }
}

fn element_fallback_color(el: &Element) -> &str {
    match el.kind {
        ElementType::Text | ElementType::Image | ElementType::Video => "transparent",
        _ => paint_fallback_color(&el.background).unwrap_or("transparent"),
    }
}

fn background_image(doc: &Doc) -> String {
    let c = &doc.canvas;
    // L'URL est validée à l'enregistrement (garde SSRF) ; ici on vérifie surtout qu'elle ne
    // peut pas s'échapper de `url(...)` pour injecter une déclaration CSS.
    let url = c.bg_image.trim();
    let clean = !url.is_empty()
        && (url.starts_with("http://") || url.starts_with("https://"))
        && !url.bytes().any(|b| {
            b <= b' ' || matches!(b, b'"' | b'\'' | b'(' | b')' | b'\\' | b';' | b'<' | b'>')
        });
    if !clean {
        return String::new();
    }
    let o = num(c.overlay.clamp(0.0, 1.0));
    format!(
        "background-image:linear-gradient(rgba(0,0,0,{o}),rgba(0,0,0,{o})),url(\"{}\");",
        esc(url)
    )
}

fn branding_link(opts: &RenderOpts) -> String {
    format!(
        "<a href=\"{}\" target=\"_blank\" style=\"font-size:10px;color:#576274;text-decoration:underline;\">Powered by siglair.com</a>",
        esc(&opts.public_url)
    )
}

fn branding_row(opts: &RenderOpts) -> String {
    if !opts.branding {
        return String::new();
    }
    format!(
        "<tr><td style=\"padding:6px 0 0;font-family:{FONT};font-size:10px;line-height:1.4;text-align:left;\">{}</td></tr>",
        branding_link(opts)
    )
}

fn num(v: f64) -> String {
    let v = if v.is_finite() { v } else { 0.0 };
    let r = (v * 1000.0).round() / 1000.0;
    if r == r.trunc() {
        format!("{}", r as i64)
    } else {
        format!("{r}")
    }
}

/// Une couleur non validée n'entre jamais telle quelle dans un attribut `style`.
fn css_color<'a>(v: &'a str, fallback: &'a str) -> &'a str {
    if v.len() == 7 && v.starts_with('#') && v[1..].bytes().all(|b| b.is_ascii_hexdigit()) {
        v
    } else {
        fallback
    }
}

fn one_of<'a>(v: &'a str, allowed: &[&'a str], fallback: &'a str) -> &'a str {
    if allowed.contains(&v) {
        v
    } else {
        fallback
    }
}

fn css_iterations(v: &str) -> String {
    if v == "infinite" {
        return "infinite".into();
    }
    v.parse::<u32>().unwrap_or(1).clamp(1, 10).to_string()
}

fn justify(align: &str) -> &'static str {
    match align {
        "center" => "center",
        "right" => "flex-end",
        _ => "flex-start",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{Anim, AnimPreset, Canvas};

    fn opts(slug: Option<&str>) -> RenderOpts {
        RenderOpts {
            public_url: "https://siglair.app".into(),
            slug: slug.map(String::from),
            ..Default::default()
        }
    }

    #[test]
    fn freeform_renders_gradients_and_safe_uses_the_first_color() {
        let mut d = doc_with(vec![Element {
            id: "aaa1111".into(),
            kind: ElementType::Button,
            content: "Démo".into(),
            background: "linear-gradient(125deg, #123456 0%, #abcdef 100%)".into(),
            ..Default::default()
        }]);
        d.canvas.bg = "radial-gradient(circle at center, #102030 0%, #405060 100%)".into();

        let freeform = render_document(&d, &Profile::new(), RenderMode::Freeform, &opts(None));
        assert!(freeform
            .contains("background:radial-gradient(circle at center, #102030 0%, #405060 100%)"));
        assert!(freeform.contains("background:linear-gradient(125deg, #123456 0%, #abcdef 100%)"));

        let safe = render_document(&d, &Profile::new(), RenderMode::Safe, &opts(None));
        assert!(safe.contains("background-color:#102030"));
        assert!(safe.contains("background-color:#123456"));
    }

    fn doc_with(els: Vec<Element>) -> Doc {
        Doc {
            canvas: Canvas::default(),
            elements: els,
            ..Default::default()
        }
    }

    fn el(id: &str) -> Element {
        Element {
            id: id.into(),
            ..Default::default()
        }
    }

    #[test]
    fn escapes_content_and_drops_javascript_href() {
        let d = doc_with(vec![Element {
            content: "<script>alert('xss')</script> & \"co\"".into(),
            href: "javascript:alert(1)".into(),
            ..el("aaa1111")
        }]);
        for mode in [RenderMode::Freeform, RenderMode::Safe] {
            let h = render_document(&d, &Profile::new(), mode, &opts(None));
            assert!(
                !h.contains("<script>"),
                "{mode:?} laisse passer un <script>"
            );
            assert!(
                h.contains("&lt;script&gt;"),
                "{mode:?} n'échappe pas le contenu"
            );
            assert!(
                !h.contains("javascript:"),
                "{mode:?} laisse passer un href javascript:"
            );
            assert!(
                !h.contains("<a "),
                "{mode:?} a produit un lien à partir d'un href refusé"
            );
        }
    }

    #[test]
    fn resolves_tokens_without_residue() {
        let profile = Profile::from([("name".into(), "Ada Lovelace".into())]);
        let d = doc_with(vec![Element {
            content: "{{name}} — {{unknown}}".into(),
            href: "mailto:{{email}}".into(),
            ..el("aaa1111")
        }]);
        for mode in [RenderMode::Freeform, RenderMode::Safe] {
            let h = render_document(&d, &profile, mode, &opts(None));
            assert!(h.contains("Ada Lovelace"));
            assert!(!h.contains("{{"), "{mode:?} laisse un jeton non résolu");
        }
    }

    #[test]
    fn safe_mode_has_no_animation() {
        let d = doc_with(vec![Element {
            content: "Bouge".into(),
            anim: Anim {
                preset: AnimPreset::Pulse,
                ..Default::default()
            },
            ..el("aaa1111")
        }]);
        let h = render_document(&d, &Profile::new(), RenderMode::Safe, &opts(None));
        assert!(!h.contains("animation"));
        assert!(!h.contains("anim-pulse"));
        assert!(!h.contains("@keyframes"));

        let f = render_document(&d, &Profile::new(), RenderMode::Freeform, &opts(None));
        assert!(f.contains("anim-pulse") && f.contains("@keyframes sg-pulse"));
    }

    #[test]
    fn rewrites_links_through_click_tracker() {
        let d = doc_with(vec![Element {
            content: "Site".into(),
            href: "https://ada.dev".into(),
            ..el("aaa1111")
        }]);
        let tracked = render_document(
            &d,
            &Profile::new(),
            RenderMode::Freeform,
            &opts(Some("abcdefgh2345")),
        );
        assert!(tracked.contains("https://siglair.app/c/abcdefgh2345/aaa1111"));
        assert!(!tracked.contains("https://ada.dev"));

        let direct = render_document(&d, &Profile::new(), RenderMode::Freeform, &opts(None));
        assert!(direct.contains("href=\"https://ada.dev\""));
    }

    #[test]
    fn hosted_points_at_the_gif_and_falls_back_without_slug() {
        let d = doc_with(vec![
            Element {
                content: "Site".into(),
                href: "https://ada.dev".into(),
                ..el("aaa1111")
            },
            Element {
                content: "Mail".into(),
                href: "mailto:a@b.c".into(),
                ..el("bbb2222")
            },
        ]);
        let h = render_document(
            &d,
            &Profile::new(),
            RenderMode::Hosted,
            &opts(Some("abcdefgh2345")),
        );
        assert!(h.contains("<img src=\"https://siglair.app/s/abcdefgh2345.gif\""));
        assert!(h.contains("usemap=\"#sg-abcdefgh2345\"") && h.contains("<area "));
        assert!(h.contains("/c/abcdefgh2345/bbb2222"));

        // pas de slug = pas de GIF publié : on retombe sur du HTML lisible, pas une image morte
        let none = render_document(&d, &Profile::new(), RenderMode::Hosted, &opts(None));
        assert!(!none.contains("<img") && none.contains("<table"));
    }

    #[test]
    fn free_branding_is_server_enforced_in_every_export_mode() {
        let d = doc_with(vec![
            el("aaa1111"),
            Element {
                id: "bbb2222".into(),
                content: "Power by siglair.com".into(),
                ..Element::default()
            },
        ]);
        for mode in [RenderMode::Hosted, RenderMode::Freeform, RenderMode::Safe] {
            let mut branded = opts(Some("abcdefgh2345"));
            branded.branding = true;
            let free = render_document(&d, &Profile::new(), mode, &branded);
            assert!(
                free.contains("Powered by siglair.com"),
                "le mode {mode:?} a perdu la marque Free"
            );
            assert_eq!(
                free.matches("Powered by siglair.com").count(),
                1,
                "le mode {mode:?} a doublé une ancienne marque"
            );
            assert!(!free.contains("Power by siglair.com"));

            let paid = render_document(&d, &Profile::new(), mode, &opts(Some("abcdefgh2345")));
            assert!(
                !paid.contains("Powered by siglair.com"),
                "le mode {mode:?} a marqué un plan payant"
            );
            assert!(!paid.contains("Power by siglair.com"));
        }
    }

    #[test]
    fn capture_page_is_exact_sized_and_opaque() {
        let d = doc_with(vec![el("aaa1111")]);
        let o = RenderOpts {
            for_capture: true,
            ..opts(None)
        };
        let h = render_document(&d, &Profile::new(), RenderMode::Freeform, &o);
        assert!(h.starts_with("<!doctype html>"));
        assert!(h.contains("width:620px;height:250px;overflow:hidden"));
        assert!(h.contains("background-color:#07111f"));
    }

    #[test]
    fn hostile_style_values_never_reach_the_output() {
        let d = doc_with(vec![Element {
            color: "red;background:url(http://evil/x)".into(),
            font_weight: "500;position:fixed".into(),
            align: "left;x".into(),
            anim: Anim {
                preset: AnimPreset::Pulse,
                easing: "ease;--x:url(http://evil)".into(),
                iterations: "infinite;color:red".into(),
                direction: "normal;z-index:9".into(),
                ..Default::default()
            },
            ..el("aaa1111")
        }]);
        let h = render_document(&d, &Profile::new(), RenderMode::Freeform, &opts(None));
        assert!(!h.contains("evil"));
        assert!(!h.contains("position:fixed"));
    }
}
