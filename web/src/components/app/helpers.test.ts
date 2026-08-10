/**
 * Un seul test pour la surface tarifaire partagée : /pricing, la landing et /app/billing
 * lisent tous ces trois fonctions. Si elles divergent, trois écrans annoncent trois grilles.
 */
import { describe, expect, it } from 'vitest';
import { formatBytes, formatPrice, formatRetention, planFeatures } from './helpers';
import type { Limits } from '../../lib/types';

// Le VRAI plan Free de src/plans.rs : le GIF hébergé y est inclus (avec la marque
// imposée), c'est la portée des campagnes qui verrouille. La fixture disait l'inverse
// et le test passait en décrivant un plan qui n'existe plus.
const FREE: Limits = {
  signatures: 1,
  assets_bytes: 10 * 1024 ** 2,
  analytics_days: 0,
  campaigns: 'none',
  hosted_gif: true,
  org_templates: false,
  branding: true,
};

const TEAM: Limits = {
  signatures: null,
  assets_bytes: 5 * 1024 ** 3,
  analytics_days: 365,
  campaigns: 'team',
  hosted_gif: true,
  org_templates: true,
  branding: false,
};

describe('formatage partagé', () => {
  it('écrit les prix à la française, sans centimes inutiles', () => {
    // Intl insère une espace insécable (étroite ou non) avant le symbole : on la normalise.
    const plain = (n: number): string => formatPrice(n).replace(/[\u00a0\u202f]/g, ' ');
    expect(plain(7.9)).toBe('7,90 €');
    expect(plain(79)).toBe('79 €');
  });

  it('donne une seule écriture des octets', () => {
    expect(formatBytes(10 * 1024 ** 2)).toBe('10 Mo');
    expect(formatBytes(5 * 1024 ** 3)).toBe('5 Go');
  });

  it('exprime la rétention comme la grille du contrat §6', () => {
    expect(formatRetention(0)).toBe('aucune');
    expect(formatRetention(30)).toBe('1 mois');
    expect(formatRetention(365)).toBe('12 mois');
  });
});

describe('planFeatures', () => {
  it('éteint exactement ce que les limites refusent, sans quota écrit en dur', () => {
    const off = planFeatures(FREE)
      .filter((f) => !f.on)
      .map((f) => f.label);
    expect(off).toContain('Campagnes datées sur vos signatures');
    // Free garde le GIF hébergé : restent éteintes les campagnes, les analytics, le
    // modèle d'équipe et l'export sans marque.
    expect(off).toEqual([
      'Campagnes datées sur vos signatures',
      'Ouvertures et clics',
      'Modèle d’équipe et déploiement en masse',
      'Export sans la marque Siglair',
    ]);
    expect(planFeatures(TEAM).every((f) => f.on)).toBe(true);
  });

  it('dérive ses libellés des limites servies par l’API', () => {
    expect(planFeatures(FREE)[0].label).toBe('1 signature');
    expect(planFeatures(TEAM)[0].label).toBe('Signatures illimitées');
    expect(planFeatures(TEAM).find((f) => f.label.startsWith('Ouvertures'))?.label).toContain(
      '12 mois',
    );
  });
});
