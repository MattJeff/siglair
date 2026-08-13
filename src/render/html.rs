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

/// Découpe les éléments en COLONNES par les vides horizontaux.
///
/// Une signature n'est pas une pile de lignes : c'est « logo à gauche, bloc texte à
/// droite », et c'est exactement ce que produit une table e-mail. On cherche donc d'abord
/// les gouttières verticales — les bandes de `x` que rien ne traverse — puis on empile à
/// l'intérieur de chaque colonne.
fn columns<'a>(els: &[&'a Element]) -> Vec<Vec<&'a Element>> {
    let mut by_x: Vec<&Element> = els.to_vec();
    by_x.sort_by(|a, b| a.x.total_cmp(&b.x));

    let mut out: Vec<Vec<&Element>> = Vec::new();
    let mut right = f64::MIN;
    for el in by_x {
        // 12 px : en dessous, deux éléments se touchent presque et appartiennent au même
        // bloc visuel. Au-dessus, l'œil voit deux colonnes.
        if out.is_empty() || el.x > right + 12.0 {
            out.push(vec![el]);
            right = el.x + el.w.max(1.0);
        } else {
            if let Some(col) = out.last_mut() {
                col.push(el);
            }
            right = right.max(el.x + el.w.max(1.0));
        }
    }
    for col in &mut out {
        col.sort_by(|a, b| {
            a.y.partial_cmp(&b.y)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.x.total_cmp(&b.x))
        });
    }
    out
}

/// Dans une colonne, regroupe ce qui est visuellement sur la même ligne.
///
/// Critère : les centres verticaux sont proches au regard de la plus grande des deux
/// hauteurs. Trois boutons posés au même `y` forment une ligne ; un nom et une fonction
/// espacés de 36 px n'en forment pas.
fn rows_in_column<'a>(col: &[&'a Element]) -> Vec<Vec<&'a Element>> {
    let mut out: Vec<Vec<&Element>> = Vec::new();
    for el in col {
        let joins = out.last().is_some_and(|row: &Vec<&Element>| {
            row.iter().any(|o| {
                let (ca, cb) = (o.y + o.h.max(1.0) / 2.0, el.y + el.h.max(1.0) / 2.0);
                (ca - cb).abs() <= o.h.max(1.0).max(el.h.max(1.0)) * 0.45
            })
        });
        if joins {
            if let Some(row) = out.last_mut() {
                row.push(el);
            }
        } else {
            out.push(vec![el]);
        }
    }
    for row in &mut out {
        row.sort_by(|a, b| a.x.total_cmp(&b.x));
    }
    out
}

/// Une ligne visuelle → un `<tr>`, un `<td>` par élément, largeurs en pixels.
///
/// Les largeurs sont en pixels et pas en pourcentage : Outlook desktop ignore les
/// pourcentages sur les cellules imbriquées, une largeur en pixels est la seule chose
/// qu'il respecte de façon fiable.
fn safe_row(row: &[&Element], doc: &Doc, profile: &Profile, opts: &RenderOpts) -> String {
    let left = row.iter().map(|e| e.x).fold(f64::MAX, f64::min);
    let mut cells = String::new();
    let mut cursor = left;
    let row_h = row
        .iter()
        .map(|e| e.h.max(1.0))
        .fold(0.0f64, f64::max)
        .max(1.0);

    for el in row {
        // Gouttière : sans elle, deux boutons distants de 8 px se retrouvent collés.
        let gap = el.x - cursor;
        if gap > 4.0 {
            let _ = write!(
                cells,
                "<td width=\"{}\" style=\"width:{}px;font-size:0;line-height:0;\">&nbsp;</td>",
                gap.round() as i64,
                num(gap)
            );
        }
        cells.push_str(&safe_cell(el, row_h, profile, opts));
        cursor = el.x + el.w.max(1.0);
    }

    let _ = doc;
    format!("<tr>{cells}</tr>")
}

fn safe_cell(el: &Element, row_h: f64, profile: &Profile, opts: &RenderOpts) -> String {
    let align = one_of(&el.align, &ALIGNS, "left");
    let color = css_color(&el.color, "#ffffff");
    let w = el.w.max(1.0);
    let content = inner(el, profile, opts, false);

    let body = match el.kind {
        // Un séparateur plus haut que large est VERTICAL : une cellule fine dont le fond
        // fait la ligne. L'ancienne version en faisait un bloc pleine largeur — c'est ce
        // qui produisait le grand rectangle gris en haut de la signature.
        ElementType::Divider | ElementType::Shape if el.h > el.w => format!(
            "<div style=\"width:{}px;height:{}px;line-height:0;font-size:0;background-color:{};border-radius:{}px;\">&nbsp;</div>",
            num(w),
            num(el.h.max(1.0)),
            element_fallback_color(el),
            num(el.radius),
        ),
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

    let valign = if row_h > el.h * 1.5 { "middle" } else { "top" };
    format!(
        "<td width=\"{}\" valign=\"{valign}\" style=\"width:{}px;padding:3px 0;font-family:{FONT};\
         font-size:{}px;font-weight:{};color:{};text-align:{align};\">{body}</td>",
        w.round() as i64,
        num(w),
        num(el.font_size),
        one_of(&el.font_weight, &FONT_WEIGHTS, "500"),
        color,
    )
}

// ------------------------------------------------------------------ safe

/// Tables imbriquées, aucune animation, styles inline — le mode qui survit à Outlook.
///
/// Le canvas est en positionnement absolu ; l'e-mail, lui, n'a que des tables. On ne peut
/// donc pas reproduire la mise en page au pixel — c'est précisément pour ça que le produit
/// vend un GIF hébergé. Mais on peut faire beaucoup mieux qu'un empilement.
///
/// Une première version triait par `y` et émettait UNE LIGNE PAR ÉLÉMENT. Résultat sur une
/// signature normale : le logo posé à gauche du texte se retrouvait seul sur sa ligne, les
/// trois boutons l'un sous l'autre, et le séparateur vertical de 1×150 px devenait un pavé
/// gris pleine largeur. Rien à voir avec l'éditeur.
///
/// On reconstitue donc de vraies lignes par RECOUVREMENT VERTICAL, puis des colonnes par
/// `x`. Deux éléments dont les bandes verticales se chevauchent appartiennent à la même
/// ligne visuelle : c'est ce que fait l'œil, et ça suffit à retrouver « logo à gauche,
/// texte à droite » et « trois boutons côte à côte ».
fn safe(doc: &Doc, profile: &Profile, opts: &RenderOpts) -> String {
    let mut els: Vec<&Element> = doc
        .elements
        .iter()
        .filter(|e| !e.hidden && !is_legacy_branding_element(e))
        .filter(|e| {
            !inner(e, profile, opts, false).is_empty()
                || matches!(e.kind, ElementType::Divider | ElementType::Shape)
        })
        .collect();
    els.sort_by(|a, b| {
        a.y.partial_cmp(&b.y)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.x.total_cmp(&b.x))
    });

    // Un séparateur nettement plus haut que large qui traverse la carte est de la pure
    // décoration : en table, il ne peut s'exprimer qu'avec un rowspan, que Outlook rend
    // mal, et il chevauche toutes les autres lignes — c'est lui qui écrasait la mise en
    // page en un seul bloc. On le laisse au GIF, qui le rend parfaitement.
    let spanning = doc.canvas.height * 0.5;
    els.retain(|e| {
        !(matches!(e.kind, ElementType::Divider | ElementType::Shape)
            && e.h > e.w * 4.0
            && e.h >= spanning)
    });

    let cols = columns(&els);
    let mut rows = String::new();

    if cols.len() > 1 {
        // Plusieurs colonnes : une seule ligne extérieure, une cellule par colonne, et
        // l'empilement se fait dans une table imbriquée. C'est la structure classique
        // d'une signature e-mail, et la seule qui tienne dans Outlook.
        let mut cells = String::new();
        for (i, col) in cols.iter().enumerate() {
            let w = col
                .iter()
                .map(|e| e.x + e.w.max(1.0))
                .fold(0.0f64, f64::max)
                - col.iter().map(|e| e.x).fold(f64::MAX, f64::min);
            let inner_rows: String = rows_in_column(col)
                .iter()
                .map(|r| safe_row(r, doc, profile, opts))
                .collect();
            let pad = if i + 1 < cols.len() { "0 14px 0 0" } else { "0" };
            let _ = write!(
                cells,
                "<td width=\"{}\" valign=\"top\" style=\"width:{}px;padding:{pad};\">\
                 <table role=\"presentation\" border=\"0\" cellpadding=\"0\" cellspacing=\"0\" \
                 style=\"border-collapse:collapse;\">{inner_rows}</table></td>",
                w.round().max(1.0) as i64,
                num(w.max(1.0)),
            );
        }
        let _ = write!(rows, "<tr>{cells}</tr>");
    } else {
        for row in rows_in_column(&els) {
            rows.push_str(&safe_row(&row, doc, profile, opts));
        }
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
                "<img src=\"{}\" alt=\"{}\" width=\"{}\" height=\"{}\" style=\"width:{}px;height:{}px;object-fit:contain;border:0;display:block;border-radius:{}px;\">",
                esc(url),
                esc(&resolve_tokens(&el.content, profile)),
                // Attributs width/height EN PLUS du style : Outlook desktop ignore les
                // dimensions CSS sur <img> et afficherait l'image à sa taille native.
                el.w.max(1.0).round() as i64,
                el.h.max(1.0).round() as i64,
                num(el.w.max(1.0)),
                num(el.h.max(1.0)),
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

/// Le badge de la marque imposée sur Free (contrat §11.2) — le seul canal d'acquisition
/// gratuit du produit, donc attribué et compté.
///
/// Trois choix tenus ici :
///
/// - **du texte, jamais gravé dans le GIF** : un badge dans l'image disparaît quand le
///   client mail bloque les images, c'est-à-dire exactement quand on a le plus besoin
///   qu'il reste visible ; il ne serait pas cliquable séparément du CTA du client ; et du
///   texte pèse zéro octet, Outlook 2016 compris ;
/// - **tout le libellé est dans le `<a>`** : la zone cliquable est le badge entier, et la
///   couleur ne dépend plus du conteneur — le bloc `freeform` impose un gris clair à 10 px
///   à ce qu'il entoure, illisible pour un lien ;
/// - **contraste** : #576274 sur blanc donne 6,1:1, au-delà de l'AA exigé pour du texte
///   sous 14 px (DESIGN.md). `--muted` (#8390a4) n'atteint pas 4,5:1 à cette taille — la
///   charte le dit elle-même — et un badge illisible ne convertit pas.
fn branding_link(opts: &RenderOpts) -> String {
    // Le code d'attribution EST le slug public (contrat §11.2 : `/r/{slug}`) : il identifie
    // déjà la signature, il est stable et il n'y a pas de seconde colonne à tenir en phase.
    // Sans slug (aperçu de l'éditeur, signature non publiée) il n'y a rien à attribuer :
    // le badge pointe la racine plutôt qu'un `/r/` vide.
    let href = match opts.slug.as_deref() {
        Some(slug) => format!("{}/r/{slug}", opts.public_url),
        None => opts.public_url.clone(),
    };
    format!(
        "<a href=\"{}\" target=\"_blank\" style=\"font-size:10px;color:#576274;text-decoration:underline;\">Powered by siglair.com</a>",
        esc(&href)
    )
}

fn branding_row(opts: &RenderOpts) -> String {
    if !opts.branding {
        return String::new();
    }
    // La couleur et la taille sont portées par le `<a>` : un `<td>` gris clair repeindrait
    // le lien dans les clients qui héritent la couleur du parent.
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
            public_url: "https://siglair.com".into(),
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

    /// Reproduit la mise en page réelle du modèle « Neon Founder » : logo à gauche,
    /// séparateur vertical, textes à droite, trois boutons côte à côte.
    ///
    /// La première version du mode safe triait par `y` et sortait UNE LIGNE PAR ÉLÉMENT.
    /// Dans Gmail, ça donnait : un pavé gris pleine largeur (le séparateur vertical de
    /// 1×150), le logo étiré en carré géant, et les trois boutons empilés. Un utilisateur
    /// l'a signalé sur sa vraie signature, capture à l'appui.
    #[test]
    fn safe_reconstitue_les_lignes_au_lieu_de_tout_empiler() {
        let asset = uuid::Uuid::nil();
        let mut o = opts(None);
        o.assets.insert(asset, "https://siglair.com/f/logo.png".into());
        let d = doc_with(vec![
            Element { kind: ElementType::Image, x: 24.0,  y: 48.0,  w: 92.0,  h: 92.0,
                      asset_id: Some(asset), ..el("img0001") },
            Element { kind: ElementType::Divider, x: 132.0, y: 35.0, w: 1.0,  h: 150.0, background: "#46556c".into(), ..el("div0001") },
            Element { x: 154.0, y: 39.0,  w: 355.0, h: 31.0, content: "Mathis Higuinen".into(), ..el("nom0001") },
            Element { x: 154.0, y: 75.0,  w: 350.0, h: 22.0, content: "Founder".into(), ..el("rol0001") },
            Element { kind: ElementType::Button, x: 154.0, y: 165.0, w: 90.0, h: 32.0, content: "Site".into(),     href: "https://a.test".into(), ..el("btn0001") },
            Element { kind: ElementType::Button, x: 252.0, y: 165.0, w: 94.0, h: 32.0, content: "LinkedIn".into(), href: "https://b.test".into(), ..el("btn0002") },
            Element { kind: ElementType::Button, x: 354.0, y: 165.0, w: 94.0, h: 32.0, content: "WhatsApp".into(), href: "https://c.test".into(), ..el("btn0003") },
        ]);
        let h = render_document(&d, &Profile::new(), RenderMode::Safe, &o);

        // Le logo et le bloc texte sont DEUX COLONNES : le logo doit précéder le nom
        // sans qu'aucune fin de ligne ne les sépare. Sinon on est retombé sur l'empilement.
        let (i_logo, i_nom) = (h.find("logo.png").unwrap(), h.find("Mathis").unwrap());
        assert!(i_logo < i_nom, "le logo passe après le texte");
        // Deux colonnes produisent « …</tr></table></td><td…><table…><tr>… » ; un
        // empilement produirait « </tr><tr> ». C'est le seul discriminant fiable, la
        // fermeture de la table imbriquée du logo contenant forcément un </tr>.
        assert!(
            !h[i_logo..i_nom].contains("</tr><tr>"),
            "le logo est sur sa propre ligne au lieu d'être à gauche du texte : {h}"
        );

        // Nom et fonction sont empilés : eux DOIVENT être séparés par une fin de ligne.
        let (a, b) = (h.find("Mathis").unwrap(), h.find("Founder").unwrap());
        assert!(
            h[a..b].contains("</tr>"),
            "le nom et la fonction sont sur la même ligne"
        );

        // Les trois boutons partagent une ligne. Chaque bouton étant lui-même une petite
        // table, on cherche « </tr><tr> » — la signature d'un vrai changement de ligne —
        // et pas un <tr> quelconque.
        let entre = &h[h.find("Site").unwrap()..h.find("WhatsApp").unwrap()];
        assert!(
            !entre.contains("</tr><tr>"),
            "les boutons sont empilés alors qu'ils étaient côte à côte dans l'éditeur"
        );

        // Le séparateur vertical qui traverse la carte est ABANDONNÉ en mode safe.
        // En table il exigerait un rowspan, qu'Outlook rend mal, et il chevauche toutes
        // les lignes — c'est lui qui écrasait la mise en page en un seul bloc. Le rendre
        // en pavé gris pleine largeur, comme avant, était bien pire que ne pas le rendre :
        // le GIF hébergé, lui, le restitue parfaitement.
        assert!(
            !h.contains("#46556c"),
            "le séparateur pleine hauteur ne doit pas être rendu en mode safe : {h}"
        );

        // L'image porte ses dimensions réelles, pas 100 % (Outlook ignore le CSS sur <img>).
        assert!(
            h.contains("width=\"92\" height=\"92\"") && !h.contains("width:100%;height:100%"),
            "l'image est étirée au lieu de faire 92×92"
        );
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
        assert!(tracked.contains("https://siglair.com/c/abcdefgh2345/aaa1111"));
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
        assert!(h.contains("<img src=\"https://siglair.com/s/abcdefgh2345.gif\""));
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
