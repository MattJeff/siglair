/**
 * Importer une signature depuis du HTML collé.
 *
 * Cas principal : reprendre une signature exportée par Siglair (mode « Libre », le HTML
 * en `position:absolute`) pour la modifier sans repartir d'une page blanche. Ça marche
 * aussi, en dégradé, sur du HTML venu d'ailleurs — mais on ne le promet pas : on dit ce
 * qu'on n'a pas su lire.
 *
 * Pourquoi ici et pas côté serveur : le navigateur a déjà un analyseur HTML complet et
 * conforme. `DOMParser` produit un document INERTE — aucun script n'est exécuté, aucune
 * image n'est chargée, aucun style externe n'est appliqué. On ne lit que des attributs
 * `style` en ligne. Le Doc produit repassera de toute façon par `Doc::validate()` côté
 * serveur à l'enregistrement, qui refuse les `javascript:` et borne les valeurs.
 *
 * Ce fichier ne rend RIEN. Le contrat §2 n'est pas concerné : il interdit de réimplémenter
 * le rendu en TypeScript, pas de lire du HTML.
 */
import type { Anim, AnimPreset, Asset, Doc, Element, ElementType } from '../lib/types';
import { ANIM_PRESETS, ELEMENT_TYPES } from '../lib/types';
import { ANIM_DEFAULT, CANVAS_DEFAULT, makeElement } from './state';

export interface ImportResult {
  doc: Doc;
  /** Ce qui n'a pas pu être repris, en clair. Affiché tel quel à l'utilisateur. */
  notes: string[];
}

/** `rgb(7, 17, 31)` ou `#07111f` → `#07111f`. Une couleur illisible renvoie `null`. */
function toHex(raw: string | undefined): string | null {
  if (!raw) return null;
  const v = raw.trim();
  if (/^#[0-9a-f]{6}$/i.test(v)) return v.toLowerCase();
  if (/^#[0-9a-f]{3}$/i.test(v)) {
    return `#${v[1]}${v[1]}${v[2]}${v[2]}${v[3]}${v[3]}`.toLowerCase();
  }
  const m = v.match(/rgba?\(\s*(\d+)[,\s]+(\d+)[,\s]+(\d+)/i);
  if (!m) return null;
  const hex = (n: string) => Number(n).toString(16).padStart(2, '0');
  return `#${hex(m[1])}${hex(m[2])}${hex(m[3])}`;
}

/** Première couleur d'un dégradé — l'éditeur ne manipule qu'une couleur de fond. */
function firstColor(raw: string | undefined): string | null {
  if (!raw) return null;
  if (!raw.includes('gradient')) return toHex(raw);
  const m = raw.match(/(#[0-9a-f]{3,6}|rgba?\([^)]+\))/i);
  return m ? toHex(m[1]) : null;
}

const px = (raw: string | undefined): number | null => {
  if (!raw) return null;
  const n = Number.parseFloat(raw);
  return Number.isFinite(n) ? n : null;
};

/** `animation-name: sg-draw` → `draw`. Un nom inconnu renvoie `none`. */
function presetOf(style: CSSStyleDeclaration): AnimPreset {
  const name = style.animationName || '';
  const m = name.match(/sg-([a-z]+)/i);
  const found = m?.[1]?.toLowerCase();
  return found && (ANIM_PRESETS as readonly string[]).includes(found)
    ? (found as AnimPreset)
    : 'none';
}

function animOf(el: HTMLElement): Partial<Anim> {
  const s = el.style;
  const preset = presetOf(s);
  if (preset === 'none') return { preset };
  // Les durées sont écrites dans des variables CSS par l'export : on les relit là, et on
  // retombe sur les propriétés d'animation si elles manquent.
  const num = (v: string, fallback: number) => {
    const n = Number.parseFloat(v);
    return Number.isFinite(n) ? n : fallback;
  };
  // Le type n'accepte que « infinite » ou 1..10 : une valeur hors liste retombe sur le
  // défaut plutôt que d'élargir le type pour accueillir une donnée qu'on ne contrôle pas.
  const raw = (s.getPropertyValue('--iterations') || s.animationIterationCount).trim();
  const iterations: Anim['iterations'] =
    raw === 'infinite' || /^([1-9]|10)$/.test(raw)
      ? (raw as Anim['iterations'])
      : ANIM_DEFAULT.iterations;
  return {
    preset,
    duration: num(s.getPropertyValue('--duration') || s.animationDuration, ANIM_DEFAULT.duration),
    delay: num(s.getPropertyValue('--delay') || s.animationDelay, ANIM_DEFAULT.delay),
    iterations,
    intensity: num(s.getPropertyValue('--intensity'), ANIM_DEFAULT.intensity),
  };
}

/**
 * Devine le type d'un bloc.
 *
 * L'export ne pose pas de marqueur de type — il n'a jamais eu à être relu. On déduit donc
 * de la forme, dans cet ordre : une image saute aux yeux, un bloc vide et fin est un trait,
 * un bloc coloré avec du texte est un bouton. Le reste est du texte.
 */
function typeOf(el: HTMLElement, text: string, bg: string | null): ElementType {
  if (el.querySelector('img')) return 'image';
  if (el.querySelector('video')) return 'video';
  const w = px(el.style.width) ?? 0;
  const h = px(el.style.height) ?? 0;
  if (!text) return h <= 4 || w <= 4 ? 'divider' : 'shape';
  if (bg && bg !== 'transparent') {
    return el.tagName === 'A' || (px(el.style.borderRadius) ?? 0) > 0 ? 'button' : 'badge';
  }
  return 'text';
}

/**
 * @param html   le HTML collé par l'utilisateur
 * @param assets les médias de l'organisation, pour retrouver un `assetId` depuis une URL
 */
export function importSignatureHtml(html: string, assets: readonly Asset[] = []): ImportResult {
  const notes: string[] = [];
  const parsed = new DOMParser().parseFromString(html, 'text/html');

  // Le conteneur : le premier bloc positionné qui a une largeur explicite. On ne prend pas
  // <body> — le HTML collé est souvent un fragment enveloppé par l'éditeur du client mail.
  const container =
    parsed.body.querySelector<HTMLElement>('div[style*="position: relative"], div[style*="position:relative"]') ??
    parsed.body.firstElementChild;

  if (!(container instanceof HTMLElement)) {
    return { doc: { v: 1, canvas: { ...CANVAS_DEFAULT }, elements: [], timelineDuration: 6 }, notes: ['Aucun contenu exploitable trouvé dans ce HTML.'] };
  }

  const canvas = { ...CANVAS_DEFAULT };
  canvas.width = px(container.style.width) ?? canvas.width;
  canvas.height = px(container.style.height) ?? canvas.height;
  canvas.radius = px(container.style.borderRadius) ?? canvas.radius;
  const bg = firstColor(container.style.background || container.style.backgroundColor);
  if (bg) canvas.bg = bg;
  if ((container.style.background || '').includes('gradient')) {
    notes.push('Le fond était un dégradé : seule sa première couleur est reprise.');
  }

  // Les enfants positionnés en absolu SONT les éléments. Un HTML sans positionnement
  // absolu (une signature en table, par exemple) n'a pas de coordonnées à récupérer :
  // on le dit plutôt que d'inventer une mise en page.
  const nodes = [...container.children].filter(
    (n): n is HTMLElement => n instanceof HTMLElement && n.style.position === 'absolute',
  );
  if (nodes.length === 0) {
    return {
      doc: { v: 1, canvas, elements: [], timelineDuration: 6 },
      notes: [
        "Ce HTML n'utilise pas de positionnement absolu — c'est probablement une signature en tableau. Ses positions ne peuvent pas être déduites, il faudrait la reconstruire élément par élément.",
      ],
    };
  }

  const taken = new Set<string>();
  const elements: Element[] = [];
  let mediaManquant = 0;

  // L'ordre du tableau EST le z-index (contrat §3) : on trie dessus, à défaut sur l'ordre
  // du document.
  const ordered = nodes
    .map((n, i) => ({ n, z: Number.parseInt(n.style.zIndex || '', 10) || i + 1 }))
    .sort((a, b) => a.z - b.z)
    .map((x) => x.n);

  for (const node of ordered) {
    const s = node.style;
    const text = (node.textContent ?? '').trim();
    const background = firstColor(s.background || s.backgroundColor);
    const type = typeOf(node, text, background);

    const img = node.querySelector('img, video');
    let assetId: string | null = null;
    if (img) {
      const src = img.getAttribute('src') ?? '';
      const match = assets.find((a) => src.includes(a.url) || a.url.includes(src));
      if (match) assetId = match.id;
      else mediaManquant += 1;
    }

    const href = node instanceof HTMLAnchorElement ? node.getAttribute('href') ?? '' : '';
    const seed = {
      type: (ELEMENT_TYPES as readonly string[]).includes(type) ? type : ('text' as ElementType),
      x: px(s.left) ?? 0,
      y: px(s.top) ?? 0,
      w: px(s.width) ?? 100,
      h: px(s.height) ?? 20,
      rotation: 0,
      opacity: Number.parseFloat(s.opacity || '1') || 1,
      content: type === 'image' || type === 'video' ? '' : text,
      // Un lien de suivi Siglair pointe sur /c/{slug}/{id} : le réimporter figerait la
      // signature sur l'ancien identifiant et les clics seraient comptés sur elle.
      href: /\/c\/[a-z0-9]+\//i.test(href) ? '' : href,
      assetId,
      fontSize: px(s.fontSize) ?? 13,
      fontWeight: (s.fontWeight || '500').trim(),
      color: toHex(s.color) ?? '#ffffff',
      background: background ?? 'transparent',
      radius: px(s.borderRadius) ?? 0,
      align: (s.textAlign === 'center' || s.textAlign === 'right' ? s.textAlign : 'left') as
        | 'left'
        | 'center'
        | 'right',
      locked: false,
      hidden: false,
      anim: animOf(node),
    };
    const el = makeElement(seed, taken);
    taken.add(el.id);
    elements.push(el);
  }

  if (mediaManquant > 0) {
    notes.push(
      `${mediaManquant} image${mediaManquant > 1 ? 's ne correspondent' : ' ne correspond'} à aucun de vos médias : resélectionnez-${mediaManquant > 1 ? 'les' : 'la'} dans l'inspecteur.`,
    );
  }
  if (nodes.some((n) => /\/c\/[a-z0-9]+\//i.test(n.getAttribute('href') ?? ''))) {
    notes.push(
      'Les liens de suivi de l’ancienne signature ont été retirés : ils comptaient les clics sur elle. Ressaisissez les URL de destination.',
    );
  }

  return { doc: { v: 1, canvas, elements, timelineDuration: 6 }, notes };
}
