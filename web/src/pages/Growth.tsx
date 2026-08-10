/**
 * `/app/growth` — le tableau de bord de croissance (contrat §11.4).
 *
 * Un seul principe de mise en page, et c'est tout le sujet : **clics et conversions en
 * gros, en premier, en couleur ; vues en petit, grisées, avec leur imprécision écrite à
 * côté.** Apple Mail Privacy Protection précharge les images depuis ses serveurs et le
 * proxy de Gmail met en cache : une « vue » peut n'être qu'une machine. Mettre ce chiffre
 * en avant, c'est vendre une mesure fausse — et le jour où l'utilisateur s'en aperçoit, il
 * ne croit plus aucun chiffre de la page. La mention est donc à l'écran, en une phrase,
 * pas dans une infobulle.
 *
 * Le taux qui décide est le dernier : payant / création. Le détail par signature dit
 * laquelle rapporte réellement des comptes, donc laquelle copier.
 */
import { useEffect, useMemo, useState } from 'react';
import { Link } from 'react-router-dom';
import { Button } from '../components/Button';
import { Field } from '../components/Field';
import { Select } from '../components/Select';
import { Spinner } from '../components/Spinner';
import { useToast } from '../components/Toast';
import { AppShell } from '../components/app/AppShell';
import { apiMessage, formatRetention } from '../components/app/helpers';
import { FunnelChart } from '../components/app/growth/FunnelChart';
import {
  formatCount,
  formatRate,
  periodOptions,
  rates,
  type Rates,
} from '../components/app/growth/api';
import g from '../components/app/growth/growth.module.css';
import { getGrowth, updateOrg } from '../lib/api';
import type { GrowthReport } from '../lib/types';
import { useSession } from '../lib/session';
import s from './app.module.css';

const plural = (n: number, one: string, many = `${one}s`) => (n > 1 ? many : one);

export default function Growth() {
  const { user, limits, currentOrg, refresh } = useSession();
  const toast = useToast();

  // Aucun quota en dur : la rétention vient de /api/me, comme partout ailleurs (§6).
  const retention = limits?.analytics_days ?? 0;
  const enabled = currentOrg?.analytics_enabled !== false;
  const allowed = retention > 0 && enabled;

  const [days, setDays] = useState(0);
  const [report, setReport] = useState<GrowthReport | null>(null);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    setDays(periodOptions(retention)[0] ?? 0);
  }, [retention]);

  useEffect(() => {
    if (!allowed || days <= 0) return;
    let alive = true;
    setError('');
    getGrowth(days)
      .then((r) => {
        if (alive) setReport(r);
      })
      .catch((e: unknown) => {
        if (alive) setError(apiMessage(e));
      });
    return () => {
      alive = false;
    };
  }, [allowed, days]);

  const rate: Rates = useMemo(
    () => rates(report?.funnel ?? { views: 0, badge_clicks: 0, signups: 0, paid: 0 }),
    [report],
  );

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

  /* ---------------- Écrans dédiés : jamais un tableau vide sans explication ---------------- */

  if (retention === 0) {
    return (
      <AppShell title="Croissance">
        <div className={s.empty}>
          <h2 className={s.emptyTitle}>La mesure de la croissance fait partie des plans payants</h2>
          <p className={s.muted}>
            Combien de destinataires cliquent sur votre badge, combien créent un compte,
            combien deviennent clients : c’est ce qui dit si vos signatures vous ramènent du
            monde, ou seulement des ouvertures.
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
      <AppShell title="Croissance">
        <div className={s.empty}>
          <h2 className={s.emptyTitle}>La mesure est désactivée pour cette organisation</h2>
          <p className={s.muted}>
            Ni ouverture, ni clic sur le badge n’est enregistré : c’est un choix de
            confidentialité, et il est respecté tant qu’il est actif. Vos signatures
            continuent de fonctionner normalement — seule cette page reste vide.
          </p>
          {canManage ? (
            <Button loading={busy} onClick={() => void reenable()}>
              Réactiver la mesure
            </Button>
          ) : (
            <p className={s.muted}>Seul un propriétaire ou un administrateur peut la réactiver.</p>
          )}
        </div>
      </AppShell>
    );
  }

  /* ---------------- La page ---------------- */

  const options = periodOptions(retention);
  const f = report?.funnel;

  return (
    <AppShell
      title="Croissance"
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

        {!report && !error && (
          <div className={s.center}>
            <Spinner size={28} label="Chargement de la croissance" />
          </div>
        )}

        {report && f && (
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

            {/* La phrase qu'on doit pouvoir dire à voix haute (§11.4). Les vues y sont,
                mais en petit et en gris : elles décrivent, elles ne pilotent pas. */}
            <p className={g.headline}>
              <span className={g.headlineName}>{user?.name ?? user?.email ?? 'Vous'}</span>
              <span className={g.headlineSep}>·</span>
              <span className={g.headlineSoft}>
                {formatCount(f.views)} {plural(f.views, 'vue')}
              </span>
              <span className={g.headlineSep}>·</span>
              <span>
                {formatCount(f.badge_clicks)} {plural(f.badge_clicks, 'clic')} sur le badge
              </span>
              <span className={g.headlineSep}>·</span>
              <span>
                {formatCount(f.signups)} {plural(f.signups, 'compte créé', 'comptes créés')}
              </span>
              <span className={g.headlineSep}>·</span>
              <span>
                {formatCount(f.paid)} {plural(f.paid, 'payant')}
              </span>
            </p>

            {/* 1. Ce qui est fiable et ce qui décide : en gros, en couleur. */}
            <div className={g.hero}>
              <HeroCard
                label="Clics sur le badge"
                value={f.badge_clicks}
                color="var(--cyan)"
                rate={rate.click}
                of="des vues (indicatif)"
              />
              <HeroCard
                label="Comptes créés"
                value={f.signups}
                color="var(--blue)"
                rate={rate.signup}
                of="des clics sur le badge"
              />
              <HeroCard
                decisive
                label="Comptes payants"
                value={f.paid}
                color="var(--green)"
                rate={rate.paid}
                of="des comptes créés"
              />
            </div>

            {/* 2. Les vues, en petit et grisées, avec la raison juste à côté. */}
            <div className={g.views}>
              <span className={g.viewsValue}>
                {formatCount(f.views)} {plural(f.views, 'vue')}
              </span>
              <span className={g.viewsWhy}>
                Chiffre indicatif : Apple Mail précharge les images depuis ses serveurs et le
                proxy de Gmail les met en cache — une vue peut n’être qu’une machine. Aucune
                décision ne devrait s’appuyer dessus.
              </span>
            </div>

            {f.badge_clicks === 0 && f.signups === 0 ? (
              <div className={s.empty}>
                <h2 className={s.emptyTitle}>Aucun clic pour l’instant</h2>
                <p className={s.muted}>
                  Les données arrivent quand vos destinataires cliquent sur le badge de vos
                  signatures. Envoyez quelques emails, puis revenez ici.
                </p>
              </div>
            ) : (
              <div className={s.card}>
                <div className={s.cardHead}>
                  <h2 className={s.cardTitle}>De la vue au client</h2>
                  <p className={s.muted}>
                    Le dernier taux est celui qui compte : il dit si le canal vaut quelque chose.
                  </p>
                </div>
                <FunnelChart funnel={f} rate={rate} />
              </div>
            )}

            {/* 3. L'information actionnable : quelle signature copier. */}
            <div className={s.card}>
              <div className={s.cardHead}>
                <h2 className={s.cardTitle}>Par signature</h2>
                <p className={s.muted}>Triées par comptes créés, pas par ouvertures.</p>
              </div>
              {report.signatures.length === 0 ? (
                <p className={s.muted}>
                  Aucune signature n’a encore rapporté de clic sur son badge. Le badge n’apparaît
                  que sur les signatures publiées.
                </p>
              ) : (
                <ul className={s.plainList}>
                  {report.signatures.map((sig, i) => (
                    <li key={sig.id} className={g.sigRow}>
                      <div className={g.sigMain}>
                        <p className={g.sigName}>
                          <Link to={`/app/editor/${sig.id}`}>{sig.name}</Link>
                          {i === 0 && sig.signups > 0 && (
                            <span className={g.sigTag}>celle qui convertit</span>
                          )}
                        </p>
                        <p className={g.sigViews}>
                          {formatCount(sig.views)} {plural(sig.views, 'vue')} · chiffre indicatif
                          {/* Booléen, jamais un compteur : une seule ligne par signature en
                              base, et le signal veut dire « chargée hors du navigateur de
                              son propriétaire » — donc collée quelque part. */}
                          {sig.installed && ' · installée'}
                        </p>
                      </div>
                      <div className={g.sigStats}>
                        <SigStat value={sig.badge_clicks} label="clics" color="var(--cyan)" />
                        <SigStat value={sig.signups} label="comptes" color="var(--blue)" />
                        <SigStat value={sig.paid} label="payants" color="var(--green)" />
                      </div>
                    </li>
                  ))}
                </ul>
              )}
            </div>

            {/* Le badge n'est imposé que sur le plan à marque (§6). Sans cette ligne, un
                client Pro conclurait que le canal s'est éteint tout seul. */}
            {limits?.branding === false && (
              <p className={`${s.notice} ${s.warn}`}>
                Votre plan retire la marque Siglair de vos exports : vos signatures ne portent
                plus le badge, et ne produisent donc plus de nouveaux clics ni de nouveaux
                comptes. Les chiffres ci-dessus sont ceux de la période où il était présent.
              </p>
            )}
          </>
        )}
      </div>
    </AppShell>
  );
}

/** Un chiffre de conversion : gros, coloré, et son taux de passage juste dessous. */
function HeroCard({
  label,
  value,
  color,
  rate,
  of,
  decisive = false,
}: {
  label: string;
  value: number;
  color: string;
  rate: number | null;
  /** Dénominateur, en toutes lettres : « 1,6 % » sans « de quoi » ne veut rien dire. */
  of: string;
  /** La carte qui décide du canal (payant / création) porte un liseré. */
  decisive?: boolean;
}) {
  return (
    <div className={`${g.heroCard} ${decisive ? g.heroCardKey : ''}`}>
      <p className={g.heroLabel}>{label}</p>
      <p className={g.heroValue} style={{ ['--accent' as string]: color }}>
        {formatCount(value)}
      </p>
      <p className={g.heroRate}>
        {rate === null ? 'Pas encore de donnée' : `${formatRate(rate)} ${of}`}
      </p>
    </div>
  );
}

function SigStat({ value, label, color }: { value: number; label: string; color: string }) {
  return (
    <div className={g.sigStat}>
      <p className={g.sigStatValue} style={{ ['--accent' as string]: color }}>
        {formatCount(value)}
      </p>
      <p className={g.sigStatLabel}>{label}</p>
    </div>
  );
}
