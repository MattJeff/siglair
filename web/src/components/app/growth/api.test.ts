import { describe, expect, it } from 'vitest';
import { normalizeGrowth } from '../../../lib/api';
import { formatRate, periodOptions, rates } from './api';

describe('rates', () => {
  /** L'exemple du contrat §11.4, celui qu'on doit pouvoir dire à voix haute. */
  it('calcule les trois taux de la phrase de référence', () => {
    const r = rates({ views: 870, badge_clicks: 14, signups: 3, paid: 1 });
    expect(r.click).toBeCloseTo(14 / 870, 12);
    expect(r.signup).toBeCloseTo(3 / 14, 12);
    expect(r.paid).toBeCloseTo(1 / 3, 12);
  });

  /**
   * Le bord qui compte : « 0 % » et « pas encore de donnée » ne se disent pas pareil à
   * quelqu'un qui décide d'un budget. Afficher 0 % ferait couper un canal qui n'a
   * simplement pas démarré. Miroir de `growth::rate` côté Rust, qui rend `Option`.
   */
  it('rend null plutôt que zéro quand il n’y a rien à diviser', () => {
    const vide = rates({ views: 0, badge_clicks: 0, signups: 0, paid: 0 });
    expect(vide).toEqual({ click: null, signup: null, paid: null });
    expect(formatRate(vide.paid)).toBe('—');

    // des vues sans clic : un vrai 0 %, pas une absence de donnée
    const vues = rates({ views: 100, badge_clicks: 0, signups: 0, paid: 0 });
    expect(vues.click).toBe(0);
    expect(vues.signup).toBeNull();
  });
});

describe('normalizeGrowth', () => {
  it('accepte les agrégats à plat comme sous « funnel »', () => {
    const plat = normalizeGrowth({ views: 870, badge_clicks: 14, signups: 3, paid: 1 }, 30);
    const imbrique = normalizeGrowth(
      { funnel: { views: 870, badge_clicks: 14, signups: 3, paid: 1 } },
      30,
    );
    expect(plat.funnel).toEqual(imbrique.funnel);
    expect(plat.days).toBe(30);
  });

  /** Une réponse partielle ou d'une autre version d'API donne 0, jamais NaN à l'écran. */
  it('ne laisse jamais passer un NaN ni une ligne sans identifiant', () => {
    const r = normalizeGrowth(
      {
        views: undefined,
        signatures: [
          { id: 'a', name: 'Ancienne', signups: 0, badge_clicks: 4 },
          { name: 'sans id', signups: 99 },
          null,
        ] as unknown[],
      },
      7,
    );
    expect(r.funnel).toEqual({ views: 0, badge_clicks: 0, signups: 0, paid: 0 });
    expect(r.signatures.map((x) => x.id)).toEqual(['a']);
    expect(r.signatures[0]?.views).toBe(0);
  });

  /** L'écran répond à « laquelle rapporte des comptes », pas à « laquelle est la plus vue ». */
  it('trie par conversions, jamais par ouvertures', () => {
    const r = normalizeGrowth(
      {
        by_signature: [
          { id: 'vue', name: 'Très vue', views: 9000, badge_clicks: 1, signups: 0 },
          { id: 'conv', name: 'Convertit', views: 12, badge_clicks: 3, signups: 2 },
        ],
      },
      30,
    );
    expect(r.signatures.map((x) => x.id)).toEqual(['conv', 'vue']);
  });
});

describe('periodOptions', () => {
  /** §6 : aucune grille en dur — la rétention borne les choix, elle ne les invente pas. */
  it('borne les périodes par la rétention du plan', () => {
    expect(periodOptions(30)).toEqual([7, 30]);
    expect(periodOptions(365)).toEqual([7, 30, 90, 365]);
    // une rétention plus courte que la plus petite période proposée reste offrable
    expect(periodOptions(3)).toEqual([3]);
  });
});
