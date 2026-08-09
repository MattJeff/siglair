/**
 * Deux fonctions pures, deux risques réels : une URL mal normalisée part chercher n'importe
 * quoi côté serveur, et une porte de sortie cassée perd le visiteur au premier geste.
 */
import { describe, expect, it } from 'vitest';
import { minimalBrand, normalizeUrl } from './api';

describe('saisie d’URL du héros', () => {
  it('complète le schéma manquant', () => {
    expect(normalizeUrl('votreentreprise.com')).toBe('https://votreentreprise.com/');
    expect(normalizeUrl('  http://acme.fr/tarifs ')).toBe('http://acme.fr/tarifs');
    expect(normalizeUrl('https://www.acme.fr')).toBe('https://www.acme.fr/');
  });

  it('refuse ce qui n’est pas une adresse de site', () => {
    expect(normalizeUrl('')).toBeNull();
    expect(normalizeUrl('   ')).toBeNull();
    expect(normalizeUrl('acme')).toBeNull();
    expect(normalizeUrl('acme.')).toBeNull();
    // Le garde SSRF du serveur tranche pour de bon ; ici on ne fait que ne pas partir bredouille.
    expect(normalizeUrl('javascript:alert(1)')).toBeNull();
  });

  it('bâtit une marque de repli depuis le domaine', () => {
    expect(minimalBrand('https://www.acme-robotics.com/tarifs').name).toBe('Acme Robotics');
    expect(minimalBrand(null)).toEqual({
      site: '',
      name: '',
      tagline: '',
      logo: null,
      logo_asset_id: null,
      colors: [],
      socials: {},
      contacts: {},
      font: null,
    });
  });
});
