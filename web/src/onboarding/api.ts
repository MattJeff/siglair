/**
 * Parcours « colle ton site, récupère ta signature » — contrat §6bis.6.
 *
 * Les trois appels et leurs types vivent désormais dans `lib/api.ts` et `lib/types.ts`,
 * avec le reste du contrat. Ce fichier ne garde que ce qui lui est propre : la traversée
 * de la connexion et les outils de saisie. Les réexports évitent de toucher les appelants.
 */
export type { Brand, BrandLogo, OnboardingVariant } from '../lib/types';
export { analyzeBrand, generateVariants, pickVariant } from '../lib/api';

import type { Brand, BrandLogo } from '../lib/types';

/* ------------------------------------------------- traversée de la connexion */

const BRAND_KEY = 'siglair:brand';

/** Destination du bouton « Continuer » : générer exige un compte (§6bis.6). */
export const LOGIN_THEN_ONBOARDING = `/login?next=${encodeURIComponent('/onboarding')}`;

/**
 * La marque analysée avant l'inscription survit à la connexion (même onglet, y compris après
 * la redirection pleine page d'OAuth). Sans ça le visiteur ressaisit son URL après s'être
 * inscrit — c'est-à-dire au moment exact où on lui a déjà tout demandé.
 */
export function rememberBrand(brand: Brand): void {
  try {
    sessionStorage.setItem(BRAND_KEY, JSON.stringify(brand));
  } catch {
    // Stockage refusé (navigation privée stricte) : on perd le raccourci, pas le parcours.
  }
}

export function readBrand(): Brand | null {
  try {
    const raw = sessionStorage.getItem(BRAND_KEY);
    return raw ? (JSON.parse(raw) as Brand) : null;
  } catch {
    return null;
  }
}

export function forgetBrand(): void {
  try {
    sessionStorage.removeItem(BRAND_KEY);
  } catch {
    /* rien à nettoyer */
  }
}

/* ------------------------------------------------------------------ outils */

/**
 * « votreentreprise.com » → « https://votreentreprise.com/ ». `null` si ce n'est pas une
 * adresse de site : autant le dire avant l'aller-retour réseau.
 * Le vrai garde reste `doc::check_remote_url` côté serveur — ceci n'est qu'un confort de saisie.
 */
export function normalizeUrl(raw: string): string | null {
  const value = raw.trim();
  if (!value) return null;
  try {
    const url = new URL(/^https?:\/\//i.test(value) ? value : `https://${value}`);
    return url.hostname.includes('.') && !url.hostname.endsWith('.') ? url.toString() : null;
  } catch {
    return null;
  }
}

/**
 * La porte de sortie : le site n'a pas pu être analysé, on part quand même de son nom de
 * domaine. Le composeur sait faire trois propositions présentables d'une marque vide (§6bis.5).
 */
export function minimalBrand(site: string | null): Brand {
  let name = '';
  if (site) {
    try {
      const label = new URL(site).hostname.replace(/^www\./, '').split('.')[0] ?? '';
      name = label
        .split(/[-_]+/)
        .filter(Boolean)
        .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
        .join(' ');
    } catch {
      name = '';
    }
  }
  return {
    site: site ?? '',
    name,
    tagline: '',
    logo: null,
    logo_asset_id: null,
    colors: [],
    socials: {},
    contacts: {},
    font: null,
  };
}

/** Source affichable d'un logo reçu en base64. `null` = rien à montrer. */
export function logoSrc(logo: BrandLogo | null): string | null {
  if (!logo?.bytes) return null;
  return `data:${logo.content_type || 'image/png'};base64,${logo.bytes}`;
}

/** Même plafond que `brand.rs` : au-delà, ce n'est plus un logo. */
export const MAX_LOGO_BYTES = 2 * 1024 * 1024;
const LOGO_TYPES = ['image/png', 'image/jpeg', 'image/webp', 'image/gif'];

/**
 * Logo de remplacement choisi par l'utilisateur, mis à la forme de `crate::ai::Logo`.
 * Les contrôles ci-dessous sont du confort : le serveur revalide les octets (magic bytes,
 * plafond, SVG refusé) comme pour n'importe quel upload (§8).
 */
export async function readLogoFile(file: File): Promise<BrandLogo> {
  if (file.size > MAX_LOGO_BYTES) {
    throw new Error('Ce fichier dépasse 2 Mo. Choisissez une image plus légère.');
  }
  if (!LOGO_TYPES.includes(file.type)) {
    throw new Error('Formats acceptés : PNG, JPEG, WebP ou GIF.');
  }
  const bitmap = await createImageBitmap(file).catch(() => null);
  if (!bitmap) throw new Error('Cette image n’a pas pu être lue.');
  const { width, height } = bitmap;
  bitmap.close();

  const bytes = new Uint8Array(await file.arrayBuffer());
  let binary = '';
  // Par tranches : `String.fromCharCode(...tableau)` dépasse la pile au-delà de ~100 000 octets.
  for (let i = 0; i < bytes.length; i += 0x8000) {
    binary += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  }
  return { bytes: btoa(binary), content_type: file.type, width, height, source: file.name };
}
