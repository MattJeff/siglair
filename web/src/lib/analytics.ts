/**
 * First-party product analytics for Siglair.
 *
 * Privacy contract:
 * - no advertising cookies or fingerprinting;
 * - no raw email, phone, name, prompt, submitted URL, IP or User-Agent;
 * - Do Not Track, Global Privacy Control and the local opt-out always win;
 * - attribution URLs are reduced to a path or an origin before storage.
 *
 * The module has no import-time side effects. Create a client, call `start()`, then track
 * typed events. The server remains authoritative for payments and signature clicks.
 */

export const ANALYTICS_ENDPOINT = '/api/events';
export const ANALYTICS_SDK_VERSION = 1;

export const ANALYTICS_STORAGE_KEYS = {
  visitorId: 'siglair.analytics.visitor.v1',
  sessionId: 'siglair.analytics.session.v1',
  attribution: 'siglair.analytics.attribution.v1',
  queue: 'siglair.analytics.queue.v1',
  optOut: 'siglair.analytics.opt_out',
} as const;

type IdProperties = {
  signature_id?: string;
  generation_id?: string;
};

type EntryPoint =
  | 'hero'
  | 'navigation'
  | 'pricing'
  | 'template'
  | 'editor'
  | 'settings'
  | 'upgrade_modal'
  | 'unknown';

type EmailClient = 'gmail' | 'outlook' | 'apple_mail' | 'other' | 'unknown';

export interface AnalyticsEventMap {
  // Acquisition and landing
  page_viewed: { page: string; authenticated?: boolean };
  referral_arrived: { referral_id?: string; viral_depth?: number };
  outbound_link_clicked: { placement: EntryPoint; target_host: string };
  landing_viewed: { variant?: string };
  hero_cta_clicked: { placement: EntryPoint };
  url_input_focused: { placement: EntryPoint };
  url_entered: { placement: EntryPoint; input_method?: 'paste' | 'type' | 'unknown' };
  url_submitted: { placement: EntryPoint; had_protocol: boolean };

  // URL-to-signature generation
  generation_started: IdProperties & { origin: 'landing' | 'editor'; attempt: number };
  generation_completed: IdProperties & {
    origin: 'landing' | 'editor';
    latency_ms: number;
    logo_detected: boolean;
    colors_detected: boolean;
    contacts_detected: boolean;
  };
  generation_failed: IdProperties & {
    origin: 'landing' | 'editor';
    latency_ms?: number;
    error_code: string;
    recoverable: boolean;
  };
  generation_regenerated: IdProperties & { reason: 'user_request' | 'error_recovery' };
  preview_viewed: IdProperties & { origin: 'landing' | 'editor' };
  claim_clicked: IdProperties & { placement: EntryPoint };

  // Signup and handoff
  signup_started: IdProperties & { method: 'magic_link' | 'google' | 'apple' | 'unknown' };
  signup_completed: IdProperties & { method: 'magic_link' | 'google' | 'apple' | 'unknown' };
  signup_failed: IdProperties & {
    method: 'magic_link' | 'google' | 'apple' | 'unknown';
    error_code: string;
  };
  generated_signature_claimed: IdProperties & { handoff_preserved: boolean };

  // Editor
  editor_opened: IdProperties & { source: 'dashboard' | 'claim' | 'template' | 'unknown' };
  editor_action: IdProperties & {
    action:
      | 'block_added'
      | 'block_deleted'
      | 'block_moved'
      | 'block_resized'
      | 'style_changed'
      | 'content_changed'
      | 'undo'
      | 'redo';
    element_type?: string;
  };
  template_selected: IdProperties & { template_id: string; previous_template_id?: string };
  animation_selected: IdProperties & { animation_id: string; element_type?: string };
  editor_preview_opened: IdProperties & { mode: 'desktop' | 'mobile'; client?: EmailClient };
  signature_saved: IdProperties & { edit_count?: number; duration_ms?: number };
  signature_published: IdProperties & { edit_count?: number; zero_edit: boolean };
  publish_started: IdProperties;
  publish_completed: IdProperties & { duration_ms?: number };
  publish_failed: IdProperties & { failure_code: string };
  export_started: IdProperties & { export_format: string };
  export_completed: IdProperties & { export_format: string };
  asset_uploaded: IdProperties & { asset_kind: string };
  ai_prompt_submitted: IdProperties & { intent: string; prompt_length_bucket: string };
  ai_edit_completed: IdProperties & { intent: string; latency_ms: number; changed_elements: number };
  ai_edit_failed: IdProperties & { intent: string; latency_ms?: number; error_code: string };

  // Installation and activation
  install_instructions_viewed: IdProperties & { client: EmailClient };
  install_started: IdProperties & {
    client: EmailClient;
    method: 'copy_html' | 'automatic' | 'manual';
  };
  install_step_completed: IdProperties & { client: EmailClient; step: number; total_steps: number };
  install_completed: IdProperties & {
    client: EmailClient;
    method: 'copy_html' | 'automatic' | 'manual';
    duration_ms?: number;
  };
  install_failed: IdProperties & {
    client: EmailClient;
    method: 'copy_html' | 'automatic' | 'manual';
    error_code: string;
    step?: number;
  };
  install_abandoned: IdProperties & { client: EmailClient; last_step?: number };
  test_email_started: IdProperties & { client: EmailClient };
  test_email_completed: IdProperties & { client: EmailClient; successful: boolean };
  signature_verified: IdProperties & { client: EmailClient };

  // Upgrade and Team
  upgrade_prompt_viewed: { placement: EntryPoint; trigger: string; current_plan: 'free' | 'pro' };
  upgrade_clicked: { placement: EntryPoint; target_plan: 'pro' | 'team'; trigger: string };
  checkout_started: { target_plan: 'pro' | 'team'; billing_period: 'monthly' | 'annual' };
  checkout_returned: { target_plan: 'pro' | 'team'; status: 'success' | 'cancelled' | 'unknown' };
  team_invite_sent: { role: 'member' | 'admin'; seat_count: number };
  analytics_viewed: IdProperties;
  settings_updated: { area: string };
  billing_viewed: Record<string, never>;
}

export type AnalyticsEventName = keyof AnalyticsEventMap;
export type AnalyticsProperties<Event extends AnalyticsEventName> = AnalyticsEventMap[Event];

export type AnalyticsDevice = 'mobile' | 'tablet' | 'desktop' | 'unknown';
export type AnalyticsChannel =
  | 'direct'
  | 'organic_search'
  | 'social'
  | 'referral'
  | 'paid'
  | 'viral_signature';

export interface AnalyticsAttribution {
  landing_page: string;
  referrer: string | null;
  channel: AnalyticsChannel;
  source: string;
  medium: string | null;
  utm_source: string | null;
  utm_medium: string | null;
  utm_campaign: string | null;
  utm_content: string | null;
  utm_term: string | null;
  referral_id: string | null;
}

export interface AnalyticsContext extends AnalyticsAttribution {
  page: string;
  device: AnalyticsDevice;
  language: string;
  sdk_version: number;
}

export interface AnalyticsEvent<Event extends AnalyticsEventName = AnalyticsEventName> {
  event_id: string;
  event: Event;
  occurred_at: string;
  visitor_id: string;
  session_id: string;
  context: AnalyticsContext;
  properties: AnalyticsEventMap[Event];
}

export interface AnalyticsBatch {
  events: Array<{
    event_id: string;
    name: AnalyticsEventName;
    occurred_at: string;
    visitor_id: string;
    session_id: string;
    properties: Record<string, string | number | boolean | null>;
    context: {
      page_path: string;
      referrer: string | null;
      utm_source: string | null;
      utm_medium: string | null;
      utm_campaign: string | null;
      utm_content: string | null;
      utm_term: string | null;
      language: string;
    };
  }>;
}

export interface StorageLike {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
}

type FetchResult = { ok: boolean };

export interface AnalyticsRuntime {
  location?: { href: string; pathname: string; search: string; origin: string };
  document?: {
    referrer: string;
    visibilityState?: string;
    addEventListener?(type: string, listener: () => void): void;
    removeEventListener?(type: string, listener: () => void): void;
  };
  navigator?: {
    language?: string;
    doNotTrack?: string | null;
    globalPrivacyControl?: boolean;
  };
  viewportWidth?: number;
  localStorage?: StorageLike;
  sessionStorage?: StorageLike;
  fetch?: (url: string, init: RequestInit) => Promise<FetchResult>;
  sendBeacon?: (url: string, data: BodyInit | null) => boolean;
  now(): number;
  randomId(): string;
  setTimer(callback: () => void, delay: number): unknown;
  clearTimer(timer: unknown): void;
  addPageHideListener?(listener: () => void): void;
  removePageHideListener?(listener: () => void): void;
}

export interface AnalyticsOptions {
  endpoint?: string;
  batchSize?: number;
  flushIntervalMs?: number;
  dedupeWindowMs?: number;
  maxQueueSize?: number;
  runtime?: AnalyticsRuntime;
}

export interface FlushOptions {
  preferBeacon?: boolean;
}

const DEFAULT_BATCH_SIZE = 20;
const DEFAULT_FLUSH_INTERVAL = 5_000;
const DEFAULT_DEDUPE_WINDOW = 1_000;
const DEFAULT_MAX_QUEUE_SIZE = 200;
const MAX_PROPERTY_LENGTH = 160;
const MAX_PATH_LENGTH = 256;

const EVENT_NAMES = new Set<AnalyticsEventName>([
  'page_viewed',
  'referral_arrived',
  'outbound_link_clicked',
  'landing_viewed',
  'hero_cta_clicked',
  'url_input_focused',
  'url_entered',
  'url_submitted',
  'generation_started',
  'generation_completed',
  'generation_failed',
  'generation_regenerated',
  'preview_viewed',
  'claim_clicked',
  'signup_started',
  'signup_completed',
  'signup_failed',
  'generated_signature_claimed',
  'editor_opened',
  'editor_action',
  'template_selected',
  'animation_selected',
  'editor_preview_opened',
  'signature_saved',
  'signature_published',
  'publish_started',
  'publish_completed',
  'publish_failed',
  'export_started',
  'export_completed',
  'asset_uploaded',
  'ai_prompt_submitted',
  'ai_edit_completed',
  'ai_edit_failed',
  'install_instructions_viewed',
  'install_started',
  'install_step_completed',
  'install_completed',
  'install_failed',
  'install_abandoned',
  'test_email_started',
  'test_email_completed',
  'signature_verified',
  'upgrade_prompt_viewed',
  'upgrade_clicked',
  'checkout_started',
  'checkout_returned',
  'team_invite_sent',
  'analytics_viewed',
  'settings_updated',
  'billing_viewed',
]);

const FORBIDDEN_PROPERTY =
  /(^|_)(email|e_mail|phone|telephone|first_name|last_name|full_name|address|street|postal|zip|ip|user_agent|prompt|raw_url|website_url|access_token|refresh_token|password|secret)($|_)/i;
const EMAIL_VALUE = /\b[^\s@]+@[^\s@]+\.[^\s@]+\b/i;
const PHONE_VALUE = /(?:^|[^A-Za-z0-9_])(?:\+?\d[\s().-]*){7,}(?:$|[^A-Za-z0-9_])/;

function read(storage: StorageLike | undefined, key: string): string | null {
  try {
    return storage?.getItem(key) ?? null;
  } catch {
    return null;
  }
}

function write(storage: StorageLike | undefined, key: string, value: string): void {
  try {
    storage?.setItem(key, value);
  } catch {
    // Storage can be disabled or full. Analytics must never break the product.
  }
}

function remove(storage: StorageLike | undefined, key: string): void {
  try {
    storage?.removeItem(key);
  } catch {
    // Same fail-open policy as reads and writes.
  }
}

function safeJsonParse(value: string | null): unknown {
  if (!value) return null;
  try {
    return JSON.parse(value) as unknown;
  } catch {
    return null;
  }
}

function shortIdentifier(value: unknown): value is string {
  return (
    typeof value === 'string' &&
    value.length >= 8 &&
    value.length <= 128 &&
    /^[A-Za-z0-9_-]+$/.test(value)
  );
}

function safeText(value: string): string | null {
  const trimmed = value.trim().slice(0, MAX_PROPERTY_LENGTH);
  if (!trimmed || EMAIL_VALUE.test(trimmed) || PHONE_VALUE.test(trimmed)) return null;
  return trimmed;
}

/** Drops nested data, risky keys and values that resemble direct personal data. */
export function sanitizeAnalyticsProperties(value: unknown): Record<string, string | number | boolean | null> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return {};

  const clean: Record<string, string | number | boolean | null> = {};
  for (const [key, property] of Object.entries(value)) {
    if (FORBIDDEN_PROPERTY.test(key) || Object.keys(clean).length >= 40) continue;
    if (typeof property === 'string') {
      const text = safeText(property);
      if (text !== null) clean[key] = text;
    } else if (typeof property === 'number' && Number.isFinite(property)) {
      clean[key] = property;
    } else if (typeof property === 'boolean' || property === null) {
      clean[key] = property;
    }
  }
  return clean;
}

function cleanMarketingValue(value: string | null): string | null {
  if (value === null) return null;
  const text = safeText(value.slice(0, 100));
  return text?.replace(/[^A-Za-z0-9_.:/-]+/g, '_') ?? null;
}

function safePath(pathname: string): string {
  let path = pathname.startsWith('/') ? pathname : `/${pathname}`;
  path = path
    .replace(/^\/invite\/[^/]+/, '/invite/:token')
    .replace(/^\/app\/editor\/[^/]+/, '/app/editor/:id')
    .replace(/^\/app\/analytics\/[^/]+/, '/app/analytics/:id');
  return path.slice(0, MAX_PATH_LENGTH);
}

export function analyticsPagePath(pathname: string): string {
  return safePath(pathname);
}

function referrerOrigin(referrer: string): string | null {
  if (!referrer) return null;
  try {
    const url = new URL(referrer);
    return url.origin === 'null' ? null : url.origin.slice(0, MAX_PROPERTY_LENGTH);
  } catch {
    return null;
  }
}

function channelFor(source: string, medium: string | null, referrer: string | null, referralId: string | null): AnalyticsChannel {
  if (referralId) return 'viral_signature';
  if (medium && /^(cpc|ppc|paid|display|affiliate)$/i.test(medium)) return 'paid';
  const host = referrer ? new URL(referrer).hostname.toLowerCase() : '';
  if (/google\.|bing\.|duckduckgo\.|yahoo\.|ecosia\./.test(host)) return 'organic_search';
  if (/linkedin\.|twitter\.|x\.com$|facebook\.|instagram\.|tiktok\.|youtube\./.test(host)) return 'social';
  if (source === 'direct') return 'direct';
  return 'referral';
}

function validAttribution(value: unknown): value is AnalyticsAttribution {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return false;
  const candidate = value as Partial<AnalyticsAttribution>;
  const nullableText = (item: unknown): boolean => item === null || typeof item === 'string';
  return (
    typeof candidate.landing_page === 'string' &&
    candidate.landing_page === safePath(candidate.landing_page) &&
    typeof candidate.source === 'string' &&
    safeText(candidate.source) === candidate.source &&
    candidate.channel !== undefined &&
    ['direct', 'organic_search', 'social', 'referral', 'paid', 'viral_signature'].includes(
      candidate.channel,
    ) &&
    nullableText(candidate.referrer) &&
    nullableText(candidate.medium) &&
    nullableText(candidate.utm_source) &&
    nullableText(candidate.utm_medium) &&
    nullableText(candidate.utm_campaign) &&
    nullableText(candidate.utm_content) &&
    nullableText(candidate.utm_term) &&
    nullableText(candidate.referral_id)
  );
}

/** Captures first-touch attribution without retaining arbitrary query strings. */
export function captureAttribution(runtime: AnalyticsRuntime): AnalyticsAttribution {
  const existing = safeJsonParse(read(runtime.localStorage, ANALYTICS_STORAGE_KEYS.attribution));
  if (validAttribution(existing)) return existing;

  const location = runtime.location;
  const params = new URLSearchParams(location?.search ?? '');
  const referrer = referrerOrigin(runtime.document?.referrer ?? '');
  const utmSource = cleanMarketingValue(params.get('utm_source'));
  const utmMedium = cleanMarketingValue(params.get('utm_medium'));
  const referralId = cleanMarketingValue(params.get('ref'));
  const source = utmSource ?? (referrer ? new URL(referrer).hostname : 'direct');
  const attribution: AnalyticsAttribution = {
    landing_page: safePath(location?.pathname ?? '/'),
    referrer,
    channel: channelFor(source, utmMedium, referrer, referralId),
    source,
    medium: utmMedium,
    utm_source: utmSource,
    utm_medium: utmMedium,
    utm_campaign: cleanMarketingValue(params.get('utm_campaign')),
    utm_content: cleanMarketingValue(params.get('utm_content')),
    utm_term: cleanMarketingValue(params.get('utm_term')),
    referral_id: referralId,
  };
  write(runtime.localStorage, ANALYTICS_STORAGE_KEYS.attribution, JSON.stringify(attribution));
  return attribution;
}

function detectDevice(width: number | undefined): AnalyticsDevice {
  if (width === undefined || !Number.isFinite(width)) return 'unknown';
  if (width < 768) return 'mobile';
  if (width < 1_024) return 'tablet';
  return 'desktop';
}

function normalizeLanguage(language: string | undefined): string {
  const clean = language?.trim().replace('_', '-').slice(0, 16);
  return clean && /^[a-z]{2,3}(?:-[A-Za-z]{2,4})?$/.test(clean) ? clean : 'unknown';
}

function defaultRuntime(): AnalyticsRuntime {
  const browser = typeof window !== 'undefined' ? window : undefined;
  const browserNavigator = browser?.navigator;
  let localStorage: StorageLike | undefined;
  let sessionStorage: StorageLike | undefined;
  try {
    localStorage = browser?.localStorage;
    sessionStorage = browser?.sessionStorage;
  } catch {
    // Access itself can throw in privacy-restricted contexts.
  }

  return {
    location: browser?.location,
    document: browser?.document,
    navigator: browserNavigator
      ? {
          language: browserNavigator.language,
          doNotTrack: browserNavigator.doNotTrack,
          globalPrivacyControl: Boolean(
            (browserNavigator as Navigator & { globalPrivacyControl?: boolean }).globalPrivacyControl,
          ),
        }
      : undefined,
    viewportWidth: browser?.innerWidth,
    localStorage,
    sessionStorage,
    fetch: browser?.fetch.bind(browser),
    sendBeacon: browserNavigator?.sendBeacon?.bind(browserNavigator),
    now: () => Date.now(),
    randomId: () => globalThis.crypto?.randomUUID?.() ?? randomFallback(),
    setTimer: (callback, delay) => globalThis.setTimeout(callback, delay),
    clearTimer: (timer) => globalThis.clearTimeout(timer as ReturnType<typeof setTimeout>),
    addPageHideListener: browser?.addEventListener.bind(browser, 'pagehide'),
    removePageHideListener: browser?.removeEventListener.bind(browser, 'pagehide'),
  };
}

function randomFallback(): string {
  const bytes = new Uint8Array(16);
  globalThis.crypto?.getRandomValues?.(bytes);
  if (bytes.every((byte) => byte === 0)) {
    for (let index = 0; index < bytes.length; index += 1) {
      bytes[index] = Math.floor(Math.random() * 256);
    }
  }
  // RFC 4122 v4, afin que le contrat UUID de l'API reste vrai sans randomUUID().
  bytes[6] = (bytes[6] & 0x0f) | 0x40;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;
  const hex = Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('');
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}

function privacySignalEnabled(runtime: AnalyticsRuntime): boolean {
  const dnt = runtime.navigator?.doNotTrack?.toLowerCase();
  return dnt === '1' || dnt === 'yes' || runtime.navigator?.globalPrivacyControl === true;
}

function queueEvent(value: unknown): value is AnalyticsEvent {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return false;
  const event = value as Partial<AnalyticsEvent>;
  return (
    shortIdentifier(event.event_id) &&
    typeof event.event === 'string' &&
    EVENT_NAMES.has(event.event as AnalyticsEventName) &&
    shortIdentifier(event.visitor_id) &&
    shortIdentifier(event.session_id) &&
    typeof event.occurred_at === 'string' &&
    Boolean(event.context) &&
    typeof event.context === 'object' &&
    !Array.isArray(event.context) &&
    validAttribution(event.context) &&
    Boolean(event.properties) &&
    typeof event.properties === 'object' &&
    !Array.isArray(event.properties)
  );
}

function dedupeKey(event: AnalyticsEventName, properties: Record<string, string | number | boolean | null>): string {
  const stable = Object.entries(properties).sort(([left], [right]) => left.localeCompare(right));
  return JSON.stringify([event, stable]);
}

function beaconBody(json: string): BodyInit {
  return typeof Blob === 'undefined' ? json : new Blob([json], { type: 'application/json' });
}

export class AnalyticsClient {
  private readonly endpoint: string;
  private readonly batchSize: number;
  private readonly flushIntervalMs: number;
  private readonly dedupeWindowMs: number;
  private readonly maxQueueSize: number;
  private readonly runtime: AnalyticsRuntime;
  private visitorId = '';
  private sessionId = '';
  private attribution: AnalyticsAttribution | null = null;
  private queue: AnalyticsEvent[] = [];
  private readonly recent = new Map<string, number>();
  private timer: unknown;
  private activeFlush: Promise<boolean> | null = null;
  private started = false;

  constructor(options: AnalyticsOptions = {}) {
    this.endpoint = options.endpoint ?? ANALYTICS_ENDPOINT;
    this.batchSize = Math.max(1, options.batchSize ?? DEFAULT_BATCH_SIZE);
    this.flushIntervalMs = Math.max(100, options.flushIntervalMs ?? DEFAULT_FLUSH_INTERVAL);
    this.dedupeWindowMs = Math.max(0, options.dedupeWindowMs ?? DEFAULT_DEDUPE_WINDOW);
    this.maxQueueSize = Math.max(this.batchSize, options.maxQueueSize ?? DEFAULT_MAX_QUEUE_SIZE);
    this.runtime = options.runtime ?? defaultRuntime();
    if (this.isEnabled()) this.initialize();
  }

  isEnabled(): boolean {
    return (
      Boolean(this.runtime.location) &&
      !privacySignalEnabled(this.runtime) &&
      read(this.runtime.localStorage, ANALYTICS_STORAGE_KEYS.optOut) !== '1'
    );
  }

  start(): void {
    if (this.started || !this.isEnabled()) return;
    this.started = true;
    this.initialize();
    this.runtime.document?.addEventListener?.('visibilitychange', this.onVisibilityChange);
    this.runtime.addPageHideListener?.(this.onPageHide);
    this.schedule();
  }

  stop(): void {
    if (!this.started) return;
    this.started = false;
    this.runtime.document?.removeEventListener?.('visibilitychange', this.onVisibilityChange);
    this.runtime.removePageHideListener?.(this.onPageHide);
    if (this.timer !== undefined) this.runtime.clearTimer(this.timer);
    this.timer = undefined;
  }

  setOptOut(optOut: boolean): void {
    if (optOut) {
      write(this.runtime.localStorage, ANALYTICS_STORAGE_KEYS.optOut, '1');
      this.stop();
      this.clearAnalyticsState();
      return;
    }
    remove(this.runtime.localStorage, ANALYTICS_STORAGE_KEYS.optOut);
    if (!privacySignalEnabled(this.runtime)) {
      this.initialize(true);
      this.start();
    }
  }

  track<Event extends AnalyticsEventName>(
    event: Event,
    properties: AnalyticsProperties<Event>,
  ): boolean {
    if (!this.isEnabled()) return false;
    this.initialize();
    const clean = sanitizeAnalyticsProperties(properties);
    const now = this.runtime.now();
    const key = dedupeKey(event, clean);
    const previous = this.recent.get(key);
    if (previous !== undefined && now - previous < this.dedupeWindowMs) return false;
    this.recent.set(key, now);
    for (const [recentKey, timestamp] of this.recent) {
      if (now - timestamp > this.dedupeWindowMs) this.recent.delete(recentKey);
    }

    const analyticsEvent: AnalyticsEvent = {
      event_id: this.runtime.randomId(),
      event,
      occurred_at: new Date(now).toISOString(),
      visitor_id: this.visitorId,
      session_id: this.sessionId,
      context: this.context(),
      properties: clean,
    } as AnalyticsEvent;

    this.queue.push(analyticsEvent);
    if (this.queue.length > this.maxQueueSize) {
      this.queue.splice(0, this.queue.length - this.maxQueueSize);
    }
    this.persistQueue();
    if (this.queue.length >= this.batchSize) void this.flush();
    else if (this.started) this.schedule();
    return true;
  }

  flush(options: FlushOptions = {}): Promise<boolean> {
    if (!this.isEnabled() || this.queue.length === 0) return Promise.resolve(false);
    if (this.activeFlush) return this.activeFlush;
    this.activeFlush = this.sendBatch(options).finally(() => {
      this.activeFlush = null;
      if (this.started && this.queue.length > 0) this.schedule();
    });
    return this.activeFlush;
  }

  getQueueSize(): number {
    return this.queue.length;
  }

  private initialize(forceNew = false): void {
    if (!this.isEnabled()) return;
    if (forceNew) this.clearAnalyticsState();
    if (!this.visitorId) {
      const storedVisitor = read(this.runtime.localStorage, ANALYTICS_STORAGE_KEYS.visitorId);
      this.visitorId = shortIdentifier(storedVisitor) ? storedVisitor : this.runtime.randomId();
      write(this.runtime.localStorage, ANALYTICS_STORAGE_KEYS.visitorId, this.visitorId);
    }
    if (!this.sessionId) {
      const storedSession = read(this.runtime.sessionStorage, ANALYTICS_STORAGE_KEYS.sessionId);
      this.sessionId = shortIdentifier(storedSession) ? storedSession : this.runtime.randomId();
      write(this.runtime.sessionStorage, ANALYTICS_STORAGE_KEYS.sessionId, this.sessionId);
    }
    this.attribution ??= captureAttribution(this.runtime);
    if (this.queue.length === 0) this.restoreQueue();
  }

  private context(): AnalyticsContext {
    return {
      ...(this.attribution ?? captureAttribution(this.runtime)),
      page: safePath(this.runtime.location?.pathname ?? '/'),
      device: detectDevice(this.runtime.viewportWidth),
      language: normalizeLanguage(this.runtime.navigator?.language),
      sdk_version: ANALYTICS_SDK_VERSION,
    };
  }

  private restoreQueue(): void {
    const stored = safeJsonParse(read(this.runtime.sessionStorage, ANALYTICS_STORAGE_KEYS.queue));
    if (!Array.isArray(stored)) return;
    const ids = new Set(this.queue.map((event) => event.event_id));
    for (const event of stored) {
      if (queueEvent(event) && !ids.has(event.event_id)) {
        this.queue.push({
          ...event,
          properties: sanitizeAnalyticsProperties(event.properties),
        } as AnalyticsEvent);
        ids.add(event.event_id);
      }
    }
    if (this.queue.length > this.maxQueueSize) this.queue = this.queue.slice(-this.maxQueueSize);
  }

  private persistQueue(): void {
    if (this.queue.length === 0) remove(this.runtime.sessionStorage, ANALYTICS_STORAGE_KEYS.queue);
    else write(this.runtime.sessionStorage, ANALYTICS_STORAGE_KEYS.queue, JSON.stringify(this.queue));
  }

  private schedule(): void {
    if (!this.started || this.timer !== undefined || this.queue.length === 0) return;
    this.timer = this.runtime.setTimer(() => {
      this.timer = undefined;
      void this.flush();
    }, this.flushIntervalMs);
  }

  private async sendBatch(options: FlushOptions): Promise<boolean> {
    const batch = this.queue.slice(0, this.batchSize);
    if (batch.length === 0) return false;
    const payload: AnalyticsBatch = {
      events: batch.map((event) => ({
        event_id: event.event_id,
        name: event.event,
        occurred_at: event.occurred_at,
        visitor_id: event.visitor_id,
        session_id: event.session_id,
        properties: sanitizeAnalyticsProperties(event.properties),
        context: {
          page_path: event.context.page,
          referrer: event.context.referrer,
          utm_source: event.context.utm_source,
          utm_medium: event.context.utm_medium,
          utm_campaign: event.context.utm_campaign,
          utm_content: event.context.utm_content,
          utm_term: event.context.utm_term,
          language: event.context.language,
        },
      })),
    };
    const json = JSON.stringify(payload);
    let accepted = false;

    if (options.preferBeacon && this.runtime.sendBeacon) {
      try {
        accepted = this.runtime.sendBeacon(this.endpoint, beaconBody(json));
      } catch {
        accepted = false;
      }
    }

    if (!accepted && this.runtime.fetch) {
      try {
        const response = await this.runtime.fetch(this.endpoint, {
          method: 'POST',
          headers: { 'content-type': 'application/json' },
          body: json,
          credentials: 'include',
          keepalive: true,
        });
        accepted = response.ok;
      } catch {
        accepted = false;
      }
    }

    if (accepted) {
      const sentIds = new Set(batch.map((event) => event.event_id));
      this.queue = this.queue.filter((event) => !sentIds.has(event.event_id));
      this.persistQueue();
    }
    return accepted;
  }

  private clearAnalyticsState(): void {
    this.queue = [];
    this.recent.clear();
    this.visitorId = '';
    this.sessionId = '';
    this.attribution = null;
    remove(this.runtime.localStorage, ANALYTICS_STORAGE_KEYS.visitorId);
    remove(this.runtime.localStorage, ANALYTICS_STORAGE_KEYS.attribution);
    remove(this.runtime.sessionStorage, ANALYTICS_STORAGE_KEYS.sessionId);
    remove(this.runtime.sessionStorage, ANALYTICS_STORAGE_KEYS.queue);
  }

  private readonly onVisibilityChange = (): void => {
    if (this.runtime.document?.visibilityState === 'hidden') void this.flush({ preferBeacon: true });
  };

  private readonly onPageHide = (): void => {
    void this.flush({ preferBeacon: true });
  };
}

export function createAnalytics(options: AnalyticsOptions = {}): AnalyticsClient {
  return new AnalyticsClient(options);
}

let sharedAnalytics: AnalyticsClient | null = null;

/** Singleton paresseux : importer le module ne crée aucun identifiant navigateur. */
export function getAnalytics(): AnalyticsClient {
  sharedAnalytics ??= createAnalytics();
  return sharedAnalytics;
}
