/**
 * Campagnes datées — branchement sur `/api/campaigns` (src/routes/campaigns.rs fait foi).
 *
 * Le panneau ne garde plus rien en mémoire : une campagne créée ici existe en base, pour
 * toute l'organisation, et survit au rechargement. Le statut n'est ni stocké ni reçu — il
 * se déduit des dates à chaque affichage, sinon il ment dès que l'heure tourne.
 *
 * Les portées viennent de `limits.campaigns` (plans.rs) : `none`, `own`, `team`. Aucun
 * quota, aucun prix, aucun nom de plan n'est écrit ici : le serveur répond 402 avec un
 * message français, on l'affiche tel quel.
 */
import { useCallback, useEffect, useState } from 'react';
import { ApiError, listSignatures, updateSignature } from '../lib/api';
import { useSession } from '../lib/session';
import type { ApiErrorBody, CampaignScope, Doc } from '../lib/types';
import { initialEditorState, reducer, toLocalInput } from './state';
import type { Campaign as BannerSource } from './state';

/** `routes::campaigns::Campaign`, tel quel. Les dates sont en RFC 3339 UTC. */
export interface Campaign {
  id: string;
  org_id: string;
  name: string;
  message: string;
  cta: string;
  href: string;
  color: string;
  starts_at: string;
  ends_at: string;
  created_at: string;
  updated_at: string;
}

/** Ce que le formulaire produit. Les dates sont déjà en ISO UTC. */
export interface CampaignInput {
  name: string;
  message: string;
  href: string;
  color: string;
  starts_at: string;
  ends_at: string;
}

export type CampaignPhase = 'upcoming' | 'active' | 'ended';

/**
 * Même fenêtre que `pick_active` côté serveur : début inclus, fin exclue. Deux règles
 * différentes donneraient une pastille « active » sur une bannière que l'API ne sert plus.
 */
export function campaignPhase(campaign: Campaign, now: number): CampaignPhase {
  const start = Date.parse(campaign.starts_at);
  const end = Date.parse(campaign.ends_at);
  if (now < start) return 'upcoming';
  return now < end ? 'active' : 'ended';
}

export const PHASE_LABELS: Record<CampaignPhase, string> = {
  upcoming: 'à venir',
  active: 'active',
  ended: 'terminée',
};

/** Valeur d'un `<input type="datetime-local">` (heure locale) → ISO UTC pour l'API. */
export const inputToIso = (local: string): string => new Date(local).toISOString();

/** ISO UTC → valeur d'un `<input type="datetime-local">`, en heure locale. */
export const isoToInput = (iso: string): string => toLocalInput(new Date(iso));

/**
 * Le même refus que `clean()` côté serveur, avec les mêmes mots. Le serveur reste
 * l'autorité — ceci évite juste un aller-retour pour dire une évidence.
 */
export function windowError(startLocal: string, endLocal: string): string | null {
  const start = Date.parse(startLocal);
  const end = Date.parse(endLocal);
  if (Number.isNaN(start) || Number.isNaN(end)) return 'Renseignez une date de début et une date de fin.';
  if (end <= start) return 'La fin de la campagne doit être postérieure à son début.';
  return null;
}

/** Ce que le réducteur attend pour poser la bannière. La campagne en est la source. */
export const bannerSource = (campaign: Campaign): BannerSource => ({
  id: campaign.id,
  name: campaign.name,
  message: campaign.message,
  href: campaign.href,
  color: campaign.color,
  start: isoToInput(campaign.starts_at),
  end: isoToInput(campaign.ends_at),
});

/**
 * Insertion ou mise à jour de la bannière dans un document quelconque, via le réducteur
 * de l'éditeur. Une deuxième implémentation de la même règle finirait par diverger : la
 * signature de l'auteur aurait une bannière, celles de l'équipe une autre.
 */
const withBanner = (doc: Doc, campaign: Campaign): Doc =>
  reducer(initialEditorState(doc), { type: 'applyCampaign', campaign: bannerSource(campaign) }).doc;

/* ------------------------------------------------------------------ */
/* Appels HTTP                                                         */
/* ------------------------------------------------------------------ */

// ponytail : lib/api.ts n'expose pas son helper `api<T>` et n'est pas dans mon périmètre.
// Ces vingt lignes en sont la copie stricte (cookie de session, corps d'erreur du §5.4,
// expulsion sur 401) et ont vocation à disparaître au profit de fonctions dans lib/api.ts.
async function request<T>(path: string, init: RequestInit = {}): Promise<T> {
  const res = await fetch(path, {
    credentials: 'include',
    headers: typeof init.body === 'string' ? { 'Content-Type': 'application/json' } : undefined,
    ...init,
  });
  if (res.ok) return res.status === 204 ? (undefined as T) : ((await res.json()) as T);

  const body: unknown = await res.json().catch(() => null);
  const detail = (body as ApiErrorBody | null)?.error;
  if (res.status === 401 && !location.pathname.startsWith('/login')) {
    location.assign(`/login?next=${encodeURIComponent(location.pathname + location.search)}`);
  }
  throw new ApiError(
    detail?.code ?? 'internal',
    detail?.message ?? 'Une erreur est survenue. Réessayez dans un instant.',
    res.status,
  );
}

const body = (input: CampaignInput): string => JSON.stringify(input);

/**
 * Exactement l'`ORDER BY starts_at DESC, created_at DESC, id` du serveur : une campagne
 * créée ici doit se ranger là où elle apparaîtra au prochain chargement, pas en tête.
 * Comparaison numérique et non lexicographique — les fractions de seconde de `chrono`
 * (`…:00.5Z` contre `…:00Z`) inverseraient l'ordre d'un tri de chaînes.
 */
const bySchedule = (a: Campaign, b: Campaign): number =>
  Date.parse(b.starts_at) - Date.parse(a.starts_at) ||
  Date.parse(b.created_at) - Date.parse(a.created_at) ||
  a.id.localeCompare(b.id);

/* ------------------------------------------------------------------ */
/* Le hook                                                             */
/* ------------------------------------------------------------------ */

/** Le badge doit vieillir tout seul : un onglet ouvert une heure mentirait sinon. */
const TICK_MS = 30_000;

export interface TeamPushResult {
  updated: number;
  failed: number;
}

export interface UseCampaigns {
  /** `limits.campaigns`. `none` = panneau en démonstration. */
  scope: CampaignScope;
  /** Le serveur exige un rôle admin pour écrire (`access.require_admin`). */
  canWrite: boolean;
  items: Campaign[];
  loading: boolean;
  /** Erreur de chargement, message du serveur tel quel. */
  error: string | null;
  /** Horloge de référence des pastilles d'état, rafraîchie toute seule. */
  now: number;
  /** Nombre de signatures de l'organisation, pour le récapitulatif du plan Team. */
  teamSignatures: number | null;
  create: (input: CampaignInput) => Promise<Campaign>;
  update: (id: string, input: CampaignInput) => Promise<Campaign>;
  remove: (id: string) => Promise<void>;
  /** Pose la bannière sur toutes les signatures de l'organisation. Action de masse. */
  pushToTeam: (campaign: Campaign) => Promise<TeamPushResult>;
}

export function useCampaigns(): UseCampaigns {
  const { limits, currentOrg } = useSession();
  const scope: CampaignScope = limits?.campaigns ?? 'none';
  const canWrite = currentOrg?.role !== 'member';

  const [items, setItems] = useState<Campaign[]>([]);
  const [loading, setLoading] = useState(scope !== 'none');
  const [error, setError] = useState<string | null>(null);
  const [teamSignatures, setTeamSignatures] = useState<number | null>(null);
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    const id = window.setInterval(() => setNow(Date.now()), TICK_MS);
    return () => window.clearInterval(id);
  }, []);

  useEffect(() => {
    // Plan Free : rien à charger, le panneau montre une démonstration.
    if (scope === 'none') {
      setLoading(false);
      return;
    }
    let alive = true;
    setLoading(true);
    request<Campaign[]>('/api/campaigns')
      .then((rows) => {
        if (!alive) return;
        setItems(rows);
        setError(null);
      })
      .catch((e: unknown) => {
        if (alive) setError(e instanceof ApiError ? e.message : 'Chargement des campagnes impossible.');
      })
      .finally(() => {
        if (alive) setLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [scope]);

  // Le nombre affiché sur le bouton de déploiement : promettre « toute l'équipe » sans
  // dire combien de signatures ça touche, c'est demander une confirmation à l'aveugle.
  useEffect(() => {
    if (scope !== 'team') return;
    let alive = true;
    listSignatures()
      .then((rows) => alive && setTeamSignatures(rows.length))
      .catch(() => alive && setTeamSignatures(null));
    return () => {
      alive = false;
    };
  }, [scope]);

  const create = useCallback(async (input: CampaignInput) => {
    const created = await request<Campaign>('/api/campaigns', { method: 'POST', body: body(input) });
    setItems((list) => [...list, created].sort(bySchedule));
    return created;
  }, []);

  const update = useCallback(async (id: string, input: CampaignInput) => {
    const saved = await request<Campaign>(`/api/campaigns/${id}`, { method: 'PATCH', body: body(input) });
    setItems((list) => list.map((c) => (c.id === saved.id ? saved : c)).sort(bySchedule));
    return saved;
  }, []);

  const remove = useCallback(async (id: string) => {
    await request<void>(`/api/campaigns/${id}`, { method: 'DELETE' });
    setItems((list) => list.filter((c) => c.id !== id));
  }, []);

  const pushToTeam = useCallback(async (campaign: Campaign): Promise<TeamPushResult> => {
    const signatures = await listSignatures();
    setTeamSignatures(signatures.length);
    // ponytail : un PATCH par signature. Une organisation en a quelques dizaines ; au-delà,
    // c'est une route serveur `POST /api/campaigns/{id}/rollout` qu'il faudra, pas une
    // boucle plus maligne ici.
    const results = await Promise.allSettled(
      signatures.map((sig) => updateSignature(sig.id, { doc: withBanner(sig.doc, campaign) })),
    );
    const failed = results.filter((r) => r.status === 'rejected').length;
    return { updated: results.length - failed, failed };
  }, []);

  return {
    scope,
    canWrite,
    items,
    loading,
    error,
    now,
    teamSignatures,
    create,
    update,
    remove,
    pushToTeam,
  };
}
