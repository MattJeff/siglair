import { Aperture, CircleDot, MoveHorizontal, PaintBucket } from 'lucide-react';
import type { ComponentType } from 'react';
import { parseFill, serializeFill } from './paint';
import type { FillKind, FillState } from './paint';
import s from './editor.module.css';

interface FillControlProps {
  value: string;
  onChange: (value: string) => void;
  label?: string;
}

const MODES: { kind: FillKind; label: string; icon: ComponentType<{ size?: number }> }[] = [
  { kind: 'solid', label: 'Uni', icon: PaintBucket },
  { kind: 'linear', label: 'Linéaire', icon: MoveHorizontal },
  { kind: 'radial', label: 'Radial', icon: CircleDot },
  { kind: 'conic', label: 'Conique', icon: Aperture },
];

const PRESETS = [
  ['#315cff', '#39d4ff'],
  ['#7c3aed', '#ec4899'],
  ['#0f766e', '#4ade80'],
  ['#f59e0b', '#ef4444'],
  ['#111827', '#475569'],
  ['#f8fafc', '#cbd5e1'],
] as const;


export function FillControl({ value, onChange, label = 'Remplissage' }: FillControlProps) {
  const fill = parseFill(value);
  const update = (patch: Partial<FillState>) => onChange(serializeFill({ ...fill, ...patch }));

  return (
    <div className={s.fillControl}>
      <span className={s.fillLabel}>{label}</span>
      <div className={s.fillModes} role="group" aria-label={`Type de ${label.toLowerCase()}`}>
        {MODES.map(({ kind, label: modeLabel, icon: Icon }) => (
          <button
            key={kind}
            type="button"
            className={fill.kind === kind ? s.fillModeActive : undefined}
            aria-pressed={fill.kind === kind}
            title={modeLabel}
            onClick={() => update({ kind })}
          >
            <Icon size={15} />
            <span>{modeLabel}</span>
          </button>
        ))}
      </div>

      <div className={s.fillPreview} style={{ background: serializeFill(fill) }} aria-hidden="true" />

      <div className={s.fillColors}>
        <label>
          <span>{fill.kind === 'solid' ? 'Couleur' : 'Départ'}</span>
          <input type="color" value={fill.first} onChange={(event) => update({ first: event.target.value })} />
        </label>
        {fill.kind !== 'solid' && (
          <label>
            <span>Arrivée</span>
            <input type="color" value={fill.second} onChange={(event) => update({ second: event.target.value })} />
          </label>
        )}
      </div>

      {(fill.kind === 'linear' || fill.kind === 'conic') && (
        <label className={s.fillAngle}>
          <span>Angle</span>
          <input
            type="range"
            min={0}
            max={360}
            step={5}
            value={fill.angle}
            onChange={(event) => update({ angle: Number(event.target.value) })}
          />
          <output>{fill.angle}°</output>
        </label>
      )}

      {fill.kind !== 'solid' && (
        <div className={s.fillPresets} aria-label="Dégradés suggérés">
          {PRESETS.map(([first, second]) => (
            <button
              key={`${first}-${second}`}
              type="button"
              title={`${first} vers ${second}`}
              aria-label={`Appliquer le dégradé ${first} vers ${second}`}
              style={{ background: `linear-gradient(135deg, ${first}, ${second})` }}
              onClick={() => update({ first, second })}
            />
          ))}
        </div>
      )}
    </div>
  );
}
