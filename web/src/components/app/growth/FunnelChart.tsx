/**
 * L'entonnoir viral en SVG pur — aucune librairie (contrat §11.5 : tout est en première
 * partie, et une dépendance de graphes pèse plus lourd que quatre `<rect>`).
 *
 * La hiérarchie du §11.4 est portée par la COULEUR autant que par la taille : les vues
 * sont grises, les clics et les conversions sont colorés. Un graphique qui peint les
 * ouvertures en bleu vif contredirait la phrase qui, juste au-dessus, dit de s'en méfier.
 */
import type { Funnel } from '../../../lib/types';
import { formatCount, formatRate, type Rates } from './api';
import s from './growth.module.css';

const W = 720;
const ROW = 52;
const LABEL_W = 190;
const NUM_W = 96;
const BAR_H = 20;
const TRACK = W - LABEL_W - NUM_W;

interface Stage {
  key: string;
  label: string;
  value: number;
  /** Jeton de couleur. `--muted` pour les vues : le chiffre le moins fiable est le plus terne. */
  color: string;
  /** Taux de passage depuis l'étape précédente, déjà formaté. */
  step: string | null;
}

function stages(f: Funnel, r: Rates): Stage[] {
  return [
    { key: 'views', label: 'Vues', value: f.views, color: 'var(--muted)', step: null },
    {
      key: 'clicks',
      label: 'Clics sur le badge',
      value: f.badge_clicks,
      color: 'var(--cyan)',
      step: `${formatRate(r.click)} des vues`,
    },
    {
      key: 'signups',
      label: 'Comptes créés',
      value: f.signups,
      color: 'var(--blue)',
      step: `${formatRate(r.signup)} des clics`,
    },
    {
      key: 'paid',
      label: 'Comptes payants',
      value: f.paid,
      color: 'var(--green)',
      step: `${formatRate(r.paid)} des comptes`,
    },
  ];
}

export function FunnelChart({ funnel, rate }: { funnel: Funnel; rate: Rates }) {
  const rows = stages(funnel, rate);
  const max = Math.max(1, ...rows.map((r) => r.value));
  const H = rows.length * ROW;

  // ponytail : échelle linéaire avec un plancher de 3 px. Sans plancher, « 1 payant » face
  // à « 870 vues » fait une barre de zéro pixel et disparaît — or c'est l'étape qui compte.
  // Le nombre exact est écrit à droite de chaque barre : la barre situe, le texte mesure.
  const width = (v: number) => (v === 0 ? 0 : Math.max(3, (v / max) * TRACK));

  return (
    <>
      <div className={s.chartWrap}>
        <svg
          className={s.chart}
          viewBox={`0 0 ${W} ${H}`}
          role="img"
          aria-label={`Entonnoir : ${rows.map((r) => `${r.value} ${r.label.toLowerCase()}`).join(', ')}.`}
        >
          {rows.map((row, i) => {
            const y = i * ROW;
            return (
              <g key={row.key}>
                <text className={s.chartLabel} x="0" y={y + BAR_H}>
                  {row.label}
                </text>
                {/* Rail : donne l'échelle même quand la barre est un trait. */}
                <rect
                  className={s.chartTrack}
                  x={LABEL_W}
                  y={y + 4}
                  width={TRACK}
                  height={BAR_H}
                  rx="4"
                />
                <rect
                  x={LABEL_W}
                  y={y + 4}
                  width={width(row.value)}
                  height={BAR_H}
                  rx="4"
                  fill={row.color}
                />
                <text
                  className={row.key === 'views' ? s.chartNumMuted : s.chartNum}
                  x={W}
                  y={y + BAR_H}
                  textAnchor="end"
                >
                  {formatCount(row.value)}
                </text>
                {row.step && (
                  <text className={s.chartStep} x={LABEL_W} y={y + BAR_H + 18}>
                    {row.step}
                  </text>
                )}
              </g>
            );
          })}
        </svg>
      </div>

      {/* Un graphique ne se lit pas au lecteur d'écran : les données restent accessibles. */}
      <details>
        <summary className={s.summary}>Voir les données en tableau</summary>
        <table className={s.table}>
          <caption className={s.summary}>Entonnoir de croissance sur la période</caption>
          <thead>
            <tr>
              <th scope="col">Étape</th>
              <th scope="col">Nombre</th>
              <th scope="col">Taux de passage</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((row) => (
              <tr key={row.key}>
                <th scope="row">{row.label}</th>
                <td>{formatCount(row.value)}</td>
                <td>{row.step ?? '—'}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </details>
    </>
  );
}
