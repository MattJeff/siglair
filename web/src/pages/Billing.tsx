import { useEffect, useState } from 'react';
import { Button } from '../components/Button';
import { Field } from '../components/Field';
import { Input } from '../components/Input';
import { Modal } from '../components/Modal';
import { Spinner } from '../components/Spinner';
import { useToast } from '../components/Toast';
import { AppShell } from '../components/app/AppShell';
import {
  billingState,
  mustUsePortal,
  normalizeSubscription,
  seatMath,
  type BillingState,
  type SubState,
} from '../components/app/billing/compute';
import { apiMessage, formatDate, formatPrice, planFeatures } from '../components/app/helpers';
import { createCheckout, getSubscription, openBillingPortal } from '../lib/api';
import { useSession } from '../lib/session';
import type { PlanInfo } from '../lib/types';
import s from './app.module.css';

/** Libellés des états Stripe. Un état inconnu s'affiche tel quel plutôt que d'être masqué. */
const STATUS_LABEL: Record<string, string> = {
  active: 'Actif',
  trialing: 'Période d’essai',
  past_due: 'Paiement en retard',
  unpaid: 'Impayé',
  paused: 'En pause',
  canceled: 'Résilié',
  incomplete: 'Authentification requise',
  incomplete_expired: 'Paiement abandonné',
};

/** Le badge suit l'urgence, pas le nom du statut. */
const BADGE: Partial<Record<BillingState, string>> = {
  past_due: 'badgeErr',
  action_required: 'badgeErr',
  canceled: 'badgeErr',
  canceled_running: 'badgeWarn',
  paused: 'badgeWarn',
  trialing: 'badgeWork',
  active: 'badgeOk',
};

export default function Billing() {
  const { plan, plans, usage, features, currentOrg } = useSession();
  const toast = useToast();

  const [sub, setSub] = useState<SubState | null>(null);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState('');
  const [seatsFor, setSeatsFor] = useState<PlanInfo | null>(null);
  const [wantedSeats, setWantedSeats] = useState(0);

  const billingOn = features?.billing === true;

  useEffect(() => {
    if (!billingOn) return;
    let alive = true;
    getSubscription()
      .then((r) => {
        // Le serveur n'envoie pas tout à fait la forme déclarée dans types.ts ; c'est
        // `normalizeSubscription` qui fait foi (voir son commentaire).
        if (alive) setSub(normalizeSubscription(r));
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

  const activePlanId = sub?.planId ?? plan;
  const current = plans.find((p) => p.plan === activePlanId) ?? null;
  const members = usage?.members ?? 0;
  const seats = seatMath(current, members, sub?.seats ?? 0);
  const state = sub ? billingState(sub) : null;
  const viaPortal = state !== null && mustUsePortal(state);

  /** Choix d'un plan : le portail dès qu'un abonnement encaisse, sinon le Checkout. */
  const choose = (info: PlanInfo) => {
    if (viaPortal) {
      void goPortal();
      return;
    }
    if (info.per_seat) {
      setWantedSeats(Math.max(info.min_seats, members));
      setSeatsFor(info);
      return;
    }
    void goCheckout(info);
  };

  const portalButton = (label: string) => (
    <Button loading={busy === 'portal'} onClick={() => void goPortal()}>
      {label}
    </Button>
  );

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

        {/* ---------------- États qui demandent une action ---------------- */}

        {state === 'past_due' && (
          <div className={`${s.notice} ${s.alert}`} role="alert">
            <span>
              <strong>Votre dernier paiement a échoué.</strong> Votre accès reste complet pendant
              la période de grâce. Passé ce délai, l’organisation repasse aux limites du plan
              gratuit et les GIF hébergés cessent d’être servis — y compris dans les e-mails déjà
              partis, chez des destinataires que vous ne contrôlez plus.
            </span>
            {portalButton('Corriger le moyen de paiement')}
          </div>
        )}

        {state === 'action_required' && (
          <div className={`${s.notice} ${s.alert}`} role="alert">
            <span>
              <strong>Votre banque demande une confirmation.</strong> Le paiement n’est ni accepté
              ni refusé : il attend une authentification 3-D Secure. Tant qu’elle n’est pas faite,
              votre plan reste inactif et la facture sera abandonnée au bout de quelques jours.
              L’opération prend moins d’une minute.
            </span>
            {portalButton('Confirmer le paiement')}
          </div>
        )}

        {state === 'canceled_running' && (
          <div className={`${s.notice} ${s.warn}`}>
            <span>
              <strong>Abonnement résilié.</strong> Vous gardez toutes les fonctions payantes
              jusqu’au {formatDate(sub?.periodEnd ?? null)}. Après cette date, l’organisation
              repasse au plan gratuit.
            </span>
            {current && current.price_eur_month > 0 && (
              <Button loading={busy === current.plan} onClick={() => choose(current)}>
                Réactiver l’abonnement
              </Button>
            )}
          </div>
        )}

        {state === 'paused' && (
          <div className={`${s.notice} ${s.warn}`}>
            <span>
              <strong>Abonnement en pause.</strong> Plus aucun prélèvement n’a lieu et
              l’organisation suit les limites du plan gratuit. Vos signatures sont conservées.
            </span>
            {portalButton('Reprendre l’abonnement')}
          </div>
        )}

        {state === 'trialing' && (
          <p className={s.notice}>
            Période d’essai en cours. Le premier prélèvement aura lieu le{' '}
            {formatDate(sub?.periodEnd ?? null)}.
          </p>
        )}

        {/* ---------------- Plan courant ---------------- */}

        <section className={s.card}>
          <div className={s.cardHead}>
            <h2 className={s.cardTitle}>Plan actuel</h2>
            {sub?.status && (
              <span className={`${s.badge} ${state ? (s[BADGE[state] ?? ''] ?? '') : ''}`}>
                {STATUS_LABEL[sub.status] ?? sub.status}
              </span>
            )}
          </div>

          {sub === null && !error ? (
            <Spinner size={22} label="Chargement de l’abonnement" />
          ) : (
            <>
              <div className={s.grid}>
                <div className={s.stat}>
                  <p className={s.muted}>Formule</p>
                  <p className={s.statValue}>{current?.name ?? '—'}</p>
                  {seats.perSeat && <p className={s.muted}>{seats.billed} sièges facturés</p>}
                </div>
                <div className={s.stat}>
                  <p className={s.muted}>
                    {state === 'canceled_running' ? 'Actif jusqu’au' : 'Prochaine échéance'}
                  </p>
                  <p className={s.statValue}>{formatDate(sub?.periodEnd ?? null)}</p>
                </div>
                <div className={s.stat}>
                  <p className={s.muted}>Montant par mois</p>
                  <p className={s.statValue}>{formatPrice(seats.monthlyTotal)}</p>
                  <p className={s.muted}>hors taxes</p>
                </div>
              </div>

              {/* La TVA est calculée par Stripe à l'émission : elle dépend du pays du
                  preneur et de son numéro intracommunautaire, que nous n'affichons pas
                  ici faute de les recevoir. On énonce le régime, jamais un taux inventé. */}
              {seats.monthlyTotal > 0 && (
                <p className={s.muted}>
                  Les prix sont indiqués hors taxes, en euros. La TVA applicable est calculée sur
                  la facture d’après votre pays et votre numéro de TVA intracommunautaire :{' '}
                  <strong className={s.strong}>TVA française</strong> pour un preneur établi en
                  France, <strong className={s.strong}>autoliquidation</strong> (article 283-2 du
                  CGI, 0 %) pour un assujetti d’un autre État membre ayant fourni un numéro
                  valide. Le montant TTC exact figure sur chaque facture.
                </p>
              )}
              <p className={s.muted}>
                Contrat conclu entre professionnels : le droit de rétractation des consommateurs
                ne s’applique pas.
              </p>
            </>
          )}
        </section>

        {/* ---------------- Sièges (plans facturés au membre) ---------------- */}

        {seats.perSeat && sub !== null && (
          <section className={s.card}>
            <div className={s.cardHead}>
              <h2 className={s.cardTitle}>Sièges</h2>
            </div>
            <div className={s.grid}>
              <div className={s.stat}>
                <p className={s.muted}>Utilisés / facturés</p>
                <p className={s.statValue}>
                  {seats.used} / {seats.billed}
                </p>
                <p className={s.muted}>
                  {seats.free > 0
                    ? `${seats.free} siège${seats.free > 1 ? 's' : ''} déjà payé${
                        seats.free > 1 ? 's' : ''
                      } et libre${seats.free > 1 ? 's' : ''}`
                    : 'Tous les sièges payés sont occupés'}
                </p>
              </div>
              <div className={s.stat}>
                <p className={s.muted}>Un membre de plus</p>
                <p className={s.statValue}>
                  {seats.nextMemberCost === 0 ? 'Inclus' : `+ ${formatPrice(seats.nextMemberCost)}`}
                </p>
                <p className={s.muted}>
                  {seats.nextMemberCost === 0
                    ? 'Aucun surcoût tant qu’un siège reste libre'
                    : 'par mois, au prorata jusqu’à la prochaine échéance'}
                </p>
              </div>
              <div className={s.stat}>
                <p className={s.muted}>Minimum facturé</p>
                <p className={s.statValue}>{seats.minSeats} sièges</p>
                <p className={s.muted}>quel que soit l’effectif</p>
              </div>
            </div>
            <p className={s.muted}>
              Le nombre de sièges suit automatiquement les arrivées et les départs de votre
              équipe. Il ne descend jamais sous l’effectif présent.
            </p>
          </section>
        )}

        {/* ---------------- Factures ---------------- */}

        <section className={s.card}>
          <div className={s.cardHead}>
            <h2 className={s.cardTitle}>Factures et moyen de paiement</h2>
          </div>
          <p className={s.muted}>
            Vos factures acquittées — avec le détail de la TVA, notre numéro de TVA
            intracommunautaire et les mentions légales — sont émises et conservées par notre
            prestataire de paiement Stripe. Vous les téléchargez en PDF depuis le portail, qui
            sert aussi à changer de carte, à modifier votre adresse de facturation ou votre numéro
            de TVA, et à résilier.
          </p>
          <div className={s.actions}>{portalButton('Ouvrir le portail de paiement')}</div>
        </section>

        {/* ---------------- Changement de plan ---------------- */}

        <section>
          <div className={s.cardHead}>
            <h2 className={s.cardTitle}>Changer de plan</h2>
          </div>
          {viaPortal && (
            <p className={s.muted}>
              Un abonnement est déjà en cours. Les changements de plan passent par le portail
              Stripe, qui affiche le montant exact de la proration — ce que vous payez ou ce qui
              vous est recrédité pour la période déjà entamée — avant toute confirmation.
            </p>
          )}
          <div className={s.grid}>
            {plans.map((info) => {
              const isCurrent = info.plan === activePlanId;
              const total = seatMath(info, members, sub?.seats ?? 0).monthlyTotal;
              return (
                <div key={info.plan} className={`${s.planCard} ${isCurrent ? s.planCurrent : ''}`}>
                  <h3 className={s.cardTitle}>{info.name}</h3>
                  <p className={s.price}>
                    {info.price_eur_month === 0 ? 'Gratuit' : formatPrice(info.price_eur_month)}
                    {info.price_eur_month > 0 && (
                      <span className={s.muted}>{info.per_seat ? ' / membre / mois' : ' / mois'}</span>
                    )}
                  </p>
                  {info.per_seat && (
                    <p className={s.muted}>
                      À partir de {info.min_seats} sièges, soit {formatPrice(total)} par mois pour
                      votre équipe actuelle.
                    </p>
                  )}
                  <ul className={s.features}>
                    {planFeatures(info).map((f) => (
                      <li key={f.label} className={f.on ? undefined : s.featureOff}>
                        {f.label}
                      </li>
                    ))}
                  </ul>
                  <span className={s.spacer} />
                  {isCurrent ? (
                    <Button disabled>Plan actuel</Button>
                  ) : info.price_eur_month === 0 ? (
                    <Button variant="ghost" loading={busy === 'portal'} onClick={() => void goPortal()}>
                      Résilier depuis le portail
                    </Button>
                  ) : (
                    <Button
                      loading={busy === info.plan || (viaPortal && busy === 'portal')}
                      onClick={() => choose(info)}
                    >
                      {viaPortal ? `Passer à ${info.name} (voir la proration)` : `Passer à ${info.name}`}
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
            hint={`Minimum ${seatsFor?.min_seats ?? 0}. Vous êtes ${members} dans l’organisation aujourd’hui.`}
          >
            <Input
              type="number"
              className={s.seats}
              min={seatsFor?.min_seats ?? 1}
              step={1}
              value={wantedSeats}
              onChange={(e) => setWantedSeats(Number(e.currentTarget.value))}
            />
          </Field>
          <p className={s.muted}>
            {seatsFor &&
              `Soit ${formatPrice(
                seatMath(seatsFor, members, wantedSeats).monthlyTotal
              )} par mois hors taxes. La TVA et le montant total vous sont indiqués sur la page de paiement avant de valider.`}
          </p>
          <div className={s.actions}>
            <Button
              loading={busy === seatsFor?.plan}
              onClick={() => {
                if (seatsFor) {
                  void goCheckout(seatsFor, seatMath(seatsFor, members, wantedSeats).billed);
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
