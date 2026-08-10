import { lazy } from 'react';
import type { ComponentType } from 'react';

const RELOAD_MARKER = 'siglair:lazy-reload';
const SYNC_PARAM = '_siglair_sync';

export function isLazyLoadError(value: unknown): boolean {
  if (!(value instanceof Error)) return false;
  return /ChunkLoadError|Failed to fetch dynamically imported module|Importing a module script failed|error loading dynamically imported module|Load failed/i.test(
    `${value.name}: ${value.message}`,
  );
}

function recoveryPath(): string {
  const url = new URL(location.href);
  url.searchParams.delete(SYNC_PARAM);
  return `${url.pathname}${url.search}`;
}

export function applicationRecoveryUrl(href: string, nonce = Date.now()): string {
  const url = new URL(href);
  url.searchParams.set(SYNC_PARAM, String(nonce));
  return `${url.pathname}${url.search}${url.hash}`;
}

function clearRecoveryMarker() {
  try {
    sessionStorage.removeItem(RELOAD_MARKER);
  } catch {
    // Une politique navigateur peut bloquer sessionStorage. Le chargement normal continue.
  }
}

function clearRecoveryParam() {
  const url = new URL(location.href);
  if (!url.searchParams.has(SYNC_PARAM)) return;
  url.searchParams.delete(SYNC_PARAM);
  history.replaceState(history.state, '', `${url.pathname}${url.search}${url.hash}`);
}

/** Recharge le shell HTML en contournant explicitement les caches intermédiaires. */
export function reloadApplication() {
  clearRecoveryMarker();
  location.replace(applicationRecoveryUrl(location.href));
}

/**
 * Un onglet ouvert avant un déploiement peut demander un ancien chunk Vite qui n'existe plus.
 * On recharge alors une seule fois afin de récupérer le nouvel index.html et ses nouveaux hashes.
 */
export function lazyPage<T extends ComponentType>(loader: () => Promise<{ default: T }>) {
  return lazy(async () => {
    try {
      const module = await loader();
      clearRecoveryMarker();
      clearRecoveryParam();
      return module;
    } catch (error) {
      if (isLazyLoadError(error)) {
        const path = recoveryPath();
        try {
          if (sessionStorage.getItem(RELOAD_MARKER) !== path) {
            sessionStorage.setItem(RELOAD_MARKER, path);
            location.replace(applicationRecoveryUrl(location.href));
            return await new Promise<never>(() => undefined);
          }
          sessionStorage.removeItem(RELOAD_MARKER);
        } catch {
          // Sans marqueur fiable, ne pas risquer une boucle de rechargement.
        }
      }
      throw error;
    }
  });
}
