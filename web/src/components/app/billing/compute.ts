/**
 * Calculs de facturation — purs, sans React, testés par `compute.test.ts`.
 *
 * Aucun montant n'est écrit ici. Les prix viennent tous de `PlanInfo`, servi par
 * `/api/config` depuis `src/plans.rs` (source unique, contrat §6). Ce fichier ne fait
 * que combiner ces prix avec l'effectif et l'état de l'abonnement.
 *
 * Il existe parce que la page ne doit pas faire d'arithmétique de sous dans du JSX :
 * `7.90 * 3` vaut `23.700000000000003` en IEEE 754, et un centime de trop affiché sur
 * un écran de facturation est un ticket de support.
 */
import type { Plan, PlanInfo } from '../../../lib/types';

/** Arrondi au centime. Les prix arrivent en euros flottants, pas en centimes entiers. */
const round2 = (n: number): number => Math.round(n * 100) / 100;

/* ------------------------------------------------------------------ */
/* Lecture de /api/billing/subscription                                */
/* ------------------------------------------------------------------ */

/**
 * L'état d'abonnement tel que la page l'utilise, normalisé.
 *
 * `web/src/lib/types.ts` déclare `Subscription.plan: Plan` (une chaîne) et un
 * `cancel_at_period_end: boolean`. Ce n'est pas ce que le serveur envoie :
 * `src/billing/mod.rs` sérialise `"plan": Plan::get(&org.plan)`, donc un OBJET
 * `{plan, name, price_eur_month, …}`, et n'envoie aucun `cancel_at_period_end`.
 * `tsc` ne type pas une réponse réseau : le désaccord ne se voit qu'à l'écran.
 * On normalise ici plutôt que de faire confiance au type déclaré.
 */
export interface SubState {
  planId: Plan;
  /** Statut Stripe brut, ou `null` si l'organisation n'a jamais été facturée. */
  status: string | null;
  /** Sièges facturés (quantité de la ligne d'abonnement Stripe). */
  seats: number;
  periodEnd: string | null;
  hasSubscription: boolean;
  minSeats: number;
}

const PLAN_IDS: readonly string[] = ['free', 'pro', 'team'];

/** Un identifiant inconnu retombe sur `free` : moins de droits, jamais plus. */
const asPlanId = (v: unknown): Plan =>
  typeof v === 'string' && PLAN_IDS.includes(v) ? (v as Plan) : 'free';

const asNumber = (v: unknown, fallback: number): number =>
  typeof v === 'number' && Number.isFinite(v) ? v : fallback;

/**
 * Lit la réponse du serveur sans lui faire confiance sur la forme.
 *
 * `plan` est accepté sous ses deux formes (objet Stripe-side, ou chaîne) : le jour où
 * le serveur alignera sa réponse sur `types.ts`, cette page continuera de fonctionner
 * au lieu d'afficher « — » à la place de la formule.
 */
export function normalizeSubscription(raw: unknown): SubState {
  const o = (raw ?? {}) as Record<string, unknown>;
  const plan = o.plan;
  const planId = asPlanId(
    typeof plan === 'string' ? plan : (plan as { plan?: unknown } | null)?.plan
  );
  const status = typeof o.status === 'string' && o.status !== '' ? o.status : null;

  return {
    planId,
    status,
    seats: Math.max(0, Math.trunc(asNumber(o.seats, 0))),
    periodEnd: typeof o.current_period_end === 'string' ? o.current_period_end : null,
    // `has_subscription` est envoyé par le serveur mais absent de `types.ts`. À défaut,
    // un statut présent trahit déjà un abonnement.
    hasSubscription: typeof o.has_subscription === 'boolean' ? o.has_subscription : status !== null,
    minSeats: Math.max(1, Math.trunc(asNumber(o.min_seats, 1))),
  };
}

/* ------------------------------------------------------------------ */
/* Machine à états de l'affichage                                      */
/* ------------------------------------------------------------------ */

/**
 * Ce que l'écran doit raconter. Volontairement distinct du statut Stripe brut : deux
 * statuts différents (`past_due`, `unpaid`) demandent le même bandeau et la même action,
 * et un `canceled` ne se raconte pas pareil selon que la période courue est finie ou non.
 */
export type BillingState =
  | 'none'
  | 'trialing'
  | 'active'
  | 'past_due'
  | 'action_required'
  | 'canceled_running'
  | 'canceled'
  | 'paused';

/**
 * `now` est injecté : « résilié mais encore actif jusqu'au 12 » dépend de l'horloge, et
 * un test qui dépend de l'horloge réelle casse tout seul un jour de février.
 */
export function billingState(sub: SubState, now: Date = new Date()): BillingState {
  const { status } = sub;
  if (status === null) return sub.hasSubscription ? 'active' : 'none';

  switch (status) {
    // Impayé : Stripe passe de `past_due` à `unpaid` selon SA politique de relance.
    // Pour l'utilisateur c'est la même chose — une carte à corriger.
    case 'past_due':
    case 'unpaid':
      return 'past_due';
    // 3-D Secure : le paiement n'est ni accepté ni refusé, il attend la banque.
    case 'incomplete':
      return 'action_required';
    case 'paused':
      return 'paused';
    case 'trialing':
      return 'trialing';
    case 'canceled':
    case 'incomplete_expired':
      return isFuture(sub.periodEnd, now) ? 'canceled_running' : 'canceled';
    case 'active':
      return 'active';
    // Statut inventé par une future version de l'API : pas de bandeau inventé avec.
    // Le statut brut reste affiché tel quel dans le badge.
    default:
      return 'active';
  }
}

function isFuture(iso: string | null, now: Date): boolean {
  if (!iso) return false;
  const t = new Date(iso).getTime();
  return Number.isFinite(t) && t > now.getTime();
}

/** Les états où l'abonnement prélève encore : le Checkout est alors refusé (409). */
const LIVE: readonly BillingState[] = ['active', 'trialing', 'past_due'];

/**
 * Vrai quand un changement de plan doit passer par le portail Stripe et non par le
 * Checkout.
 *
 * `src/billing/mod.rs::checkout` renvoie un 409 « Un abonnement est déjà actif » dès que
 * l'organisation a un abonnement vivant. Un bouton « Passer à Team » qui mène à cette
 * erreur n'est pas un détail cosmétique : c'est un cul-de-sac sur la page qui encaisse.
 *
 * C'est aussi la bonne réponse produit : le portail affiche la proration exacte avant
 * confirmation, ce que nous ne savons pas calculer sans l'API de prévisualisation.
 */
export const mustUsePortal = (state: BillingState): boolean => LIVE.includes(state);

/* ------------------------------------------------------------------ */
/* Sièges et montants                                                  */
/* ------------------------------------------------------------------ */

export interface SeatMath {
  perSeat: boolean;
  /** Membres réellement présents dans l'organisation. */
  used: number;
  /** Sièges facturés. */
  billed: number;
  /** Sièges payés mais libres. Un nouveau membre y entre sans surcoût. */
  free: number;
  minSeats: number;
  /** Total mensuel hors taxes, au tarif du plan. */
  monthlyTotal: number;
  /** Ce que coûterait UN membre de plus, en plus du total actuel. */
  nextMemberCost: number;
}

/**
 * Miroir de `billed_seats()` (src/billing/mod.rs) : les sièges facturés ne descendent
 * jamais sous l'effectif présent ni sous le minimum du plan (§6). On recalcule le
 * plancher plutôt que d'afficher `seats` tel quel, sinon l'écran montre une facture plus
 * basse que la réalité pendant la fenêtre où Stripe n'a pas encore été synchronisé.
 */
export function seatMath(info: PlanInfo | null, used: number, billedSeats: number): SeatMath {
  const perSeat = info?.per_seat === true;
  const minSeats = Math.max(1, info?.min_seats ?? 1);
  const price = info?.price_eur_month ?? 0;
  const members = Math.max(0, used);

  if (!perSeat) {
    return {
      perSeat: false,
      used: members,
      billed: 1,
      free: 0,
      minSeats: 1,
      monthlyTotal: round2(price),
      // Pro et Free sont facturés à l'organisation : un membre de plus ne coûte rien.
      nextMemberCost: 0,
    };
  }

  // `billedSeats` peut venir d'un <input> vidé par l'utilisateur : sans ce garde, un NaN
  // se propage jusqu'au montant affiché et la carte annonce « NaN € ».
  const wanted = Number.isFinite(billedSeats) ? billedSeats : 0;
  const billed = Math.max(wanted, minSeats, members);
  return {
    perSeat: true,
    used: members,
    billed,
    free: Math.max(0, billed - members),
    minSeats,
    monthlyTotal: round2(price * billed),
    // Tant qu'un siège payé reste libre, le membre suivant est déjà financé : la
    // quantité Stripe visée est `max(membres, minimum)`, elle ne bouge pas.
    nextMemberCost: members + 1 <= billed ? 0 : round2(price),
  };
}
