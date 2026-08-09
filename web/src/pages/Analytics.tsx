import { useEffect, useMemo, useState } from 'react';
import { Link, useParams } from 'react-router-dom';
import { Button } from '../components/Button';
import { Field } from '../components/Field';
import { Select } from '../components/Select';
import { Spinner } from '../components/Spinner';
import { useToast } from '../components/Toast';
import { AppShell } from '../components/app/AppShell';
import { apiMessage, formatDay, formatRetention } from '../components/app/helpers';
import { getAnalytics, getSignature, updateOrg } from '../lib/api';
import { useSession } from '../lib/session';
import type { AnalyticsPoint, AnalyticsSeries } from '../lib/types';
import s from './app.module.css';

/** Périodes proposées, bornées par la rétention du plan (jamais codée en dur ici). */
function periodOptions(retentionDays: number): number[] {
  const all = [7, 30, 90, 365];
  const kept = all.filter((d) => d <= retentionDays);
  return kept.length > 0 ? kept : [retentionDays];
}

function cutoff(days: number): string {
  const d = new Date();
  d.setDate(d.getDate() - (days - 1));
  return d.toISOString().slice(0, 10);
}

export default function Analytics() {
  const { id = '' } = useParams();
  const { limits, currentOrg, refresh } = useSession();
  const toast = useToast();

  const retention = limits?.analytics_days ?? 0;
  const enabled = currentOrg?.analytics_enabled !== false;
  const allowed = retention > 0 && enabled;

  const [name, setName] = useState('');
  const [series, setSeries] = useState<AnalyticsSeries | null>(null);
  const [error, setError] = useState('');
  const [days, setDays] = useState(0);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    setDays(periodOptions(retention)[0] ?? 0);
  }, [retention]);

  useEffect(() => {
    let alive = true;
    void getSignature(id)
      .then((sig) => {
        if (alive) setName(sig.name);
      })
      .catch(() => undefined);
    return () => {
      alive = false;
    };
  }, [id]);

  useEffect(() => {
    if (!allowed) return;
    let alive = true;
    getAnalytics(id)
      .then((r) => {
        if (alive) setSeries(r);
      })
      .catch((e: unknown) => {
        if (alive) setError(apiMessage(e));
      });
    return () => {
      alive = false;
    };
  }, [id, allowed]);

  const points = useMemo(() => {
    if (!series) return [];
    const from = cutoff(days || 30);
    return series.points.filter((p) => p.date >= from);
  }, [series, days]);

  const opens = points.reduce((n, p) => n + p.opens, 0);
  const clicks = points.reduce((n, p) => n + p.clicks, 0);
  const ctr = opens > 0 ? Math.round((clicks / opens) * 1000) / 10 : 0;

  const reenable = async () => {
    if (!currentOrg) return;
    setBusy(true);
    try {
      await updateOrg(currentOrg.id, { analytics_enabled: true });
      await refresh();
      toast('Mesure réactivée.', 'success');
    } catch (e) {
      toast(apiMessage(e), 'error');
    } finally {
      setBusy(false);
    }
  };

  const title = name ? `Statistiques — ${name}` : 'Statistiques';

  if (retention === 0) {
    return (
      <AppShell title={title}>
        <div className={s.empty}>
          <h2 className={s.emptyTitle}>Les statistiques font partie des plans payants</h2>
          <p className={s.muted}>
            Ouvertures et clics, jour par jour, et le détail des éléments cliqués : c’est ce qui
            permet de savoir si votre signature travaille pour vous.
          </p>
          <Link className={s.linkBtn} to="/app/billing">
            Voir les plans
          </Link>
        </div>
      </AppShell>
    );
  }

  if (!enabled) {
    const canManage = currentOrg?.role === 'owner' || currentOrg?.role === 'admin';
    return (
      <AppShell title={title}>
        <div className={s.empty}>
          <h2 className={s.emptyTitle}>La mesure est désactivée pour cette organisation</h2>
          <p className={s.muted}>
            Aucune ouverture ni aucun clic n’est enregistré : c’est un choix de confidentialité,
            et il est respecté tant qu’il est actif. Les signatures continuent de fonctionner
            normalement.
          </p>
          {canManage && (
            <Button loading={busy} onClick={() => void reenable()}>
              Réactiver la mesure
            </Button>
          )}
        </div>
      </AppShell>
    );
  }

  const options = periodOptions(retention);

  return (
    <AppShell
      title={title}
      subtitle={`Votre plan conserve les données ${formatRetention(retention)}.`}
      actions={
        <Link className={s.linkBtn} to="/app">
          Retour aux signatures
        </Link>
      }
    >
      <div className={s.stack}>
        {error && (
          <p className={`${s.notice} ${s.alert}`} role="alert">
            {error}
          </p>
        )}

        {series === null && !error && (
          <div className={s.center}>
            <Spinner size={28} label="Chargement des statistiques" />
          </div>
        )}

        {series !== null && (
          <>
            <div className={s.narrow}>
              <Field label="Période">
                <Select value={days} onChange={(e) => setDays(Number(e.currentTarget.value))}>
                  {options.map((d) => (
                    <option key={d} value={d}>
                      {d >= 365 ? '12 derniers mois' : `${d} derniers jours`}
                    </option>
                  ))}
                </Select>
              </Field>
            </div>

            <div className={s.grid}>
              <div className={s.stat}>
                <p className={s.muted}>Ouvertures</p>
                <p className={s.statValue}>{opens}</p>
              </div>
              <div className={s.stat}>
                <p className={s.muted}>Clics</p>
                <p className={s.statValue}>{clicks}</p>
              </div>
              <div className={s.stat}>
                <p className={s.muted}>Taux de clic</p>
                <p className={s.statValue}>{ctr} %</p>
              </div>
            </div>

            {opens === 0 && clicks === 0 ? (
              <div className={s.empty}>
                <h2 className={s.emptyTitle}>Aucune ouverture pour l’instant</h2>
                <p className={s.muted}>
                  Les données arrivent quand vos destinataires ouvrent vos emails. Envoyez un
                  message avec votre signature, puis revenez ici — le comptage n’est pas
                  instantané côté messagerie.
                </p>
              </div>
            ) : (
              <div className={s.card}>
                <div className={s.cardHead}>
                  <h2 className={s.cardTitle}>Jour par jour</h2>
                  <p className={s.legend}>
                    <span>
                      <span className={s.swatch} style={{ background: 'var(--blue)' }} />
                      Ouvertures
                    </span>
                    <span>
                      <span className={s.swatch} style={{ background: 'var(--cyan)' }} />
                      Clics
                    </span>
                  </p>
                </div>
                <Chart points={points} opens={opens} clicks={clicks} />
              </div>
            )}

            <div className={s.card}>
              <h2 className={s.cardTitle}>Éléments les plus cliqués</h2>
              {series.top_elements.length === 0 ? (
                <p className={s.muted}>
                  Aucun clic enregistré. Seuls les éléments porteurs d’un lien sont cliquables.
                </p>
              ) : (
                <ul className={`${s.rows} ${s.plainList}`}>
                  {series.top_elements.map((el) => (
                    <li key={el.element_id} className={s.row}>
                      <div className={s.rowMain}>
                        <p className={s.rowName}>{el.label || el.element_id}</p>
                        <div
                          className={s.bar}
                          style={{
                            width: `${Math.max(
                              4,
                              (el.clicks / Math.max(...series.top_elements.map((x) => x.clicks))) *
                                100,
                            )}%`,
                          }}
                        />
                      </div>
                      <p className={s.strong}>{el.clicks}</p>
                    </li>
                  ))}
                </ul>
              )}
            </div>

            {/* Un chiffre présenté comme exact alors qu'il ne l'est pas détruit la confiance. */}
            <div className={`${s.notice} ${s.warn}`}>
              <span>
                <strong>Ces chiffres sont des ordres de grandeur.</strong> Une ouverture est
                comptée quand la messagerie charge l’image. La protection de la confidentialité
                d’Apple Mail précharge les images sans que personne n’ait rien lu : elle
                surestime. Les clients qui bloquent les images et le cache du proxy d’images de
                Gmail, eux, sous-estiment. Les clics, en revanche, sont fiables : ils viennent
                d’une action réelle.
              </span>
            </div>
          </>
        )}
      </div>
    </AppShell>
  );
}

/* Graphique en barres, SVG pur : deux barres par jour, aucune librairie. */
function Chart({
  points,
  opens,
  clicks,
}: {
  points: AnalyticsPoint[];
  opens: number;
  clicks: number;
}) {
  const W = 720;
  const H = 220;
  const TOP = 10;
  const BOTTOM = 28;
  const plot = H - TOP - BOTTOM;
  const max = Math.max(1, ...points.map((p) => Math.max(p.opens, p.clicks)));
  const slot = W / Math.max(1, points.length);
  const bw = Math.max(1, Math.min(12, slot / 2 - 1));
  const h = (v: number) => (v / max) * plot;

  const first = points[0];
  const last = points[points.length - 1];

  return (
    <>
      <div className={s.chartWrap}>
        <svg
          className={s.chart}
          viewBox={`0 0 ${W} ${H}`}
          role="img"
          aria-label={`Ouvertures et clics par jour. ${opens} ouvertures et ${clicks} clics sur la période.`}
        >
          <g className={s.gridLines}>
            <line x1="0" y1={TOP} x2={W} y2={TOP} />
            <line x1="0" y1={TOP + plot / 2} x2={W} y2={TOP + plot / 2} />
            <line x1="0" y1={TOP + plot} x2={W} y2={TOP + plot} />
          </g>
          {points.map((p, i) => {
            const x = i * slot + (slot - bw * 2 - 2) / 2;
            return (
              <g key={p.date}>
                <rect
                  x={x}
                  y={TOP + plot - h(p.opens)}
                  width={bw}
                  height={h(p.opens)}
                  rx="2"
                  fill="var(--blue)"
                />
                <rect
                  x={x + bw + 2}
                  y={TOP + plot - h(p.clicks)}
                  width={bw}
                  height={h(p.clicks)}
                  rx="2"
                  fill="var(--cyan)"
                />
              </g>
            );
          })}
          <text className={s.axis} x="0" y={TOP + plot + 18}>
            {first ? formatDay(first.date) : ''}
          </text>
          <text className={s.axis} x={W} y={TOP + plot + 18} textAnchor="end">
            {last ? formatDay(last.date) : ''}
          </text>
          <text className={s.axis} x="0" y={TOP - 1}>
            {max}
          </text>
        </svg>
      </div>

      {/* Un graphique n'est pas lisible au lecteur d'écran : les données restent accessibles. */}
      <details>
        <summary className={s.muted}>Voir les données en tableau</summary>
        <table className={s.table}>
          <caption className={s.muted}>Ouvertures et clics par jour</caption>
          <thead>
            <tr>
              <th scope="col">Jour</th>
              <th scope="col">Ouvertures</th>
              <th scope="col">Clics</th>
            </tr>
          </thead>
          <tbody>
            {points.map((p) => (
              <tr key={p.date}>
                <th scope="row">{formatDay(p.date)}</th>
                <td>{p.opens}</td>
                <td>{p.clicks}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </details>
    </>
  );
}
