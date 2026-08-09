/**
 * Un seul test pour la surface tarifaire partagée : /pricing, la landing et /app/billing
 * lisent tous ces trois fonctions. Si elles divergent, trois écrans annoncent trois grilles.
 */
import { describe, expect, it } from 'vitest';
import { formatBytes, formatPrice, formatRetention, planFeatures } from './helpers';
import type { Limits } from '../../lib/types';

const FREE: Limits = {
  signatures: 1,
  assets_bytes: 10 * 1024 ** 2,
  analytics_days: 0,
  hosted_gif: false,
  org_templates: false,
  branding: true,
};

const TEAM: Limits = {
  signatures: null,
  assets_bytes: 5 * 1024 ** 3,
  analytics_days: 365,
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
    expect(off).toContain('Campagnes datées et CTA republiables');
    expect(off).toHaveLength(5); // GIF, campagnes, analytics, modèle d'équipe, sans marque
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
