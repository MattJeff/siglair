/**
 * Tableau de bord de croissance — mise en forme de l'entonnoir viral (contrat §11.4).
 *
 * L'appel réseau et la normalisation vivent dans `lib/api.ts` avec tous les autres
 * endpoints (`getGrowth`, `normalizeGrowth`) ; ici il ne reste que ce qui s'affiche.
 * Le miroir Rust est `growth::Funnel` : quatre entiers, trois taux dérivés.
 */
import type { Funnel } from '../../../lib/types';

/**
 * Les trois taux du §11.4. `null` — et non `0` — quand le dénominateur est vide :
 * « 0 % » et « pas encore de donnée » ne se disent pas pareil à quelqu'un qui décide
 * d'un budget. Miroir exact de `Funnel::click_rate` et de ses deux sœurs.
 */
export interface Rates {
  /** clic / vue. Indicatif : son dénominateur est le chiffre le moins fiable du tableau. */
  click: number | null;
  /** création / clic. */
  signup: number | null;
  /** payant / création. C'est celui-ci qui décide si le canal vaut quelque chose. */
  paid: number | null;
}

export function rates(f: Funnel): Rates {
  const div = (num: number, den: number): number | null => (den > 0 ? num / den : null);
  return {
    click: div(f.badge_clicks, f.views),
    signup: div(f.signups, f.badge_clicks),
    paid: div(f.paid, f.signups),
  };
}

const PERCENT = new Intl.NumberFormat('fr-FR', { style: 'percent', maximumFractionDigits: 1 });
const COUNT = new Intl.NumberFormat('fr-FR');

/** « 1,6 % », ou le tiret cadratin quand il n'y a rien à diviser. */
export const formatRate = (rate: number | null): string =>
  rate === null ? '—' : PERCENT.format(rate);

/** « 870 » avec l'espace insécable des milliers, comme partout ailleurs dans l'app. */
export const formatCount = (n: number): string => COUNT.format(n);

/**
 * Périodes proposées, bornées par la rétention du plan — jamais codée en dur ici (§6).
 * Duplique volontairement les cinq lignes de `pages/Analytics.tsx`, qui ne les exporte pas :
 * à remonter dans `components/app/helpers` le jour où un troisième écran en a besoin.
 */
export function periodOptions(retentionDays: number): number[] {
  const kept = [7, 30, 90, 365].filter((d) => d <= retentionDays);
  return kept.length > 0 ? kept : [retentionDays];
}
