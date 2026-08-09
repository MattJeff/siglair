/**
 * Sauvegarde automatique vers PATCH /api/signatures/{id}, anti-rebond de 800 ms.
 *
 * Une sauvegarde silencieusement échouée sur un éditeur, c'est du travail perdu : l'échec est
 * un état affiché, réessayable, et la valeur en attente n'est jamais jetée.
 */
import { useCallback, useEffect, useRef, useState } from 'react';
import { ApiError, updateSignature } from '../lib/api';
import type { Doc } from '../lib/types';

const DEBOUNCE = 800;

export type SaveStatus = 'saved' | 'saving' | 'error';

export interface SavePatch {
  doc: Doc;
  name: string;
}

export interface Autosave {
  status: SaveStatus;
  error: string | null;
  retry: () => void;
  /** Écrit tout de suite et attend : à appeler avant un export ou une publication,
   *  qui lisent le document stocké côté serveur, pas celui de l'écran. */
  flush: () => Promise<void>;
}

export function useAutosave(id: string, patch: SavePatch): Autosave {
  const [status, setStatus] = useState<SaveStatus>('saved');
  const [error, setError] = useState<string | null>(null);

  /** Dernière valeur confirmée par le serveur. */
  const saved = useRef(patch);
  /** Valeur affichée à l'écran. */
  const pending = useRef(patch);
  const timer = useRef<number | null>(null);
  const inFlight = useRef<Promise<void> | null>(null);

  const write = useCallback(async () => {
    // Une seule écriture à la fois : deux PATCH concurrents peuvent arriver dans le désordre.
    if (inFlight.current) await inFlight.current;
    const target = pending.current;
    if (target === saved.current) return;

    setStatus('saving');
    const task = (async () => {
      try {
        await updateSignature(id, { doc: target.doc, name: target.name });
        saved.current = target;
        setError(null);
        setStatus(pending.current === saved.current ? 'saved' : 'saving');
      } catch (cause) {
        setStatus('error');
        setError(cause instanceof ApiError ? cause.message : 'Enregistrement impossible.');
      } finally {
        inFlight.current = null;
      }
    })();
    inFlight.current = task;
    await task;
  }, [id]);

  useEffect(() => {
    pending.current = patch;
    if (patch === saved.current) return undefined;
    setStatus((current) => (current === 'error' ? current : 'saving'));
    if (timer.current !== null) window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => void write(), DEBOUNCE);
    return () => {
      if (timer.current !== null) window.clearTimeout(timer.current);
    };
  }, [patch, write]);

  // Quitter l'éditeur ne doit pas jeter les 800 dernières millisecondes de travail.
  useEffect(
    () => () => {
      if (pending.current !== saved.current) void write();
    },
    [write],
  );

  useEffect(() => {
    const onUnload = (event: BeforeUnloadEvent) => {
      if (pending.current !== saved.current) event.preventDefault();
    };
    window.addEventListener('beforeunload', onUnload);
    return () => window.removeEventListener('beforeunload', onUnload);
  }, []);

  const flush = useCallback(async () => {
    if (timer.current !== null) {
      window.clearTimeout(timer.current);
      timer.current = null;
    }
    await write();
  }, [write]);

  const retry = useCallback(() => void flush(), [flush]);

  return { status, error, retry, flush };
}
