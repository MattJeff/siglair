import { describe, expect, it } from 'vitest';
import type { PlanInfo } from '../../../lib/types';
import { billingState, mustUsePortal, normalizeSubscription, seatMath } from './compute';

/** Grille du contrat §6, telle que /api/config la sert. Aucun prix inventé ici. */
const TEAM: PlanInfo = {
  plan: 'team',
  name: 'Team',
  price_eur_month: 5.9,
  per_seat: true,
  min_seats: 3,
  ai_generations: 100,
  ai_generations_monthly: true,
  limits: {
    signatures: null,
    campaigns: 'team',
    assets_bytes: 0,
    analytics_days: 365,
    hosted_gif: true,
    org_templates: true,
    branding: false,
  },
};
const PRO: PlanInfo = { ...TEAM, plan: 'pro', name: 'Pro', price_eur_month: 7.9, per_seat: false, min_seats: 1 };

describe('normalizeSubscription', () => {
  it('lit le plan quand le serveur envoie un OBJET et non une chaîne', () => {
    // Ce que `src/billing/mod.rs` envoie réellement : `Plan::get(&org.plan)` sérialisé.
    // Le lire comme une chaîne affichait « — » à la place de la formule.
    const sub = normalizeSubscription({
      plan: { plan: 'team', name: 'Team', price_eur_month: 5.9, per_seat: true, min_seats: 3 },
      seats: 4,
      status: 'active',
      current_period_end: '2026-09-01T00:00:00Z',
      has_subscription: true,
      min_seats: 3,
    });
    expect(sub.planId).toBe('team');
    expect(sub.seats).toBe(4);
    expect(sub.hasSubscription).toBe(true);
  });

  it('accepte aussi la chaîne, pour le jour où le serveur suivra types.ts', () => {
    expect(normalizeSubscription({ plan: 'pro' }).planId).toBe('pro');
  });

  it('retombe sur free devant une réponse absente ou inconnue', () => {
    expect(normalizeSubscription(null).planId).toBe('free');
    expect(normalizeSubscription({ plan: 'enterprise' }).planId).toBe('free');
    expect(normalizeSubscription({}).hasSubscription).toBe(false);
  });
});

describe('billingState', () => {
  const at = (status: string | null, periodEnd: string | null = null) =>
    billingState(
      { planId: 'team', status, seats: 3, periodEnd, hasSubscription: true, minSeats: 3 },
      new Date('2026-08-10T12:00:00Z')
    );

  it('regroupe les deux statuts d’impayé sur le même bandeau', () => {
    expect(at('past_due')).toBe('past_due');
    expect(at('unpaid')).toBe('past_due');
  });

  it('distingue le 3-D Secure d’un échec de paiement', () => {
    // L'utilisateur croit avoir payé : sans cet état il ne comprend pas son plan inactif.
    expect(at('incomplete')).toBe('action_required');
  });

  it('sépare un abonnement résilié encore couru de celui qui est éteint', () => {
    expect(at('canceled', '2026-09-01T00:00:00Z')).toBe('canceled_running');
    expect(at('canceled', '2026-07-01T00:00:00Z')).toBe('canceled');
    expect(at('canceled', null)).toBe('canceled');
  });

  it('n’invente pas de bandeau pour un statut inconnu', () => {
    expect(at('some_future_status')).toBe('active');
  });

  it('distingue « jamais facturé » de « actif »', () => {
    expect(
      billingState({ planId: 'free', status: null, seats: 1, periodEnd: null, hasSubscription: false, minSeats: 1 })
    ).toBe('none');
  });

  it('n’envoie au Checkout que les états où Stripe n’encaisse plus', () => {
    // `checkout` renvoie 409 sur un abonnement vivant : un bouton qui y mène est un cul-de-sac.
    expect(mustUsePortal('active')).toBe(true);
    expect(mustUsePortal('past_due')).toBe(true);
    expect(mustUsePortal('canceled_running')).toBe(false);
    expect(mustUsePortal('none')).toBe(false);
  });
});

describe('seatMath', () => {
  it('applique le minimum de 3 sièges du contrat §6', () => {
    const m = seatMath(TEAM, 1, 0);
    expect(m.billed).toBe(3);
    expect(m.monthlyTotal).toBe(17.7); // et pas 17.700000000000003
  });

  it('ne facture jamais moins que l’effectif présent', () => {
    // Fenêtre où un membre est entré mais où sync_seats n'a pas encore joint Stripe.
    expect(seatMath(TEAM, 7, 3).billed).toBe(7);
    expect(seatMath(TEAM, 7, 3).monthlyTotal).toBe(41.3);
  });

  it('dit qu’un membre de plus est gratuit tant qu’un siège payé est libre', () => {
    const m = seatMath(TEAM, 1, 3);
    expect(m.free).toBe(2);
    expect(m.nextMemberCost).toBe(0);
  });

  it('facture le membre suivant dès que les sièges payés sont tous occupés', () => {
    const m = seatMath(TEAM, 3, 3);
    expect(m.free).toBe(0);
    expect(m.nextMemberCost).toBe(5.9);
  });

  it('ne compte pas les membres sur un plan facturé à l’organisation', () => {
    const m = seatMath(PRO, 9, 1);
    expect(m.monthlyTotal).toBe(7.9);
    expect(m.nextMemberCost).toBe(0);
  });

  it('survit à une absence de plan sans afficher NaN', () => {
    expect(seatMath(null, 0, 0).monthlyTotal).toBe(0);
  });

  it('survit à un champ de sièges vidé sans afficher NaN €', () => {
    const m = seatMath(TEAM, 1, Number.NaN);
    expect(m.billed).toBe(3);
    expect(m.monthlyTotal).toBe(17.7);
  });
});
