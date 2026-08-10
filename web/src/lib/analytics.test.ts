import { describe, expect, it, vi } from 'vitest';
import {
  ANALYTICS_ENDPOINT,
  ANALYTICS_STORAGE_KEYS,
  analyticsPagePath,
  captureAttribution,
  createAnalytics,
  sanitizeAnalyticsProperties,
  type AnalyticsBatch,
  type AnalyticsRuntime,
  type StorageLike,
} from './analytics';

class MemoryStorage implements StorageLike {
  private readonly values = new Map<string, string>();

  getItem(key: string): string | null {
    return this.values.get(key) ?? null;
  }

  setItem(key: string, value: string): void {
    this.values.set(key, value);
  }

  removeItem(key: string): void {
    this.values.delete(key);
  }
}

function runtime(overrides: Partial<AnalyticsRuntime> = {}): AnalyticsRuntime {
  let sequence = 0;
  return {
    location: {
      href: 'https://siglair.com/?utm_source=linkedin&utm_medium=social&utm_campaign=launch&ref=sig_12345678',
      pathname: '/',
      search: '?utm_source=linkedin&utm_medium=social&utm_campaign=launch&ref=sig_12345678',
      origin: 'https://siglair.com',
    },
    document: { referrer: 'https://www.linkedin.com/feed/?secret=discarded' },
    navigator: { language: 'fr-FR', doNotTrack: '0', globalPrivacyControl: false },
    viewportWidth: 1_440,
    localStorage: new MemoryStorage(),
    sessionStorage: new MemoryStorage(),
    fetch: vi.fn(async () => ({ ok: true })),
    now: () => Date.parse('2026-08-10T12:00:00.000Z'),
    randomId: () => `anonymous-id-${++sequence}`,
    setTimer: () => 1,
    clearTimer: () => undefined,
    ...overrides,
  };
}

function bodyFrom(fetchMock: ReturnType<typeof vi.fn>): AnalyticsBatch {
  const init = fetchMock.mock.calls[0]?.[1] as RequestInit;
  return JSON.parse(String(init.body)) as AnalyticsBatch;
}

describe('attribution first-party', () => {
  it('captures first-touch UTM data while stripping referrer paths and arbitrary queries', () => {
    const localStorage = new MemoryStorage();
    const current = runtime({ localStorage });

    expect(captureAttribution(current)).toEqual({
      landing_page: '/',
      referrer: 'https://www.linkedin.com',
      channel: 'viral_signature',
      source: 'linkedin',
      medium: 'social',
      utm_source: 'linkedin',
      utm_medium: 'social',
      utm_campaign: 'launch',
      utm_content: null,
      utm_term: null,
      referral_id: 'sig_12345678',
    });

    const secondTouch = runtime({
      localStorage,
      location: {
        href: 'https://siglair.com/tarifs?utm_source=google',
        pathname: '/tarifs',
        search: '?utm_source=google',
        origin: 'https://siglair.com',
      },
    });
    expect(captureAttribution(secondTouch).utm_source).toBe('linkedin');
    expect(localStorage.getItem(ANALYTICS_STORAGE_KEYS.attribution)).not.toContain('secret');
  });
});

describe('privacy', () => {
  it('redacts dynamic route secrets before they can enter an event', () => {
    expect(analyticsPagePath('/invite/super-secret-token')).toBe('/invite/:token');
    expect(analyticsPagePath('/app/editor/96b7651d-5ac7-4ef5-a095-5d7b63d7c11c')).toBe(
      '/app/editor/:id',
    );
  });

  it('is SSR-safe and performs no work without a browser location', async () => {
    const fetchMock = vi.fn(async () => ({ ok: true }));
    const client = createAnalytics({ runtime: runtime({ location: undefined, fetch: fetchMock }) });

    expect(client.isEnabled()).toBe(false);
    expect(client.track('landing_viewed', {})).toBe(false);
    expect(await client.flush()).toBe(false);
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it.each([
    { doNotTrack: '1', globalPrivacyControl: false },
    { doNotTrack: 'yes', globalPrivacyControl: false },
    { doNotTrack: '0', globalPrivacyControl: true },
  ])('respects browser privacy signals: %o', (privacy) => {
    const localStorage = new MemoryStorage();
    const client = createAnalytics({
      runtime: runtime({ localStorage, navigator: { language: 'fr-FR', ...privacy } }),
    });

    expect(client.isEnabled()).toBe(false);
    expect(client.track('hero_cta_clicked', { placement: 'hero' })).toBe(false);
    expect(localStorage.getItem(ANALYTICS_STORAGE_KEYS.visitorId)).toBeNull();
  });

  it('purges identifiers and queued events on local opt-out', () => {
    const localStorage = new MemoryStorage();
    const sessionStorage = new MemoryStorage();
    const client = createAnalytics({ runtime: runtime({ localStorage, sessionStorage }) });
    client.track('landing_viewed', { variant: 'control' });

    client.setOptOut(true);

    expect(client.isEnabled()).toBe(false);
    expect(client.getQueueSize()).toBe(0);
    expect(localStorage.getItem(ANALYTICS_STORAGE_KEYS.optOut)).toBe('1');
    expect(localStorage.getItem(ANALYTICS_STORAGE_KEYS.visitorId)).toBeNull();
    expect(sessionStorage.getItem(ANALYTICS_STORAGE_KEYS.queue)).toBeNull();
  });

  it('replaces a storage value that is not a pseudonymous identifier', async () => {
    const localStorage = new MemoryStorage();
    localStorage.setItem(ANALYTICS_STORAGE_KEYS.visitorId, 'mathis@example.com');
    const fetchMock = vi.fn(async () => ({ ok: true }));
    const client = createAnalytics({ runtime: runtime({ localStorage, fetch: fetchMock }) });
    client.track('landing_viewed', {});
    await client.flush();

    const visitorId = bodyFrom(fetchMock).events[0]?.visitor_id;
    expect(visitorId).toMatch(/^[A-Za-z0-9_-]+$/);
    expect(visitorId).not.toContain('@');
  });

  it('drops direct PII, suspicious values and nested data defensively', () => {
    expect(
      sanitizeAnalyticsProperties({
        template_id: 'founder-motion',
        email: 'mathis@example.com',
        innocent_but_personal: 'mathis@example.com',
        phone_number: '+33 6 12 34 56 78',
        prompt: 'Build my signature',
        latency_ms: 420,
        flags: ['one', 'two'],
      }),
    ).toEqual({ template_id: 'founder-motion', latency_ms: 420 });
  });
});

describe('identity, queue and transport', () => {
  it('persists a pseudonymous visitor across sessions and rotates the tab session id', async () => {
    const localStorage = new MemoryStorage();
    const firstFetch = vi.fn(async () => ({ ok: true }));
    const first = createAnalytics({ runtime: runtime({ localStorage, fetch: firstFetch }) });
    first.track('landing_viewed', {});
    await first.flush();

    const secondFetch = vi.fn(async () => ({ ok: true }));
    const second = createAnalytics({ runtime: runtime({ localStorage, fetch: secondFetch }) });
    second.track('landing_viewed', {});
    await second.flush();

    const firstEvent = bodyFrom(firstFetch).events[0];
    const secondEvent = bodyFrom(secondFetch).events[0];
    expect(secondEvent?.visitor_id).toBe(firstEvent?.visitor_id);
    expect(secondEvent?.session_id).not.toBe(firstEvent?.session_id);
    expect(secondEvent?.context).toMatchObject({
      page_path: '/',
      language: 'fr-FR',
    });
  });

  it('deduplicates rapid repeats and batches typed events with fetch keepalive', async () => {
    const fetchMock = vi.fn(async () => ({ ok: true }));
    const current = runtime({ fetch: fetchMock });
    const client = createAnalytics({ runtime: current, batchSize: 10 });

    expect(client.track('hero_cta_clicked', { placement: 'hero' })).toBe(true);
    expect(client.track('hero_cta_clicked', { placement: 'hero' })).toBe(false);
    expect(
      client.track('generation_completed', {
        origin: 'landing',
        latency_ms: 850,
        logo_detected: true,
        colors_detected: true,
        contacts_detected: false,
      }),
    ).toBe(true);
    expect(await client.flush()).toBe(true);

    expect(fetchMock).toHaveBeenCalledOnce();
    expect(fetchMock).toHaveBeenCalledWith(
      ANALYTICS_ENDPOINT,
      expect.objectContaining({ method: 'POST', keepalive: true, credentials: 'include' }),
    );
    const body = bodyFrom(fetchMock);
    expect(body.events).toHaveLength(2);
    expect(body).not.toHaveProperty('sent_at');
    expect(body.events[0]).toHaveProperty('name', 'hero_cta_clicked');
    expect(body.events[0]).not.toHaveProperty('event');
    expect(body.events[1]?.properties).toMatchObject({ latency_ms: 850, logo_detected: true });
    expect(client.getQueueSize()).toBe(0);
  });

  it('uses sendBeacon on lifecycle flush and falls back to fetch when rejected', async () => {
    const fetchMock = vi.fn(async () => ({ ok: true }));
    const rejectedBeacon = vi.fn(() => false);
    const client = createAnalytics({
      runtime: runtime({ fetch: fetchMock, sendBeacon: rejectedBeacon }),
    });
    client.track('install_started', {
      signature_id: 'signature-12345678',
      client: 'gmail',
      method: 'copy_html',
    });

    expect(await client.flush({ preferBeacon: true })).toBe(true);
    expect(rejectedBeacon).toHaveBeenCalledWith(ANALYTICS_ENDPOINT, expect.anything());
    expect(fetchMock).toHaveBeenCalledOnce();
  });

  it('accepts a lifecycle batch through sendBeacon without a second network request', async () => {
    const fetchMock = vi.fn(async () => ({ ok: true }));
    const acceptedBeacon = vi.fn(() => true);
    const client = createAnalytics({
      runtime: runtime({ fetch: fetchMock, sendBeacon: acceptedBeacon }),
    });
    client.track('signature_published', {
      signature_id: 'signature-12345678',
      zero_edit: false,
      edit_count: 4,
    });

    expect(await client.flush({ preferBeacon: true })).toBe(true);
    expect(acceptedBeacon).toHaveBeenCalledOnce();
    expect(fetchMock).not.toHaveBeenCalled();
    expect(client.getQueueSize()).toBe(0);
  });

  it('keeps a failed batch for a later retry', async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce({ ok: false })
      .mockResolvedValueOnce({ ok: true });
    const sessionStorage = new MemoryStorage();
    const client = createAnalytics({ runtime: runtime({ fetch: fetchMock, sessionStorage }) });
    client.track('checkout_started', { target_plan: 'pro', billing_period: 'annual' });

    expect(await client.flush()).toBe(false);
    expect(client.getQueueSize()).toBe(1);
    expect(sessionStorage.getItem(ANALYTICS_STORAGE_KEYS.queue)).not.toBeNull();
    expect(await client.flush()).toBe(true);
    expect(client.getQueueSize()).toBe(0);
  });
});
