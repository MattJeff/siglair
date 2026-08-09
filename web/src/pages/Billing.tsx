import { useEffect, useState } from 'react';
import { Button } from '../components/Button';
import { Field } from '../components/Field';
import { Input } from '../components/Input';
import { Modal } from '../components/Modal';
import { Spinner } from '../components/Spinner';
import { useToast } from '../components/Toast';
import { AppShell } from '../components/app/AppShell';
import { apiMessage, formatDate, formatPrice, planFeatures } from '../components/app/helpers';
import { createCheckout, getSubscription, openBillingPortal } from '../lib/api';
import { useSession } from '../lib/session';
import type { PlanInfo, Subscription } from '../lib/types';
import s from './app.module.css';

/** Libellés des états Stripe. Un état inconnu s'affiche tel quel plutôt que d'être masqué. */
const STATUS_LABEL: Record<string, string> = {
  active: 'Actif',
  trialing: 'Période d’essai',
  past_due: 'Paiement en retard',
  canceled: 'Résilié',
  unpaid: 'Impayé',
  incomplete: 'Paiement incomplet',
  incomplete_expired: 'Paiement abandonné',
};

export default function Billing() {
  const { plan, plans, usage, features, currentOrg } = useSession();
  const toast = useToast();

  const [sub, setSub] = useState<Subscription | null>(null);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState('');
  const [seatsFor, setSeatsFor] = useState<PlanInfo | null>(null);
  const [seats, setSeats] = useState(0);

  const billingOn = features?.billing === true;

  useEffect(() => {
    if (!billingOn) return;
    let alive = true;
    getSubscription()
      .then((r) => {
        if (alive) setSub(r);
      })
      .catch((e: unknown) => {
        if (alive) setError(apiMessage(e));
      });
    return () => {
      alive = false;
    };
  }, [billingOn]);

  const goCheckout = async (info: PlanInfo, wanted?: number) => {
    setBusy(info.plan);
    try {
      const { url } = await createCheckout(info.plan, wanted);
      location.assign(url);
    } catch (e) {
      toast(apiMessage(e), 'error');
      setBusy('');
    }
  };

  const goPortal = async () => {
    setBusy('portal');
    try {
      const { url } = await openBillingPortal();
      location.assign(url);
    } catch (e) {
      toast(apiMessage(e), 'error');
      setBusy('');
    }
  };

  const choose = (info: PlanInfo) => {
    if (info.per_seat) {
      setSeats(Math.max(info.min_seats, usage?.members ?? 0));
      setSeatsFor(info);
      return;
    }
    void goCheckout(info);
  };

  if (!billingOn) {
    return (
      <AppShell title="Abonnement">
        <div className={s.empty}>
          <h2 className={s.emptyTitle}>La facturation n’est pas activée sur cette instance</h2>
          <p className={s.muted}>
            Aucun paiement n’est configuré côté serveur. Contactez l’administrateur de votre
            installation.
          </p>
        </div>
      </AppShell>
    );
  }

  const status = sub?.status ?? null;
  const pastDue = status === 'past_due' || status === 'unpaid';
  const current = plans.find((p) => p.plan === (sub?.plan ?? plan)) ?? null;

  return (
    <AppShell
      title="Abonnement"
      subtitle={currentOrg?.name}
      actions={
        <Button variant="ghost" loading={busy === 'portal'} onClick={() => void goPortal()}>
          Portail de paiement
        </Button>
      }
    >
      <div className={s.stack}>
        {error && (
          <p className={`${s.notice} ${s.alert}`} role="alert">
            {error}
          </p>
        )}

        {pastDue && (
          <div className={`${s.notice} ${s.alert}`} role="alert">
            <span>
              <strong>Votre dernier paiement a échoué.</strong> Sans moyen de paiement valide,
              l’abonnement sera résilié et vos signatures repasseront aux limites du plan gratuit
              — les URL hébergées cesseront de servir vos GIF.
            </span>
            <Button loading={busy === 'portal'} onClick={() => void goPortal()}>
              Mettre à jour le moyen de paiement
            </Button>
          </div>
        )}

        {sub?.cancel_at_period_end && !pastDue && (
          <p className={`${s.notice} ${s.warn}`}>
            Votre abonnement prend fin le {formatDate(sub.current_period_end)}. Vous gardez toutes
            les fonctions payantes jusqu’à cette date.
          </p>
        )}

        <section className={s.card}>
          <div className={s.cardHead}>
            <h2 className={s.cardTitle}>Plan actuel</h2>
            {status && <span className={s.badge}>{STATUS_LABEL[status] ?? status}</span>}
          </div>

          {sub === null && !error ? (
            <Spinner size={22} label="Chargement de l’abonnement" />
          ) : (
            <div className={s.grid}>
              <div className={s.stat}>
                <p className={s.muted}>Formule</p>
                <p className={s.statValue}>{current?.name ?? '—'}</p>
              </div>
              <div className={s.stat}>
                <p className={s.muted}>Prochaine échéance</p>
                <p className={s.statValue}>{formatDate(sub?.current_period_end ?? null)}</p>
              </div>
              <div className={s.stat}>
                <p className={s.muted}>Sièges</p>
                <p className={s.statValue}>
                  {usage?.members ?? 0} / {sub?.seats ?? usage?.members ?? 0}
                </p>
                <p className={s.muted}>utilisés / facturés</p>
              </div>
            </div>
          )}

          <p className={s.muted}>
            Vos factures, reçus et le moyen de paiement se gèrent dans le portail de paiement.
          </p>
        </section>

        <section>
          <div className={s.cardHead}>
            <h2 className={s.cardTitle}>Changer de plan</h2>
          </div>
          <div className={s.grid}>
            {plans.map((info) => {
              const isCurrent = info.plan === (sub?.plan ?? plan);
              return (
                <div
                  key={info.plan}
                  className={`${s.planCard} ${isCurrent ? s.planCurrent : ''}`}
                >
                  <h3 className={s.cardTitle}>{info.name}</h3>
                  <p className={s.price}>
                    {info.price_eur_month === 0 ? 'Gratuit' : formatPrice(info.price_eur_month)}
                    {info.price_eur_month > 0 && (
                      <span className={s.muted}>
                        {info.per_seat ? ' / membre / mois' : ' / mois'}
                      </span>
                    )}
                  </p>
                  {info.per_seat && (
                    <p className={s.muted}>À partir de {info.min_seats} sièges.</p>
                  )}
                  <ul className={s.features}>
                    {planFeatures(info.limits).map((f) => (
                      <li key={f.label} className={f.on ? undefined : s.featureOff}>
                        {f.label}
                      </li>
                    ))}
                  </ul>
                  <span className={s.spacer} />
                  {isCurrent ? (
                    <Button disabled>Plan actuel</Button>
                  ) : info.price_eur_month === 0 ? (
                    <Button
                      variant="ghost"
                      loading={busy === 'portal'}
                      onClick={() => void goPortal()}
                    >
                      Résilier depuis le portail
                    </Button>
                  ) : (
                    <Button loading={busy === info.plan} onClick={() => choose(info)}>
                      Passer à {info.name}
                    </Button>
                  )}
                </div>
              );
            })}
          </div>
        </section>
      </div>

      <Modal
        open={seatsFor !== null}
        onClose={() => setSeatsFor(null)}
        title={`Passer à ${seatsFor?.name ?? ''}`}
      >
        <div className={s.stack}>
          <Field
            label="Nombre de sièges"
            hint={`Minimum ${seatsFor?.min_seats ?? 0}. Vous êtes ${
              usage?.members ?? 0
            } dans l’organisation aujourd’hui.`}
          >
            <Input
              type="number"
              className={s.seats}
              min={seatsFor?.min_seats ?? 1}
              step={1}
              value={seats}
              onChange={(e) => setSeats(Number(e.currentTarget.value))}
            />
          </Field>
          <p className={s.muted}>
            {seatsFor &&
              `Soit ${formatPrice(seatsFor.price_eur_month * Math.max(seats, seatsFor.min_seats))} par mois.`}
          </p>
          <div className={s.actions}>
            <Button
              loading={busy === seatsFor?.plan}
              onClick={() => {
                if (seatsFor) {
                  void goCheckout(seatsFor, Math.max(seats, seatsFor.min_seats));
                }
              }}
            >
              Continuer vers le paiement
            </Button>
            <Button variant="ghost" onClick={() => setSeatsFor(null)}>
              Annuler
            </Button>
          </div>
        </div>
      </Modal>
    </AppShell>
  );
}
