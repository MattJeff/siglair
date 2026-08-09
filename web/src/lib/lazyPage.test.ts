import { describe, expect, it } from 'vitest';
import { isLazyLoadError } from './lazyPage';

describe('lazy page recovery', () => {
  it.each([
    'Failed to fetch dynamically imported module: https://siglair.com/assets/Settings-old.js',
    'Importing a module script failed.',
    'error loading dynamically imported module',
    'Load failed',
  ])('recognizes a stale deployment chunk: %s', (message) => {
    expect(isLazyLoadError(new TypeError(message))).toBe(true);
  });

  it('does not reload for an application error', () => {
    expect(isLazyLoadError(new Error('Le profil est invalide'))).toBe(false);
  });
});
