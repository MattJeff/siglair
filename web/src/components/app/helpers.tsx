/**
 * Petits utilitaires partagés par les écrans connectés.
 * Aucun composant ici : formatage (Intl, pas de librairie de dates) et lecture d'erreur API.
 */
import { ApiError } from '../../lib/api';
import type { Limits } from '../../lib/types';

const DATE = new Intl.DateTimeFormat('fr-FR', { day: 'numeric', month: 'long', year: 'numeric' });
const DAY = new Intl.DateTimeFormat('fr-FR', { day: '2-digit', month: '2-digit' });
const NUM1 = new Intl.NumberFormat('fr-FR', { maximumFractionDigits: 1 });

/** Date longue, ou « — » si absente/illisible. */
export function formatDate(iso: string | null): string {
  if (!iso) return '—';
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? '—' : DATE.format(d);
}

/**
 * Jour court pour les axes de graphique (JJ/MM).
 * `new Date('2026-08-09')` est interprété en UTC : sans l'heure locale, un utilisateur à
 * l'ouest de Greenwich verrait la veille sur chaque étiquette.
 */
export function formatDay(isoDay: string): string {
  const d = new Date(`${isoDay}T00:00:00`);
  return Number.isNaN(d.getTime()) ? isoDay : DAY.format(d);
}

/** « 7,90 € », « 79 € » — pas de centimes inutiles sur un prix rond. */
export function formatPrice(eur: number): string {
  return new Intl.NumberFormat('fr-FR', {
    style: 'currency',
    currency: 'EUR',
    minimumFractionDigits: Number.isInteger(eur) ? 0 : 2,
    maximumFractionDigits: 2,
  }).format(eur);
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} o`;
  const units = ['Ko', 'Mo', 'Go', 'To'];
  let value = bytes / 1024;
  let i = 0;
  while (value >= 1024 && i < units.length - 1) {
    value /= 1024;
    i += 1;
  }
  return `${NUM1.format(value)} ${units[i]}`;
}

/** Rétention analytics en français, sans coder la grille : la valeur vient de l'API. */
export function formatRetention(days: number): string {
  if (days <= 0) return 'aucune';
  if (days >= 365) return `${Math.round(days / 30.44)} mois`;
  if (days % 30 === 0) return `${days / 30} mois`;
  return `${days} jours`;
}

export interface PlanFeature {
  label: string;
  /** false = la ligne est affichée barrée / éteinte, jamais masquée. */
  on: boolean;
}

/**
 * Capacités d'un plan, dérivées des limites servies par l'API.
 * Aucun libellé n'est associé en dur à un nom de plan, aucun quota n'est écrit ici :
 * un changement de grille côté serveur se reflète sans redéployer le front.
 *
 * Une seule liste pour /pricing, la landing et /app/billing : trois listes finiraient
 * par vendre trois grilles différentes.
 */
export function planFeatures(l: Limits): PlanFeature[] {
  return [
    {
      label:
        l.signatures === null
          ? 'Signatures illimitées'
          : `${l.signatures} signature${l.signatures > 1 ? 's' : ''}`,
      on: true,
    },
    { label: 'GIF animé hébergé sur une URL stable', on: l.hosted_gif },
    // L'API des campagnes utilise exactement ce verrou : sans URL hébergée, une bannière
    // programmée ne pourrait pas changer dans les signatures déjà installées.
    { label: 'Campagnes datées et CTA republiables', on: l.hosted_gif },
    {
      label:
        l.analytics_days > 0
          ? `Ouvertures et clics sur ${formatRetention(l.analytics_days)}`
          : 'Ouvertures et clics',
      on: l.analytics_days > 0,
    },
    { label: `${formatBytes(l.assets_bytes)} d’images et de GIF`, on: true },
    { label: 'Modèle d’équipe et déploiement en masse', on: l.org_templates },
    { label: 'Export sans la marque Siglair', on: !l.branding },
  ];
}

/** Message affichable : l'API renvoie déjà du français (CONTRACT §5.4). */
export function apiMessage(e: unknown): string {
  if (e instanceof ApiError) return e.message;
  return 'Une erreur est survenue. Réessayez dans un instant.';
}

export function errorCode(e: unknown): string | null {
  return e instanceof ApiError ? e.code : null;
}

/**
 * Destination post-connexion. Le serveur redirige toujours vers /app après un magic link
 * ou un OAuth : on mémorise la page demandée ici et /app la rejoue une fois.
 */
export const PENDING_NEXT = 'siglair:next';

/** Refuse les URL absolues et les `//host` : une redirection ouverte est une faille. */
export function safeNext(raw: string | null): string {
  if (!raw || !raw.startsWith('/') || raw.startsWith('//')) return '/app';
  return raw;
}
