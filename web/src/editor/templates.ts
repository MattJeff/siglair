/**
 * Modèles de départ : des compositions suffisamment distinctes pour servir de vraie direction,
 * pas de simples variations de couleur du même squelette.
 *
 * Deux écarts assumés vis-à-vis de la maquette :
 *  - les images n'embarquent plus de data-URL (contrat §3) : `assetId` reste nul et
 *    l'utilisateur choisit un média dans sa bibliothèque ;
 *  - plus d'image de fond distante (la maquette pointait une photo Unsplash) : on ne fait
 *    pas dépendre un modèle par défaut d'un tiers, et le garde SSRF revalide de toute façon.
 */
import type { Doc } from '../lib/types';
import { CANVAS_DEFAULT, makeElement } from './state';
import type { ElementSeed } from './state';

export interface Template {
  id: string;
  name: string;
  description: string;
  tag: string;
  canvas: Doc['canvas'];
  timelineDuration: number;
  elements: ElementSeed[];
}

export const TEMPLATES: Template[] = [
  {
    id: 'founder-motion',
    name: 'Founder Motion',
    description: 'Logo animé, texte révélé, trois CTA',
    tag: 'Fondateur',
    canvas: { ...CANVAS_DEFAULT, width: 620, height: 250, bg: 'linear-gradient(135deg, #07111f 0%, #102754 100%)', overlay: 0.82, radius: 18 },
    timelineDuration: 6,
    elements: [
      { type: 'image', x: 24, y: 48, w: 92, h: 92, radius: 46, anim: { preset: 'draw', duration: 1.6, iterations: '1' } },
      { type: 'divider', x: 132, y: 35, w: 1, h: 150, background: '#46556c', opacity: 0.65 },
      { type: 'text', x: 154, y: 39, w: 355, h: 31, content: '{{name}}', fontSize: 21, fontWeight: '700', anim: { preset: 'slide', duration: 0.9, delay: 0.35, iterations: '1' } },
      { type: 'text', x: 154, y: 75, w: 350, h: 22, content: '{{role}}', fontSize: 13, anim: { preset: 'reveal', duration: 0.9, delay: 0.55, iterations: '1' } },
      { type: 'text', x: 154, y: 101, w: 420, h: 20, content: '{{tagline}}', fontSize: 10, fontWeight: '400', color: '#cbd5e1', anim: { preset: 'fade', duration: 1.2, delay: 0.75, iterations: '1' } },
      { type: 'text', x: 154, y: 130, w: 390, h: 19, content: '{{email}}  ·  {{phone}}', fontSize: 10, color: '#93c5fd' },
      { type: 'button', x: 154, y: 165, w: 90, h: 32, content: 'Site web', href: '{{website}}', background: 'linear-gradient(120deg, #315cff 0%, #39d4ff 100%)', anim: { preset: 'shimmer', duration: 2.6, delay: 1.1, intensity: 0.7 } },
      { type: 'button', x: 252, y: 165, w: 94, h: 32, content: 'LinkedIn', href: '{{linkedin}}', background: '#0a66c2' },
      { type: 'button', x: 354, y: 165, w: 94, h: 32, content: 'WhatsApp', href: '{{whatsapp}}', background: '#16a34a', anim: { preset: 'pulse', duration: 2.2, delay: 1.5, intensity: 0.5 } },
      { type: 'shape', x: 24, y: 219, w: 520, h: 2, background: '#2563eb', radius: 2, opacity: 1 },
    ],
  },
  {
    id: 'neon-founder',
    name: 'Neon Founder',
    description: 'Halo discret et badge LIVE',
    tag: 'Tech',
    canvas: { ...CANVAS_DEFAULT, width: 620, height: 240, bg: 'radial-gradient(circle at center, #18215c 0%, #050814 100%)', overlay: 0, radius: 20 },
    timelineDuration: 6,
    elements: [
      { type: 'shape', x: 18, y: 18, w: 584, h: 204, background: '#0b1121', radius: 18, opacity: 0.95 },
      { type: 'image', x: 42, y: 65, w: 82, h: 82, radius: 41, anim: { preset: 'glow', duration: 2.4, intensity: 1.2 } },
      { type: 'text', x: 150, y: 50, w: 340, h: 30, content: '{{name}}', fontSize: 21, fontWeight: '800', anim: { preset: 'reveal', duration: 1, delay: 0.2, iterations: '1' } },
      { type: 'text', x: 150, y: 85, w: 330, h: 20, content: '{{role}}', fontSize: 11, fontWeight: '600', color: '#90b8ff' },
      { type: 'button', x: 150, y: 130, w: 112, h: 34, content: 'Voir le site ↗', href: '{{website}}', background: 'linear-gradient(110deg, #7c3aed 0%, #22d3ee 100%)', radius: 17, anim: { preset: 'pulse', duration: 2.3, delay: 0.9, intensity: 0.4 } },
      { type: 'badge', x: 272, y: 130, w: 74, h: 34, content: '● LIVE', color: '#c7f9e3', background: '#12382b', radius: 17, anim: { preset: 'flicker', duration: 2.8, delay: 1.2, intensity: 0.3 } },
    ],
  },
  {
    id: 'sales',
    name: 'Sales Conversion',
    description: 'Bannière de campagne et CTA animé',
    tag: 'Commercial',
    canvas: { ...CANVAS_DEFAULT, width: 620, height: 250, bg: 'linear-gradient(145deg, #0c1117 0%, #12372f 100%)', overlay: 0, radius: 10 },
    timelineDuration: 6,
    elements: [
      { type: 'text', x: 26, y: 25, w: 350, h: 28, content: '{{name}}', fontSize: 20, fontWeight: '700' },
      { type: 'text', x: 26, y: 58, w: 350, h: 18, content: '{{role}}', fontSize: 11, color: '#8ca0b9' },
      { type: 'banner', x: 26, y: 95, w: 566, h: 68, content: '🚀 {{company}} — découvrez la nouveauté', href: '{{website}}', fontSize: 11, background: 'linear-gradient(105deg, #2563eb 0%, #14b8a6 100%)', radius: 12, anim: { preset: 'shimmer', duration: 3, delay: 0.4, intensity: 0.8 } },
      { type: 'text', x: 26, y: 185, w: 360, h: 18, content: '{{email}} · {{phone}}', fontSize: 10, color: '#cbd5e1' },
      { type: 'button', x: 465, y: 181, w: 127, h: 30, content: 'Réserver un appel →', href: 'mailto:{{email}}', color: '#06120d', background: '#4ade80', radius: 7, anim: { preset: 'bounce', duration: 2.8, delay: 1, intensity: 0.35 } },
    ],
  },
  {
    id: 'minimal',
    name: 'Minimal Motion',
    description: 'Fond clair, très sobre',
    tag: 'Minimal',
    canvas: { ...CANVAS_DEFAULT, width: 600, height: 210, bg: '#ffffff', overlay: 0, radius: 12 },
    timelineDuration: 6,
    elements: [
      { type: 'image', x: 24, y: 39, w: 72, h: 72, radius: 36, anim: { preset: 'rotate', duration: 8, intensity: 0.3 } },
      { type: 'text', x: 122, y: 32, w: 320, h: 28, content: '{{name}}', fontSize: 19, fontWeight: '700', color: '#0f172a', anim: { preset: 'slide', duration: 0.8, delay: 0.2, iterations: '1' } },
      { type: 'text', x: 122, y: 65, w: 300, h: 19, content: '{{role}}', fontSize: 11, fontWeight: '600', color: '#2563eb' },
      { type: 'text', x: 122, y: 92, w: 420, h: 18, content: '{{tagline}}', fontSize: 9, fontWeight: '400', color: '#64748b' },
      { type: 'button', x: 122, y: 128, w: 108, h: 30, content: 'Site web', href: '{{website}}', fontSize: 9, background: '#0f172a', radius: 6, anim: { preset: 'shimmer', duration: 2.6, delay: 1, intensity: 0.4 } },
    ],
  },
  {
    id: 'studio-signal',
    name: 'Studio Signal',
    description: 'Clair, éditorial et très lisible',
    tag: 'Créatif',
    canvas: {
      ...CANVAS_DEFAULT,
      width: 620,
      height: 230,
      bg: 'linear-gradient(125deg, #f8fafc 0%, #dbeafe 100%)',
      overlay: 0,
      radius: 16,
    },
    timelineDuration: 5,
    elements: [
      { type: 'shape', x: 20, y: 20, w: 10, h: 190, background: 'linear-gradient(180deg, #315cff 0%, #22d3ee 100%)', radius: 5 },
      { type: 'image', x: 54, y: 52, w: 92, h: 92, radius: 18, anim: { preset: 'float', duration: 3.6, intensity: 0.35 } },
      { type: 'text', x: 174, y: 37, w: 360, h: 34, content: '{{name}}', fontSize: 23, fontWeight: '800', color: '#0f172a', anim: { preset: 'reveal', duration: 0.8, iterations: '1' } },
      { type: 'text', x: 174, y: 73, w: 360, h: 22, content: '{{role}}', fontSize: 12, fontWeight: '600', color: '#315cff' },
      { type: 'text', x: 174, y: 104, w: 390, h: 36, content: '{{tagline}}', fontSize: 10, fontWeight: '400', color: '#475569' },
      { type: 'button', x: 174, y: 158, w: 118, h: 34, content: 'Découvrir ↗', href: '{{website}}', background: 'linear-gradient(115deg, #315cff 0%, #38bdf8 100%)', radius: 8, align: 'center', anim: { preset: 'shimmer', duration: 3, delay: 0.8, intensity: 0.45 } },
      { type: 'text', x: 310, y: 162, w: 250, h: 24, content: '{{email}}', fontSize: 10, color: '#334155' },
    ],
  },
  {
    id: 'executive-line',
    name: 'Executive Line',
    description: 'Une présence premium sans surcharge',
    tag: 'Direction',
    canvas: { ...CANVAS_DEFAULT, width: 620, height: 210, bg: '#f8fafc', overlay: 0, radius: 4 },
    timelineDuration: 5,
    elements: [
      { type: 'shape', x: 0, y: 0, w: 196, h: 210, background: 'linear-gradient(145deg, #111827 0%, #334155 100%)', radius: 0 },
      { type: 'image', x: 54, y: 45, w: 88, h: 88, radius: 44, anim: { preset: 'draw', duration: 1.4, iterations: '1' } },
      { type: 'text', x: 225, y: 36, w: 330, h: 34, content: '{{name}}', fontSize: 22, fontWeight: '700', color: '#111827', anim: { preset: 'slide', duration: 0.8, delay: 0.2, iterations: '1' } },
      { type: 'text', x: 225, y: 72, w: 330, h: 20, content: '{{role}}', fontSize: 11, fontWeight: '600', color: '#475569' },
      { type: 'divider', x: 225, y: 106, w: 320, h: 2, background: 'linear-gradient(90deg, #315cff 0%, #22d3ee 100%)' },
      { type: 'text', x: 225, y: 122, w: 330, h: 20, content: '{{email}} · {{phone}}', fontSize: 9, color: '#64748b' },
      { type: 'button', x: 225, y: 154, w: 108, h: 30, content: 'LinkedIn', href: '{{linkedin}}', color: '#ffffff', background: '#111827', radius: 3, align: 'center' },
    ],
  },
  {
    id: 'aurora-product',
    name: 'Aurora Product',
    description: 'Impact visuel pour produit ou startup',
    tag: 'Produit',
    canvas: { ...CANVAS_DEFAULT, width: 620, height: 240, bg: 'linear-gradient(135deg, #090d1d 0%, #18215c 100%)', overlay: 0, radius: 22 },
    timelineDuration: 6,
    elements: [
      { type: 'shape', x: 420, y: -68, w: 250, h: 250, background: 'conic-gradient(from 45deg at center, #7c3aed 0deg, #22d3ee 360deg)', radius: 125, opacity: 0.34, anim: { preset: 'rotate', duration: 10, intensity: 0.25 } },
      { type: 'badge', x: 30, y: 27, w: 104, h: 28, content: '✦ PRODUCT', color: '#dbeafe', background: '#24305f', radius: 14, align: 'center' },
      { type: 'text', x: 30, y: 70, w: 380, h: 38, content: '{{name}}', fontSize: 25, fontWeight: '800', anim: { preset: 'reveal', duration: 0.9, delay: 0.2, iterations: '1' } },
      { type: 'text', x: 30, y: 110, w: 400, h: 26, content: '{{role}} · {{company}}', fontSize: 12, color: '#a5b4fc' },
      { type: 'text', x: 30, y: 143, w: 420, h: 22, content: '{{tagline}}', fontSize: 10, color: '#cbd5e1' },
      { type: 'button', x: 30, y: 184, w: 136, h: 34, content: 'Voir le produit →', href: '{{website}}', background: 'linear-gradient(110deg, #7c3aed 0%, #22d3ee 100%)', radius: 17, align: 'center', anim: { preset: 'pulse', duration: 2.8, delay: 1, intensity: 0.3 } },
      { type: 'text', x: 184, y: 190, w: 220, h: 20, content: '{{email}}', fontSize: 9, color: '#e2e8f0' },
    ],
  },
  {
    id: 'compact-contact',
    name: 'Compact Contact',
    description: 'Tout l’essentiel dans un format court',
    tag: 'Compact',
    canvas: { ...CANVAS_DEFAULT, width: 620, height: 170, bg: '#ffffff', overlay: 0, radius: 10 },
    timelineDuration: 4,
    elements: [
      { type: 'image', x: 22, y: 35, w: 82, h: 82, radius: 18, anim: { preset: 'zoom', duration: 0.7, iterations: '1' } },
      { type: 'divider', x: 126, y: 27, w: 2, h: 116, background: 'linear-gradient(180deg, #315cff 0%, #22d3ee 100%)', radius: 2 },
      { type: 'text', x: 150, y: 25, w: 280, h: 30, content: '{{name}}', fontSize: 20, fontWeight: '800', color: '#0f172a' },
      { type: 'text', x: 150, y: 57, w: 300, h: 20, content: '{{role}}', fontSize: 10, fontWeight: '600', color: '#315cff' },
      { type: 'text', x: 150, y: 84, w: 300, h: 18, content: '{{email}} · {{phone}}', fontSize: 9, color: '#475569' },
      { type: 'button', x: 150, y: 115, w: 88, h: 28, content: 'Site', href: '{{website}}', fontSize: 9, background: '#0f172a', radius: 6, align: 'center' },
      { type: 'button', x: 246, y: 115, w: 96, h: 28, content: 'LinkedIn', href: '{{linkedin}}', fontSize: 9, color: '#0f172a', background: 'linear-gradient(120deg, #dbeafe 0%, #bae6fd 100%)', radius: 6, align: 'center' },
    ],
  },
];

/** Construit un document neuf : les identifiants sont régénérés à chaque application. */
export function docFromTemplate(template: Template): Doc {
  const taken = new Set<string>();
  const elements = template.elements.map((seed) => {
    const element = makeElement(seed, taken);
    taken.add(element.id);
    return element;
  });
  return { v: 1, canvas: { ...template.canvas }, elements, timelineDuration: template.timelineDuration };
}
