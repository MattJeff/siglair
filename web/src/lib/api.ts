/**
 * Client HTTP. Une fonction par endpoint du contrat §5, pas de client générique.
 * La session est un cookie HttpOnly : `credentials: 'include'` partout, rien en localStorage.
 */
import type {
  AnalyticsSeries,
  AiEditResult,
  AnalyzeResult,
  ApiErrorBody,
  ApiErrorCode,
  Asset,
  Brand,
  Config,
  Doc,
  Funnel,
  GenerateResult,
  GrowthReport,
  OnboardingClaimResult,
  OnboardingDraftResult,
  ExportMode,
  Me,
  Member,
  Org,
  Plan,
  Profile,
  PublishResult,
  Role,
  Signature,
  SignatureFunnel,
  SignatureKind,
  SignatureStatus,
  Subscription,
} from './types';

/** Erreur portant le code du contrat §5.4 et un message affichable en français. */
export class ApiError extends Error {
  constructor(
    readonly code: ApiErrorCode,
    message: string,
    readonly status: number,
  ) {
    super(message);
    this.name = 'ApiError';
  }
}

type Init = Omit<RequestInit, 'body'> & {
  body?: BodyInit | null;
  /** false pour /api/me : un 401 y est une réponse normale, pas une expulsion. */
  redirectOn401?: boolean;
};

function isErrorBody(value: unknown): value is ApiErrorBody {
  if (typeof value !== 'object' || value === null || !('error' in value)) return false;
  const { error } = value as { error: unknown };
  return (
    typeof error === 'object' &&
    error !== null &&
    'code' in error &&
    'message' in error &&
    typeof (error as { message: unknown }).message === 'string'
  );
}

async function api<T>(path: string, init: Init = {}): Promise<T> {
  const { redirectOn401 = true, headers, ...rest } = init;
  const res = await fetch(path, {
    credentials: 'include',
    // Content-Type seulement si on envoie du JSON : un FormData doit garder son boundary.
    headers:
      typeof rest.body === 'string'
        ? { 'Content-Type': 'application/json', ...headers }
        : headers,
    ...rest,
  });

  if (res.ok) {
    // 204 (magic link, DELETE) n'a pas de corps.
    return res.status === 204 ? (undefined as T) : ((await res.json()) as T);
  }

  const body: unknown = await res.json().catch(() => null);
  const err = isErrorBody(body)
    ? new ApiError(body.error.code, body.error.message, res.status)
    : new ApiError('internal', 'Une erreur est survenue. Réessayez dans un instant.', res.status);

  if (res.status === 401 && redirectOn401 && !location.pathname.startsWith('/login')) {
    location.assign(`/login?next=${encodeURIComponent(location.pathname + location.search)}`);
  }
  throw err;
}

const json = (body: unknown): string => JSON.stringify(body);

/* ---------------- Auth et bootstrap (§5.2, §9) ---------------- */

/** null = déconnecté. Le seul endpoint où un 401 n'est pas une erreur. */
export async function getMe(): Promise<Me | null> {
  try {
    return await api<Me>('/api/me', { redirectOn401: false });
  } catch (e) {
    if (e instanceof ApiError && e.status === 401) return null;
    throw e;
  }
}

export const getConfig = (): Promise<Config> => api<Config>('/api/config', { redirectOn401: false });

/** Réponse toujours 204, même email inconnu — ne rien déduire du retour. */
export const requestMagicLink = (
  email: string,
  handoff?: string | null,
  /** Code de parrainage (§11.3) : il traverse la connexion dans l'URL, jamais un cookie. */
  ref?: string | null,
): Promise<void> =>
  api<void>('/api/auth/magic/request', {
    method: 'POST',
    body: json({ email, handoff: handoff || undefined, ref: ref || undefined }),
    redirectOn401: false,
  });

export const logout = (): Promise<void> => api<void>('/api/auth/logout', { method: 'POST' });

/** Hors contrat §5.3, exigé par l'écran Réglages (RGPD). Signalé dans le rapport. */
export const deleteAccount = (): Promise<void> => api<void>('/api/me', { method: 'DELETE' });

/** Enregistre le profil membre et toutes ses signatures dans une transaction serveur. */
export const updateOwnProfile = (
  profile: Profile,
): Promise<{ updated_signatures: number }> =>
  api<{ updated_signatures: number }>('/api/me/profile', {
    method: 'PATCH',
    body: json({ profile }),
  });

/** URL de départ OAuth — navigation pleine page, pas de fetch (302 vers le fournisseur). */
export const oauthStartUrl = (provider: 'google' | 'apple'): string =>
  `/api/auth/${provider}/start`;

/* ---------------- Signatures (§5.3) ---------------- */

export const listSignatures = (): Promise<Signature[]> => api<Signature[]>('/api/signatures');

export const getSignature = (id: string): Promise<Signature> =>
  api<Signature>(`/api/signatures/${id}`);

export const createSignature = (input: {
  name: string;
  doc?: Doc;
  kind?: SignatureKind;
}): Promise<Signature> =>
  api<Signature>('/api/signatures', { method: 'POST', body: json(input) });

export const updateSignature = (
  id: string,
  patch: { name?: string; doc?: Doc; profile?: Profile },
): Promise<Signature> =>
  api<Signature>(`/api/signatures/${id}`, { method: 'PATCH', body: json(patch) });

export const editSignatureWithAi = (
  id: string,
  message: string,
  selectedId: string | null,
): Promise<AiEditResult> =>
  api<AiEditResult>(`/api/signatures/${id}/ai`, {
    method: 'POST',
    body: json({ message, selected_id: selectedId }),
  });

export const deleteSignature = (id: string): Promise<void> =>
  api<void>(`/api/signatures/${id}`, { method: 'DELETE' });

export const duplicateSignature = (id: string): Promise<Signature> =>
  api<Signature>(`/api/signatures/${id}/duplicate`, { method: 'POST' });

export const publishSignature = (id: string): Promise<PublishResult> =>
  api<PublishResult>(`/api/signatures/${id}/publish`, { method: 'POST' });

/** À interroger en boucle après un publish jusqu'à job.status done|failed. */
export const getSignatureStatus = (id: string): Promise<SignatureStatus> =>
  api<SignatureStatus>(`/api/signatures/${id}/status`);

export const exportSignature = (id: string, mode: ExportMode): Promise<{ html: string }> =>
  api<{ html: string }>(`/api/signatures/${id}/export?mode=${mode}`);

type AnalyticsWire = Omit<AnalyticsSeries, 'from' | 'to' | 'points' | 'top_elements'> & {
  from?: string;
  to?: string;
  points?: Array<{ date?: string; day?: string; opens: number; clicks: number }>;
  /** Ancienne forme servie avant l'alignement du contrat Analytics. */
  series?: Array<{ date?: string; day?: string; opens: number; clicks: number }>;
  top_elements?: Array<{ element_id: string; label?: string; clicks: number }>;
};

/** Accepte aussi l'ancienne réponse pendant un déploiement où web et API se croisent. */
export function normalizeAnalytics(value: AnalyticsWire): AnalyticsSeries {
  const points = (value.points ?? value.series ?? [])
    .map((point) => ({
      date: point.date ?? point.day ?? '',
      opens: point.opens,
      clicks: point.clicks,
    }))
    .filter((point) => point.date !== '');

  return {
    from: value.from ?? points[0]?.date ?? '',
    to: value.to ?? points[points.length - 1]?.date ?? '',
    points,
    totals: value.totals,
    top_elements: (value.top_elements ?? []).map((element) => ({
      ...element,
      label: element.label ?? '',
    })),
  };
}

export const getAnalytics = async (id: string): Promise<AnalyticsSeries> =>
  normalizeAnalytics(await api<AnalyticsWire>(`/api/signatures/${id}/analytics`));

/* ---------------- Croissance (§11.4) ---------------- */

/** Ce que le serveur peut envoyer. Tout est facultatif : rien ici n'est supposé présent. */
type GrowthWire = {
  days?: number;
  /** Les quatre agrégats, soit sous `funnel` (`growth::Funnel`), soit à plat. */
  funnel?: Partial<Funnel>;
  signatures?: unknown;
  by_signature?: unknown;
} & Partial<Funnel>;

const int = (v: unknown): number => (typeof v === 'number' && Number.isFinite(v) ? Math.trunc(v) : 0);
const str = (v: unknown): string => (typeof v === 'string' ? v : '');

const funnelOf = (row: Record<string, unknown>): Funnel => ({
  views: int(row.views),
  badge_clicks: int(row.badge_clicks),
  signups: int(row.signups),
  paid: int(row.paid),
});

/** Même parade que `normalizeAnalytics` : web et API se déploient séparément. */
export function normalizeGrowth(value: GrowthWire, fallbackDays: number): GrowthReport {
  const list = Array.isArray(value.signatures)
    ? value.signatures
    : Array.isArray(value.by_signature)
      ? value.by_signature
      : [];

  const signatures: SignatureFunnel[] = list
    .filter((row): row is Record<string, unknown> => typeof row === 'object' && row !== null)
    .map((row) => ({
      id: str(row.id) || str(row.signature_id),
      name: str(row.name) || 'Signature sans nom',
      installed: row.installed === true,
      ...funnelOf(row),
    }))
    .filter((row) => row.id !== '')
    // La question posée à cet écran est « laquelle rapporte des comptes », pas « laquelle
    // est la plus vue » : le tri suit les conversions, jamais les ouvertures.
    .sort((a, b) => b.signups - a.signups || b.badge_clicks - a.badge_clicks || b.views - a.views);

  return {
    days: int(value.days) || fallbackDays,
    funnel: funnelOf((value.funnel ?? value) as Record<string, unknown>),
    signatures,
  };
}

/**
 * `GET /api/growth?days=N`. Une erreur remonte (§5.4) : afficher « 0 vue, 0 clic » sur une
 * panne ferait conclure à l'utilisateur que le canal viral est mort.
 */
export const getGrowth = async (days: number): Promise<GrowthReport> =>
  normalizeGrowth(await api<GrowthWire>(`/api/growth?days=${days}`), days);

/** Aperçu éditeur : le HTML est produit par le serveur, jamais reconstruit ici (§2). */
export const previewDoc = (doc: Doc, profile: Profile): Promise<{ html: string }> =>
  api<{ html: string }>('/api/preview', { method: 'POST', body: json({ doc, profile }) });

/**
 * Vignette du tableau de bord. **Jamais `/s/{slug}.png`** : cette URL-là est le pixel
 * d'ouverture (§5.1), et l'afficher dans l'application ferait compter une ouverture à
 * chaque fois qu'un client regarde ses propres signatures. Ici : session requise, aucun
 * événement enregistré.
 */
export const signatureThumbUrl = (id: string): string => `/api/signatures/${id}/thumb.png`;

/* ---------------- Onboarding « colle ton site » (§6bis.6) ---------------- */

/** Gratuit et sans compte : c'est l'accroche de la vitrine. Plafonné par IP côté serveur. */
export const analyzeBrand = (url: string): Promise<AnalyzeResult> =>
  api<AnalyzeResult>('/api/onboarding/analyze', {
    method: 'POST',
    body: json({ url }),
    redirectOn401: false,
  });

/** Génère et stocke une vraie première direction avant la création du compte. */
export const createOnboardingDraft = (brand: Brand): Promise<OnboardingDraftResult> =>
  api<OnboardingDraftResult>('/api/onboarding/draft', {
    method: 'POST',
    body: json({ brand }),
    redirectOn401: false,
  });

/** Après connexion, transforme le brouillon en signature puis ouvre directement l'éditeur. */
export const claimOnboardingDraft = (handoff: string): Promise<OnboardingClaimResult> =>
  api<OnboardingClaimResult>('/api/onboarding/claim', {
    method: 'POST',
    body: json({ handoff }),
  });

/**
 * Exige un compte. 402 `quota_exceeded` = quota de générations atteint : c'est une
 * invitation, pas une panne.
 *
 * Le profil part vers NOTRE API, jamais vers le fournisseur d'IA : le serveur ne lui
 * transmet que la marque publique et injecte les coordonnées ensuite, localement (§6bis.3).
 */
export const generateVariants = (brand: Brand, profile: Profile): Promise<GenerateResult> =>
  api<GenerateResult>('/api/onboarding/generate', {
    method: 'POST',
    body: json({ brand, profile }),
  });

/** Crée la signature à partir de la proposition retenue et renvoie son id. */
export const pickVariant = (
  variantIndex: number,
  doc: Doc,
  profile: Profile,
  name?: string,
): Promise<{ id: string; name: string }> =>
  api<{ id: string; name: string }>('/api/onboarding/pick', {
    method: 'POST',
    body: json({ variant_index: variantIndex, doc, profile, name }),
  });

/* ---------------- Assets ---------------- */

export const listAssets = (): Promise<Asset[]> => api<Asset[]>('/api/assets');

export function uploadAsset(file: File): Promise<Asset> {
  const form = new FormData();
  form.append('file', file);
  return api<Asset>('/api/assets', { method: 'POST', body: form });
}

/** 409 si l'asset est utilisé par une signature. */
export const deleteAsset = (id: string): Promise<void> =>
  api<void>(`/api/assets/${id}`, { method: 'DELETE' });

/* ---------------- Organisation et équipe ---------------- */

export const listMembers = (orgId: string): Promise<Member[]> =>
  api<Member[]>(`/api/orgs/${orgId}/members`);

export const inviteMember = (orgId: string, email: string, role: Role): Promise<void> =>
  api<void>(`/api/orgs/${orgId}/invites`, { method: 'POST', body: json({ email, role }) });

export const updateMemberProfile = (
  orgId: string,
  userId: string,
  profile: Profile,
): Promise<void> =>
  api<void>(`/api/orgs/${orgId}/members/${userId}`, {
    method: 'PATCH',
    body: json({ profile }),
  });

export const removeMember = (orgId: string, userId: string): Promise<void> =>
  api<void>(`/api/orgs/${orgId}/members/${userId}`, { method: 'DELETE' });

export const updateOrg = (
  orgId: string,
  patch: { name?: string; analytics_enabled?: boolean },
): Promise<Org> => api<Org>(`/api/orgs/${orgId}`, { method: 'PATCH', body: json(patch) });

/** Génère/republie une signature par membre à partir d'un modèle d'org (plan Team). */
export const rollout = (orgId: string, templateId: string): Promise<{ count: number }> =>
  api<{ count: number }>(`/api/orgs/${orgId}/rollout`, {
    method: 'POST',
    body: json({ template_id: templateId }),
  });

/**
 * Hors contrat §5.3 : nécessaire à /invite/:token. Le jeton part dans le CORPS, pas dans
 * le chemin — il ne se retrouve alors ni dans les journaux du proxy, ni dans le `Referer`.
 * (Le chemin `/api/invites/{token}/accept` visé au départ n'existe pas côté serveur.)
 */
export const acceptInvite = (token: string): Promise<Org> =>
  api<Org>('/api/orgs/invites/accept', {
    method: 'POST',
    body: json({ token }),
    redirectOn401: false,
  });

/* ---------------- Facturation ---------------- */

export const createCheckout = (plan: Plan, seats?: number): Promise<{ url: string }> =>
  api<{ url: string }>('/api/billing/checkout', { method: 'POST', body: json({ plan, seats }) });

export const openBillingPortal = (): Promise<{ url: string }> =>
  api<{ url: string }>('/api/billing/portal', { method: 'POST' });

export const getSubscription = (): Promise<Subscription> =>
  api<Subscription>('/api/billing/subscription');
