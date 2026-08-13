/**
 * Réducteur unique du document de signature (contrat §3) + historique undo/redo.
 *
 * Aucune entrée/sortie ici : la page appelle l'API, le réducteur ne connaît que le `Doc`.
 * C'est la seule partie de l'éditeur où un bug est invisible à l'œil et détruit le travail
 * de l'utilisateur — d'où les tests de state.test.ts.
 */
import { ELEMENT_TYPES } from '../lib/types';
import type { ExportMode, Anim, Canvas, Doc, Element, ElementType, Profile } from '../lib/types';

/* ------------------------------------------------------------------ */
/* Identifiants                                                        */
/* ------------------------------------------------------------------ */

const ALPHABET = 'abcdefghijklmnopqrstuvwxyz0123456789';

/** `[a-z0-9]{7}`, unique dans le document (contrat §3). */
export function uid(taken: ReadonlySet<string> = new Set()): string {
  for (let attempt = 0; attempt < 64; attempt += 1) {
    const bytes = crypto.getRandomValues(new Uint8Array(7));
    let out = '';
    for (const byte of bytes) out += ALPHABET[byte % ALPHABET.length];
    if (!taken.has(out)) return out;
  }
  throw new Error('Impossible de générer un identifiant unique');
}

const idsOf = (doc: Doc): Set<string> => new Set(doc.elements.map((e) => e.id));

/**
 * Les anciennes signatures stockaient la marque Free dans le document. Elle est maintenant
 * un calque système : retirer ces libellés exacts empêche les doublons et nettoie les comptes
 * passés à Pro sans toucher aux autres textes de l'utilisateur.
 */
export function stripLegacyBranding(doc: Doc): Doc {
  const elements = doc.elements.filter((element) => {
    if (element.type !== 'text') return true;
    const content = element.content.trim().toLowerCase();
    return content !== 'power by siglair.com' && content !== 'powered by siglair.com';
  });
  return elements.length === doc.elements.length ? doc : { ...doc, elements };
}

/* ------------------------------------------------------------------ */
/* Jetons de profil (§3.1)                                             */
/* ------------------------------------------------------------------ */

const TOKEN = /\{\{(\w+)\}\}/g;

/**
 * Affichage éditeur uniquement. La résolution qui compte est faite par le serveur au rendu.
 * Un jeton non renseigné reste visible ici (`{{name}}`) : dans l'éditeur c'est une information
 * utile, alors que côté serveur il devient "" pour ne jamais fuiter dans un email.
 */
export function resolveTokens(text: string, profile: Profile): string {
  const lookup = profile as Record<string, string | undefined>;
  return text.replace(TOKEN, (whole: string, key: string) => {
    const value = lookup[key];
    return value && value.trim() !== '' ? value : whole;
  });
}

/* ------------------------------------------------------------------ */
/* Fabrique d'éléments                                                 */
/* ------------------------------------------------------------------ */

export const ANIM_DEFAULT: Readonly<Anim> = {
  preset: 'none',
  duration: 2.4,
  delay: 0,
  iterations: 'infinite',
  easing: 'ease-in-out',
  intensity: 1,
  direction: 'normal',
};

export const CANVAS_DEFAULT: Readonly<Canvas> = {
  width: 620,
  height: 250,
  bg: '#07111f',
  bgImage: '',
  overlay: 0.8,
  radius: 18,
};

const ELEMENT_BASE: Omit<Element, 'id' | 'type' | 'anim'> = {
  x: 20,
  y: 20,
  w: 160,
  h: 32,
  rotation: 0,
  opacity: 1,
  content: 'Texte',
  href: '',
  assetId: null,
  fontSize: 13,
  fontWeight: '500',
  color: '#ffffff',
  background: '#2563eb',
  radius: 0,
  align: 'left',
  locked: false,
  hidden: false,
};

/** Valeurs par défaut par type, reprises de l'éditeur de référence. */
const TYPE_DEFAULTS: Record<ElementType, Partial<Element>> = {
  text: { w: 180, h: 32, content: 'Nouveau texte', fontSize: 13, fontWeight: '600' },
  button: {
    w: 112,
    h: 34,
    content: 'Call to action',
    href: 'https://',
    fontSize: 10,
    fontWeight: '700',
    background: '#2563eb',
    radius: 8,
    align: 'center',
  },
  image: { w: 82, h: 82, content: '', radius: 14 },
  video: { w: 140, h: 80, content: '', radius: 10 },
  badge: {
    w: 76,
    h: 30,
    content: '● LIVE',
    fontSize: 9,
    fontWeight: '700',
    color: '#bff8dc',
    background: '#12382b',
    radius: 15,
    align: 'center',
  },
  shape: { w: 170, h: 75, content: '', background: '#18233a', radius: 12, opacity: 0.85 },
  divider: { w: 180, h: 2, content: '', background: '#64748b', radius: 2 },
  banner: {
    w: 420,
    h: 56,
    content: '🚀 Nouveauté — à découvrir',
    href: 'https://',
    fontSize: 10,
    fontWeight: '700',
    background: '#2563eb',
    radius: 11,
    align: 'center',
  },
};

/** Graine de template : un type et les seules propriétés qui changent. */
export type ElementSeed = Partial<Omit<Element, 'anim'>> & {
  type: ElementType;
  anim?: Partial<Anim>;
};

export function makeElement(seed: ElementSeed, taken: ReadonlySet<string>): Element {
  const { anim, ...rest } = seed;
  return {
    ...ELEMENT_BASE,
    ...TYPE_DEFAULTS[seed.type],
    ...rest,
    id: uid(taken),
    // Objet neuf à chaque fois : deux éléments qui partagent leur `anim` est un alias
    // impossible à diagnostiquer (animer l'un anime l'autre).
    anim: { ...ANIM_DEFAULT, ...anim },
  };
}

export const isElementType = (value: string): value is ElementType =>
  (ELEMENT_TYPES as readonly string[]).includes(value);

export const EMPTY_DOC: Doc = { v: 1, canvas: { ...CANVAS_DEFAULT }, elements: [], timelineDuration: 6 };

/* ------------------------------------------------------------------ */
/* Campagnes datées                                                    */
/* ------------------------------------------------------------------ */

export interface Campaign {
  id: string;
  name: string;
  message: string;
  href: string;
  color: string;
  /** `datetime-local`, heure locale. */
  start: string;
  end: string;
}

/** Valeur d'un `<input type="datetime-local">` : heure locale, pas UTC. */
export function toLocalInput(date: Date): string {
  const shifted = new Date(date.getTime() - date.getTimezoneOffset() * 60_000);
  return shifted.toISOString().slice(0, 16);
}

const day = (offset: number): Date => new Date(Date.now() + offset * 86_400_000);

/** Deux exemples datés par rapport à aujourd'hui : une démo figée en 2026 vieillit mal. */
export const defaultCampaigns = (): Campaign[] => [
  {
    id: uid(),
    name: 'Lancement produit',
    message: '🚀 Nouvelle version disponible — découvrez-la',
    href: 'https://',
    color: '#2563eb',
    start: toLocalInput(day(-2)),
    end: toLocalInput(day(12)),
  },
  {
    id: uid(),
    name: 'Prendre rendez-vous',
    message: '✦ Envie d’en parler ? Réservons 20 minutes.',
    href: 'https://',
    color: '#7c3aed',
    start: toLocalInput(day(13)),
    end: toLocalInput(day(40)),
  },
];

export const isCampaignActive = (campaign: Campaign, now: Date): boolean => {
  const start = new Date(campaign.start).getTime();
  const end = new Date(campaign.end).getTime();
  if (Number.isNaN(start) || Number.isNaN(end)) return false;
  return now.getTime() >= start && now.getTime() <= end;
};

/* ------------------------------------------------------------------ */
/* État et actions                                                     */
/* ------------------------------------------------------------------ */

export const HISTORY_LIMIT = 80;

export interface EditorState {
  doc: Doc;
  selectedId: string | null;
  /** Documents antérieurs, du plus ancien au plus récent. */
  past: Doc[];
  future: Doc[];
  /**
   * Deux actions consécutives portant la même clé n'écrivent qu'une entrée d'historique :
   * traîner un curseur d'intensité ne doit pas remplir la pile de 40 états.
   */
  coalesceKey: string | null;
}

export type ReorderTo = 'front' | 'back' | 'up' | 'down';

export type Action =
  /** Document venu du serveur : remet l'historique à zéro. */
  | { type: 'load'; doc: Doc }
  | { type: 'select'; id: string | null }
  | { type: 'add'; kind: ElementType; x?: number; y?: number; w?: number; h?: number; assetId?: string }
  | { type: 'update'; id: string; patch: Partial<Element>; coalesce?: string }
  | { type: 'updateAnim'; id: string; patch: Partial<Anim>; coalesce?: string }
  | { type: 'updateCanvas'; patch: Partial<Canvas>; coalesce?: string }
  | { type: 'setDuration'; seconds: number }
  | { type: 'remove'; id: string }
  | { type: 'duplicate'; id: string }
  | { type: 'reorder'; id: string; to: ReorderTo }
  /** Application d'un modèle : remplace le document, l'historique reste annulable. */
  | { type: 'replaceDoc'; doc: Doc }
  | { type: 'applyCampaign'; campaign: Campaign }
  | { type: 'undo' }
  | { type: 'redo' };

export const initialEditorState = (doc: Doc): EditorState => ({
  doc: stripLegacyBranding(doc),
  selectedId: null,
  past: [],
  future: [],
  coalesceKey: null,
});

/** Empile l'état précédent, sauf fusion avec l'action précédente de même clé. */
function commit(state: EditorState, doc: Doc, coalesce?: string): EditorState {
  const key = coalesce ?? null;
  const merge = key !== null && key === state.coalesceKey;
  return {
    ...state,
    doc,
    past: merge ? state.past : [...state.past, state.doc].slice(-HISTORY_LIMIT),
    future: [],
    coalesceKey: key,
  };
}

const withElements = (doc: Doc, map: (elements: Element[]) => Element[]): Doc => ({
  ...doc,
  elements: map(doc.elements),
});

const patchElement = (doc: Doc, id: string, patch: Partial<Element>): Doc =>
  withElements(doc, (elements) => elements.map((e) => (e.id === id ? { ...e, ...patch } : e)));

/** La sélection ne survit pas à la disparition de son élément (undo, suppression). */
const clampSelection = (state: EditorState): EditorState =>
  state.selectedId && !state.doc.elements.some((e) => e.id === state.selectedId)
    ? { ...state, selectedId: null }
    : state;

function reorder(elements: Element[], id: string, to: ReorderTo): Element[] {
  const from = elements.findIndex((e) => e.id === id);
  if (from < 0) return elements;
  const target =
    to === 'front' ? elements.length - 1 : to === 'back' ? 0 : to === 'up' ? from + 1 : from - 1;
  const index = Math.max(0, Math.min(elements.length - 1, target));
  if (index === from) return elements;
  const next = [...elements];
  const [moved] = next.splice(from, 1);
  next.splice(index, 0, moved);
  return next;
}

export function reducer(state: EditorState, action: Action): EditorState {
  switch (action.type) {
    case 'load':
      return initialEditorState(action.doc);

    case 'select':
      return { ...state, selectedId: action.id, coalesceKey: null };

    case 'add': {
      const element = makeElement(
        {
          type: action.kind,
          x: Math.round(action.x ?? 45),
          y: Math.round(action.y ?? 45),
          ...(action.w ? { w: Math.round(action.w) } : {}),
          ...(action.h ? { h: Math.round(action.h) } : {}),
          ...(action.assetId ? { assetId: action.assetId } : {}),
        },
        idsOf(state.doc),
      );
      const next = commit(state, withElements(state.doc, (els) => [...els, element]));
      return { ...next, selectedId: element.id };
    }

    case 'update':
      return commit(state, patchElement(state.doc, action.id, action.patch), action.coalesce);

    case 'updateAnim':
      return commit(
        state,
        withElements(state.doc, (elements) =>
          elements.map((e) => (e.id === action.id ? { ...e, anim: { ...e.anim, ...action.patch } } : e)),
        ),
        action.coalesce,
      );

    case 'updateCanvas':
      return commit(
        state,
        { ...state.doc, canvas: { ...state.doc.canvas, ...action.patch } },
        action.coalesce,
      );

    case 'setDuration':
      return commit(state, { ...state.doc, timelineDuration: action.seconds });

    case 'remove': {
      if (!state.doc.elements.some((e) => e.id === action.id)) return state;
      const next = commit(
        state,
        withElements(state.doc, (els) => els.filter((e) => e.id !== action.id)),
      );
      return clampSelection(next);
    }

    case 'duplicate': {
      const source = state.doc.elements.find((e) => e.id === action.id);
      if (!source) return state;
      // structuredClone : la copie ne doit partager AUCUN objet avec l'original,
      // `anim` en tête, sinon régler l'un règle l'autre.
      const copy: Element = {
        ...structuredClone(source),
        id: uid(idsOf(state.doc)),
        x: source.x + 14,
        y: source.y + 14,
      };
      const next = commit(state, withElements(state.doc, (els) => [...els, copy]));
      return { ...next, selectedId: copy.id };
    }

    case 'reorder':
      return commit(
        state,
        withElements(state.doc, (els) => reorder(els, action.id, action.to)),
      );

    case 'replaceDoc': {
      const next = commit(state, stripLegacyBranding(action.doc));
      return clampSelection({ ...next, selectedId: null });
    }

    case 'applyCampaign': {
      const { campaign } = action;
      const existing = state.doc.elements.find((e) => e.type === 'banner');
      if (existing) {
        const next = commit(
          state,
          patchElement(state.doc, existing.id, {
            content: campaign.message,
            href: campaign.href,
            background: campaign.color,
          }),
        );
        return { ...next, selectedId: existing.id };
      }
      const banner = makeElement(
        {
          type: 'banner',
          x: 25,
          y: Math.max(10, state.doc.canvas.height - 72),
          w: Math.max(120, state.doc.canvas.width - 50),
          h: 50,
          content: campaign.message,
          href: campaign.href,
          background: campaign.color,
          anim: { preset: 'shimmer', duration: 3, delay: 0.3, iterations: 'infinite', intensity: 0.7 },
        },
        idsOf(state.doc),
      );
      const next = commit(state, withElements(state.doc, (els) => [...els, banner]));
      return { ...next, selectedId: banner.id };
    }

    case 'undo': {
      const previous = state.past[state.past.length - 1];
      if (!previous) return state;
      return clampSelection({
        ...state,
        doc: previous,
        past: state.past.slice(0, -1),
        future: [state.doc, ...state.future],
        coalesceKey: null,
      });
    }

    case 'redo': {
      const [next, ...rest] = state.future;
      if (!next) return state;
      return clampSelection({
        ...state,
        doc: next,
        past: [...state.past, state.doc].slice(-HISTORY_LIMIT),
        future: rest,
        coalesceKey: null,
      });
    }
  }
}

/* ------------------------------------------------------------------ */
/* Sélecteurs                                                          */
/* ------------------------------------------------------------------ */

export const selectedElement = (state: EditorState): Element | null =>
  state.doc.elements.find((e) => e.id === state.selectedId) ?? null;

export const canUndo = (state: EditorState): boolean => state.past.length > 0;
export const canRedo = (state: EditorState): boolean => state.future.length > 0;

export const animatedElements = (doc: Doc): Element[] =>
  doc.elements.filter((e) => e.anim.preset !== 'none' && !e.hidden);

export const TYPE_LABELS: Record<ElementType, string> = {
  text: 'Texte',
  button: 'Bouton',
  image: 'Image / GIF',
  video: 'Vidéo',
  badge: 'Badge',
  shape: 'Bloc',
  divider: 'Ligne',
  banner: 'Bannière',
};

/** Libellé d'un élément dans les calques et la timeline. */
export function elementLabel(element: Element, profile: Profile): string {
  if (element.type === 'image') return 'Image';
  if (element.type === 'video') return 'Vidéo';
  const text = resolveTokens(element.content, profile).trim();
  return text === '' ? TYPE_LABELS[element.type] : text;
}

/* ------------------------------------------------------------------ */
/* Score de compatibilité client mail                                  */
/* ------------------------------------------------------------------ */

export interface Compatibility {
  score: number;
  level: 'good' | 'warn' | 'risky';
  label: string;
  issues: string[];
  warnings: string[];
}

/** Conseil, pas rendu : le HTML de vérité vient du serveur (contrat §2). */
export function compatibility(doc: Doc, mode: ExportMode = 'hosted'): Compatibility {
  const issues: string[] = [];
  const warnings: string[] = [];
  let score = 100;

  const animated = doc.elements.filter((e) => e.anim.preset !== 'none').length;
  const hasVideo = doc.elements.some((e) => e.type === 'video');
  const hasRotation = doc.elements.some((e) => e.rotation !== 0);
  const hasGradient = doc.elements.some((e) => e.background.includes('gradient'));

  if (mode === 'hosted') {
    // Le GIF est une image : la mise en page est identique PARTOUT, au pixel près. Il n'y
    // a donc rien à retirer pour une animation ou une rotation — c'est justement ce que
    // ce mode rend parfaitement. Les seuls vrais risques sont ailleurs.
    warnings.push(
      "Les images sont masquées par défaut pour un expéditeur inconnu : en prospection à froid, le destinataire ne verra rien. Préférez « Texte compatible ».",
    );
    warnings.push(
      "Outlook pour Windows fige le GIF sur sa première image : soignez ce qui est visible à l’instant 0.",
    );
    if (animated > 6) {
      score -= 15;
      issues.push(`${animated} éléments animés`);
      warnings.push(
        'Beaucoup d’éléments animés alourdissent le GIF. Au-delà d’environ 1 Mo, Gmail le coupe et l’image ne s’affiche pas.',
      );
    }
    if (hasVideo) {
      score -= 10;
      issues.push('vidéo');
      warnings.push('Vidéo : elle est figée sur sa première image au rendu.');
    }
  } else {
    // Mode table : on énumère ce qui SERA VISIBLEMENT DIFFÉRENT de l'éditeur. C'est la
    // seule information utile ici — un score élevé sur un rendu qui perd les animations
    // et réorganise la mise en page serait un mensonge.
    warnings.push(
      'Ce mode reconstruit la mise en page en colonnes : les positions sont approchées, pas identiques à l’éditeur.',
    );
    if (animated > 0) {
      score -= 20;
      issues.push('animations perdues');
      warnings.push(
        `${animated} élément${animated > 1 ? 's animés sont rendus fixes' : ' animé est rendu fixe'} : aucun client mail n’anime du HTML.`,
      );
    }
    if (doc.canvas.bgImage) {
      score -= 15;
      issues.push('image de fond perdue');
      warnings.push('L’image de fond n’est pas reprise : seule la couleur de fond est conservée.');
    }
    if (hasGradient) {
      score -= 8;
      issues.push('dégradés aplatis');
      warnings.push('Les dégradés sont remplacés par leur première couleur.');
    }
    if (hasRotation) {
      score -= 8;
      issues.push('rotations perdues');
    }
    if (hasVideo) {
      score -= 20;
      issues.push('vidéo absente');
      warnings.push('Vidéo : absente de ce mode. Utilisez le GIF hébergé.');
    }
    const spanning = doc.elements.filter(
      (e) => (e.type === 'divider' || e.type === 'shape') && e.h > e.w * 4 && e.h >= doc.canvas.height * 0.5,
    ).length;
    if (spanning > 0) {
      score -= 6;
      issues.push('séparateur vertical retiré');
      warnings.push(
        'Un séparateur qui traverse la carte ne s’exprime pas en table : il est retiré. Le GIF hébergé le conserve.',
      );
    }
  }

  score = Math.max(0, Math.min(100, score));
  const level = score >= 85 ? 'good' : score >= 65 ? 'warn' : 'risky';
  const label = level === 'good' ? 'Bonne' : level === 'warn' ? 'À optimiser' : 'Risquée';
  return { score, level, label, issues, warnings };
}
