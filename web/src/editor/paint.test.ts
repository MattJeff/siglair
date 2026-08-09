import { describe, expect, it } from 'vitest';
import { parseFill, serializeFill } from './paint';

describe('editor paint contract', () => {
  it('parses a generated gradient', () => {
    expect(parseFill('linear-gradient(120deg, #112233 0%, #aabbcc 100%)')).toEqual({
      kind: 'linear',
      first: '#112233',
      second: '#aabbcc',
      angle: 120,
    });
  });

  it.each([
    ['solid', '#112233'],
    ['linear', 'linear-gradient(45deg, #112233 0%, #aabbcc 100%)'],
    ['radial', 'radial-gradient(circle at center, #112233 0%, #aabbcc 100%)'],
    ['conic', 'conic-gradient(from 45deg at center, #112233 0deg, #aabbcc 360deg)'],
  ] as const)('serializes %s with the closed server grammar', (kind, expected) => {
    expect(serializeFill({ kind, first: '#112233', second: '#aabbcc', angle: 45 })).toBe(expected);
  });
});
