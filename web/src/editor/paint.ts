export type FillKind = 'solid' | 'linear' | 'radial' | 'conic';

export interface FillState {
  kind: FillKind;
  first: string;
  second: string;
  angle: number;
}

export function parseFill(value: string): FillState {
  const colors = value.match(/#[0-9a-f]{6}/gi) ?? [];
  const angle = Number(value.match(/(?:linear-gradient\(|from )([\d.]+)deg/)?.[1] ?? 135);
  const kind: FillKind = value.startsWith('linear-gradient(')
    ? 'linear'
    : value.startsWith('radial-gradient(')
      ? 'radial'
      : value.startsWith('conic-gradient(')
        ? 'conic'
        : 'solid';
  return {
    kind,
    first: colors[0] ?? '#315cff',
    second: colors[1] ?? colors[0] ?? '#39d4ff',
    angle: Number.isFinite(angle) ? Math.round(angle) : 135,
  };
}

export function serializeFill(fill: FillState): string {
  if (fill.kind === 'solid') return fill.first;
  if (fill.kind === 'linear') {
    return `linear-gradient(${fill.angle}deg, ${fill.first} 0%, ${fill.second} 100%)`;
  }
  if (fill.kind === 'radial') {
    return `radial-gradient(circle at center, ${fill.first} 0%, ${fill.second} 100%)`;
  }
  return `conic-gradient(from ${fill.angle}deg at center, ${fill.first} 0deg, ${fill.second} 360deg)`;
}
