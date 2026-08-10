import { describe, expect, it } from 'vitest';
import { normalizeAnalytics } from './api';

describe('analytics API compatibility', () => {
  it('normalizes the legacy series/day response without crashing the page', () => {
    expect(
      normalizeAnalytics({
        totals: { opens: 3, clicks: 1 },
        series: [{ day: '2026-08-10', opens: 3, clicks: 1 }],
        top_elements: [{ element_id: 'cta', clicks: 1 }],
      }),
    ).toEqual({
      from: '2026-08-10',
      to: '2026-08-10',
      points: [{ date: '2026-08-10', opens: 3, clicks: 1 }],
      totals: { opens: 3, clicks: 1 },
      top_elements: [{ element_id: 'cta', label: '', clicks: 1 }],
    });
  });

  it('keeps the canonical points/date response intact', () => {
    const result = normalizeAnalytics({
      from: '2026-08-01',
      to: '2026-08-10',
      totals: { opens: 0, clicks: 0 },
      points: [],
      top_elements: [],
    });

    expect(result.points).toEqual([]);
    expect(result.from).toBe('2026-08-01');
    expect(result.to).toBe('2026-08-10');
  });
});
