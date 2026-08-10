import { describe, expect, it } from 'vitest';
import type { SignatureStatus } from '../lib/types';
import { publicationPhase } from './publication';

const status = (patch: Partial<SignatureStatus>): SignatureStatus => ({
  job: null,
  render: null,
  ...patch,
});

describe('publication status', () => {
  it('does not confuse a reserved slug with a published render', () => {
    expect(publicationPhase(status({ slug: 'reserved-but-not-rendered' }))).toBe('idle');
  });

  it('resumes queued and running renders', () => {
    const job = {
      id: 'job',
      error: null,
      attempts: 1,
      created_at: '2026-08-10T00:00:00Z',
      finished_at: null,
    };
    expect(publicationPhase(status({ job: { ...job, status: 'queued' } }))).toBe('queued');
    expect(publicationPhase(status({ job: { ...job, status: 'running' } }))).toBe('rendering');
  });

  it('does not hide a failed republication behind an older render', () => {
    expect(
      publicationPhase(
        status({
          job: {
            id: 'job',
            status: 'failed',
            error: 'échec',
            attempts: 3,
            created_at: '2026-08-10T00:00:00Z',
            finished_at: '2026-08-10T00:02:00Z',
          },
          render: {
            id: 'old',
            signature_id: 'signature',
            width: 620,
            height: 250,
            frames: 72,
            fps: 12,
            bytes: 500_000,
            created_at: '2026-08-09T00:00:00Z',
          },
        }),
      ),
    ).toBe('error');
  });

  it('recognizes a published render', () => {
    expect(
      publicationPhase(
        status({
          render: {
            id: 'render',
            signature_id: 'signature',
            width: 620,
            height: 250,
            frames: 72,
            fps: 12,
            bytes: 500_000,
            created_at: '2026-08-10T00:00:00Z',
          },
        }),
      ),
    ).toBe('done');
  });
});
