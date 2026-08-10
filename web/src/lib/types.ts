/**
 * Miroir TypeScript de docs/CONTRACT.md (§3 le document, §5 l'API).
 * Source unique de vérité côté front : aucun écran ne redéfinit ces formes.
 * Rien ici qui n'existe pas dans le contrat.
 */

/* ------------------------------------------------------------------ */
/* §3 — Document de signature                                          */
/* ------------------------------------------------------------------ */

export const ELEMENT_TYPES = [
  'text',
  'button',
  'image',
  'video',
  'badge',
  'shape',
  'divider',
  'banner',
] as const;
export type ElementType = (typeof ELEMENT_TYPES)[number];

/** §3.2 — liste fermée. Le CSS vit dans src/render/anim.css côté Rust. */
export const ANIM_PRESETS = [
  'none',
  'pulse',
  'glow',
  'float',
  'rotate',
  'bounce',
  'zoom',
  'fade',
  'reveal',
  'shimmer',
  'flicker',
  'swing',
  'slide',
  'draw',
] as const;
export type AnimPreset = (typeof ANIM_PRESETS)[number];

export const EASINGS = ['linear', 'ease', 'ease-in', 'ease-out', 'ease-in-out'] as const;
export type Easing = (typeof EASINGS)[number];

export const ANIM_DIRECTIONS = ['normal', 'reverse', 'alternate'] as const;
export type AnimDirection = (typeof ANIM_DIRECTIONS)[number];

export const ANIM_ITERATIONS = [
  'infinite',
  '1',
  '2',
  '3',
  '4',
  '5',
  '6',
  '7',
  '8',
  '9',
  '10',
] as const;
export type AnimIterations = (typeof ANIM_ITERATIONS)[number];

export const ALIGNS = ['left', 'center', 'right'] as const;
export type Align = (typeof ALIGNS)[number];

export interface Anim {
  preset: AnimPreset;
  /** secondes, 0.1..20 */
  duration: number;
  /** secondes, 0..20 */
  delay: number;
  iterations: AnimIterations;
  easing: Easing;
  /** multiplicateur d'amplitude, 0.1..2 */
  intensity: number;
  direction: AnimDirection;
}

export interface Element {
  /** [a-z0-9]{7}, unique dans le document */
  id: string;
  type: ElementType;
  /** px, repère canvas, origine haut-gauche */
  x: number;
  y: number;
  w: number;
  h: number;
  /** degrés */
  rotation: number;
  /** 0..1 */
  opacity: number;
  /** texte affiché ; supporte les jetons {{...}} */
  content: string;
  /** URL cible ; supporte les jetons ; "" = non cliquable */
  href: string;
  /** uuid d'un asset, pour type image|video */
  assetId: string | null;
  fontSize: number;
  fontWeight: string;
  color: string;
  /** couleur #rrggbb ou dégradé produit par FillControl */
  background: string;
  radius: number;
  align: Align;
  locked: boolean;
  hidden: boolean;
  anim: Anim;
}

export interface Canvas {
  width: number;
  height: number;
  /** couleur #rrggbb ou dégradé produit par FillControl */
  bg: string;
  /** URL http(s) ou "" — jamais de data: */
  bgImage: string;
  /** 0..1, voile noir au-dessus de bgImage */
  overlay: number;
  radius: number;
}

export interface Doc {
  v: 1;
  canvas: Canvas;
  /** l'ordre du tableau EST le z-index, index 0 = derrière */
  elements: Element[];
  /** secondes ; borne la durée du GIF */
  timelineDuration: number;
}

/* §3.1 — jetons de profil */
export const PROFILE_KEYS = [
  'name',
  'role',
  'email',
  'phone',
  'website',
  'linkedin',
  'whatsapp',
  'tagline',
  'company',
] as const;
export type ProfileKey = (typeof PROFILE_KEYS)[number];
/** Clé absente = jeton résolu en "" côté serveur. */
export type Profile = Partial<Record<ProfileKey, string>>;

/* ------------------------------------------------------------------ */
/* §5.4 — Erreurs                                                      */
/* ------------------------------------------------------------------ */

export const API_ERROR_CODES = [
  'unauthorized',
  'forbidden',
  'not_found',
  'validation',
  'quota_exceeded',
  'conflict',
  'rate_limited',
  'internal',
] as const;
export type ApiErrorCode = (typeof API_ERROR_CODES)[number];

/** Forme exacte du corps d'erreur. `message` est affichable tel quel. */
export interface ApiErrorBody {
  error: { code: ApiErrorCode; message: string };
}

/* ------------------------------------------------------------------ */
/* §5 — Ressources                                                     */
/* ------------------------------------------------------------------ */

export type Plan = 'free' | 'pro' | 'team';
export type Role = 'owner' | 'admin' | 'member';
export type SignatureKind = 'personal' | 'org_template';
export type AssetKind = 'image' | 'video';
export type JobStatus = 'queued' | 'running' | 'done' | 'failed';
export type ExportMode = 'hosted' | 'freeform' | 'safe';

export interface User {
  id: string;
  email: string;
  email_verified: boolean;
  name: string | null;
  avatar_url: string | null;
  created_at: string;
  last_seen_at: string | null;
}

export interface Org {
  id: string;
  name: string;
  slug: string;
  plan: Plan;
  seats: number;
  subscription_status: string | null;
  current_period_end: string | null;
  trial_ends_at: string | null;
  analytics_enabled: boolean;
  created_at: string;
  /** rôle de l'utilisateur courant dans cette org */
  role: Role;
}

export interface Member {
  user_id: string;
  org_id: string;
  role: Role;
  email: string;
  name: string | null;
  avatar_url: string | null;
  profile: Profile;
  created_at: string;
}

export interface Signature {
  id: string;
  org_id: string;
  owner_user_id: string | null;
  name: string;
  doc: Doc;
  profile: Profile;
  kind: SignatureKind;
  /** null tant que jamais publiée */
  public_slug: string | null;
  published_render_id: string | null;
  created_at: string;
  updated_at: string;
}

export interface Asset {
  id: string;
  org_id: string;
  kind: AssetKind;
  filename: string;
  content_type: string;
  bytes: number;
  width: number | null;
  height: number | null;
  /** URL publique servie par l'API */
  url: string;
  created_at: string;
}

export interface Render {
  id: string;
  signature_id: string;
  width: number;
  height: number;
  frames: number;
  fps: number;
  bytes: number;
  created_at: string;
}

/** GET /api/signatures/{id}/status */
export interface SignatureStatus {
  job: { status: JobStatus; error: string | null } | null;
  render: Render | null;
}

/** POST /api/signatures/{id}/publish */
export interface PublishResult {
  slug: string;
  job_id: string;
}

/* ------------------------------------------------------------------ */
/* §6 — Plans, quotas, usage                                           */
/* ------------------------------------------------------------------ */

/** Portée des campagnes datées — cf. plans.rs, source unique. */
export type CampaignScope = 'none' | 'own' | 'team';

export interface Limits {
  /** null = illimité */
  signatures: number | null;
  /**
   * `own` = programmer une bannière sur ses propres signatures.
   * `team` = la pousser sur celles de tous les membres d'un coup.
   */
  campaigns: CampaignScope;
  assets_bytes: number;
  /** 0 = pas d'analytics */
  analytics_days: number;
  hosted_gif: boolean;
  org_templates: boolean;
  /** true = marque Siglair imposée dans l'export */
  branding: boolean;
}

export interface Usage {
  signatures: number;
  assets_bytes: number;
  members: number;
}

/** Catalogue tarifaire servi par /api/config — jamais en dur dans le front. */
export interface PlanInfo {
  plan: Plan;
  name: string;
  price_eur_month: number;
  /** true = prix par membre (Team) */
  per_seat: boolean;
  min_seats: number;
  /** null = illimité ; Free est à vie, Pro et Team sont remis à zéro chaque mois. */
  ai_generations: number | null;
  ai_generations_monthly: boolean;
  limits: Limits;
}

/** GET /api/config — quelles intégrations sont configurées côté serveur. */
export interface Features {
  google: boolean;
  apple: boolean;
  magic: boolean;
  billing: boolean;
  /** Un vrai fournisseur de modèle est configuré ; sinon le composeur local prend le relais. */
  ai_provider: boolean;
}

export interface Config extends Features {
  plans: PlanInfo[];
  /**
   * §6bis.5 : toujours vrai. Sans clé, le repli déterministe rend quand même trois
   * propositions — le champ « collez votre site » ne se masque jamais.
   */
  ai: boolean;
  ai_provider: boolean;
}

/* ------------------------------------------------------------------ */
/* §6bis — « colle ton site, récupère ta signature »                   */
/* ------------------------------------------------------------------ */

/** `crate::ai::Logo` — les octets du logo récupéré, en base64. */
export interface BrandLogo {
  bytes: string;
  content_type: string;
  width: number;
  height: number;
  source: string;
}

/**
 * `crate::ai::Brand` — ce que l'extraction déterministe tire d'un site public (§6bis.2).
 * Aucune donnée personnelle ici, par construction : c'est ce qui rend l'appel au
 * fournisseur d'IA sans objet côté RGPD (§6bis.3).
 */
export interface Brand {
  site: string;
  name: string;
  tagline: string;
  logo: BrandLogo | null;
  logo_asset_id: string | null;
  /** `#rrggbb`, triées par saillance décroissante. Peut être vide. */
  colors: string[];
  socials: Record<string, string>;
  contacts: Record<string, string>;
  font: string | null;
}

/** `POST /api/onboarding/analyze`. `notice` = échec partiel affichable (logo introuvable). */
export interface AnalyzeResult {
  brand: Brand;
  notice: string | null;
}

/**
 * Une proposition. Le document est composé par `src/ai/compose.rs` : le front ne
 * reconstruit jamais de HTML de signature (§2, §6bis.1), il l'affiche via `/api/preview`.
 */
export interface OnboardingVariant {
  /** Index d'origine dans la réponse : c'est lui que `pick` attend. */
  index: number;
  name: string;
  rationale: string;
  doc: Doc;
}

/** `POST /api/onboarding/generate`. `source` va au journal, pas à l'écran. */
export interface GenerateResult {
  variants: OnboardingVariant[];
  source: 'model' | 'fallback';
}

/** GET /api/me */
export interface Me {
  user: User;
  orgs: Org[];
  /** id de l'org courante */
  current_org: string;
  plan: Plan;
  limits: Limits;
  usage: Usage;
}

/* ------------------------------------------------------------------ */
/* Analytics, facturation                                              */
/* ------------------------------------------------------------------ */

export interface AnalyticsPoint {
  /** jour, format YYYY-MM-DD */
  date: string;
  opens: number;
  clicks: number;
}

export interface AnalyticsElement {
  element_id: string;
  /** libellé lisible de l'élément (son content au moment du rendu) */
  label: string;
  clicks: number;
}

/** GET /api/signatures/{id}/analytics */
export interface AnalyticsSeries {
  from: string;
  to: string;
  points: AnalyticsPoint[];
  totals: { opens: number; clicks: number };
  top_elements: AnalyticsElement[];
}

/**
 * GET /api/billing/subscription — la forme RÉELLE de `src/billing/mod.rs::subscription`.
 *
 * Ce type mentait sur trois champs : `plan` est l'objet `PlanInfo` sérialisé (le serveur
 * envoie `Plan::get(...)`, pas son identifiant), `cancel_at_period_end` n'a jamais existé
 * ni dans la réponse ni dans la table `orgs`, et `has_subscription` / `min_seats`
 * manquaient. `tsc` ne type pas une réponse réseau : le désaccord ne se voyait qu'à
 * l'écran, sous la forme d'une formule « — » et d'un bandeau jamais rendu.
 *
 * `normalizeSubscription` (components/app/billing/compute.ts) reste le garde-fou côté
 * exécution ; ce type dit simplement la vérité au lecteur.
 */
export interface Subscription {
  plan: PlanInfo;
  status: string | null;
  seats: number;
  current_period_end: string | null;
  /** Fin de la période de grâce d'un impayé (migration 0005). null = aucune en cours. */
  grace_until: string | null;
  has_subscription: boolean;
  min_seats: number;
}
