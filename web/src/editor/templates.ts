/**
 * Les 4 modèles de l'éditeur de référence, portés tels quels.
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
  /** Dégradé CSS de la vignette. */
  thumb: string;
  canvas: Doc['canvas'];
  timelineDuration: number;
  elements: ElementSeed[];
}

export const TEMPLATES: Template[] = [
  {
    id: 'founder-motion',
    name: 'Founder Motion',
    description: 'Logo animé, texte révélé, trois CTA',
    thumb: 'linear-gradient(135deg,#02050b,#0b2c61)',
    canvas: { ...CANVAS_DEFAULT, width: 620, height: 250, bg: '#07111f', overlay: 0.82, radius: 18 },
    timelineDuration: 6,
    elements: [
      { type: 'image', x: 24, y: 48, w: 92, h: 92, radius: 46, anim: { preset: 'draw', duration: 1.6, iterations: '1' } },
      { type: 'divider', x: 132, y: 35, w: 1, h: 150, background: '#46556c', opacity: 0.65 },
      { type: 'text', x: 154, y: 39, w: 355, h: 31, content: '{{name}}', fontSize: 21, fontWeight: '700', anim: { preset: 'slide', duration: 0.9, delay: 0.35, iterations: '1' } },
      { type: 'text', x: 154, y: 75, w: 350, h: 22, content: '{{role}}', fontSize: 13, anim: { preset: 'reveal', duration: 0.9, delay: 0.55, iterations: '1' } },
      { type: 'text', x: 154, y: 101, w: 420, h: 20, content: '{{tagline}}', fontSize: 10, fontWeight: '400', color: '#cbd5e1', anim: { preset: 'fade', duration: 1.2, delay: 0.75, iterations: '1' } },
      { type: 'text', x: 154, y: 130, w: 390, h: 19, content: '{{email}}  ·  {{phone}}', fontSize: 10, color: '#93c5fd' },
      { type: 'button', x: 154, y: 165, w: 90, h: 32, content: 'Site web', href: '{{website}}', anim: { preset: 'shimmer', duration: 2.6, delay: 1.1, intensity: 0.7 } },
      { type: 'button', x: 252, y: 165, w: 94, h: 32, content: 'LinkedIn', href: '{{linkedin}}', background: '#0a66c2' },
      { type: 'button', x: 354, y: 165, w: 94, h: 32, content: 'WhatsApp', href: '{{whatsapp}}', background: '#16a34a', anim: { preset: 'pulse', duration: 2.2, delay: 1.5, intensity: 0.5 } },
      { type: 'shape', x: 24, y: 219, w: 520, h: 2, background: '#2563eb', radius: 2, opacity: 1 },
    ],
  },
  {
    id: 'neon-founder',
    name: 'Neon Founder',
    description: 'Halo discret et badge LIVE',
    thumb: 'linear-gradient(135deg,#07090f,#1c1151,#075f86)',
    canvas: { ...CANVAS_DEFAULT, width: 620, height: 240, bg: '#050814', overlay: 0, radius: 20 },
    timelineDuration: 6,
    elements: [
      { type: 'shape', x: 18, y: 18, w: 584, h: 204, background: '#0b1121', radius: 18, opacity: 0.95 },
      { type: 'image', x: 42, y: 65, w: 82, h: 82, radius: 41, anim: { preset: 'glow', duration: 2.4, intensity: 1.2 } },
      { type: 'text', x: 150, y: 50, w: 340, h: 30, content: '{{name}}', fontSize: 21, fontWeight: '800', anim: { preset: 'reveal', duration: 1, delay: 0.2, iterations: '1' } },
      { type: 'text', x: 150, y: 85, w: 330, h: 20, content: '{{role}}', fontSize: 11, fontWeight: '600', color: '#90b8ff' },
      { type: 'button', x: 150, y: 130, w: 112, h: 34, content: 'Voir le site ↗', href: '{{website}}', background: '#315ff5', radius: 17, anim: { preset: 'pulse', duration: 2.3, delay: 0.9, intensity: 0.4 } },
      { type: 'badge', x: 272, y: 130, w: 74, h: 34, content: '● LIVE', color: '#c7f9e3', background: '#12382b', radius: 17, anim: { preset: 'flicker', duration: 2.8, delay: 1.2, intensity: 0.3 } },
    ],
  },
  {
    id: 'sales',
    name: 'Sales Conversion',
    description: 'Bannière de campagne et CTA animé',
    thumb: 'linear-gradient(135deg,#10151f,#19463e)',
    canvas: { ...CANVAS_DEFAULT, width: 620, height: 250, bg: '#0c1117', overlay: 0, radius: 10 },
    timelineDuration: 6,
    elements: [
      { type: 'text', x: 26, y: 25, w: 350, h: 28, content: '{{name}}', fontSize: 20, fontWeight: '700' },
      { type: 'text', x: 26, y: 58, w: 350, h: 18, content: '{{role}}', fontSize: 11, color: '#8ca0b9' },
      { type: 'banner', x: 26, y: 95, w: 566, h: 68, content: '🚀 {{company}} — découvrez la nouveauté', href: '{{website}}', fontSize: 11, radius: 12, anim: { preset: 'shimmer', duration: 3, delay: 0.4, intensity: 0.8 } },
      { type: 'text', x: 26, y: 185, w: 360, h: 18, content: '{{email}} · {{phone}}', fontSize: 10, color: '#cbd5e1' },
      { type: 'button', x: 465, y: 181, w: 127, h: 30, content: 'Réserver un appel →', href: 'mailto:{{email}}', color: '#06120d', background: '#4ade80', radius: 7, anim: { preset: 'bounce', duration: 2.8, delay: 1, intensity: 0.35 } },
    ],
  },
  {
    id: 'minimal',
    name: 'Minimal Motion',
    description: 'Fond clair, très sobre',
    thumb: 'linear-gradient(135deg,#f8fafc,#dae4f2)',
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
