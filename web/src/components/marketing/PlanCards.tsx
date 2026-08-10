/**
 * Grille tarifaire — partagée par /pricing et la section Tarifs de la landing.
 *
 * TOUT vient de `/api/config` (session.plans) : prix, quotas, nombre de sièges
 * minimum. Rien n'est écrit en dur (contrat §6) — un changement de tarif ne doit
 * pas demander un déploiement du front.
 *
 * Une seule dérivation locale : l'annuel = dix mois payés (deux mois offerts),
 * parce que l'API n'expose pas encore de prix annuel. Signalé dans le rapport.
 */
import { useId, useState } from 'react';
import type { ReactNode } from 'react';
import { Link } from 'react-router-dom';
import { Spinner } from '../Spinner';
import { formatBytes, formatPrice, formatRetention, planFeatures } from '../app/helpers';
import type { PlanFeature } from '../app/helpers';
import { useSession } from '../../lib/session';
import type { CampaignScope, PlanInfo } from '../../lib/types';
import { START_HREF } from './Chrome';
import s from './plans.module.css';

export type Cycle = 'monthly' | 'yearly';

/**
 * Lien de souscription : /app/billing après connexion (contrat §5.3).
 *
 * `cycle` voyage avec le plan pour que la périodicité choisie ici soit celle envoyée au
 * Checkout (`CheckoutReq.interval`, src/billing/mod.rs). /app/billing ne le lit pas encore :
 * signalé dans le rapport, l'annuel part donc en mensuel tant que ce n'est pas branché.
 */
const billingQuery = (plan: string, cycle: Cycle): string =>
  `plan=${encodeURIComponent(plan)}&cycle=${cycle}`;

const planHref = (plan: string, cycle: Cycle): string =>
  `/login?next=${encodeURIComponent('/app/billing')}&${billingQuery(plan, cycle)}`;

/** ponytail : deux mois offerts sur l'année (contrat §6, `Plan::price_eur_year`).
 *  À déplacer dans PlanInfo le jour où l'API sert un `price_eur_year`. */
const MONTHS_BILLED_YEARLY = 10;
const MONTHS_FREE_YEARLY = 12 - MONTHS_BILLED_YEARLY;

/** Première lettre en capitale : les mêmes formateurs servent en milieu de phrase. */
const cap = (text: string): string => text.charAt(0).toUpperCase() + text.slice(1);

const formatSignatures = (n: number | null): string =>
  n === null ? 'Illimitées' : `${n} signature${n > 1 ? 's' : ''}`;

/** Argumentaire par plan. Aucun quota ici : seulement à qui il s'adresse. */
const PITCH: Record<string, string> = {
  free: 'Une vraie signature animée, hébergée sur son URL, collée dans votre client mail. Elle porte une discrète mention Siglair.',
  pro: 'La même signature sans notre marque, avec des campagnes datées et la mesure des clics.',
  team: 'Les emails que votre équipe envoie déjà deviennent un canal : une campagne, poussée sur toutes les signatures, mesurée.',
};

/** Libellés du comparatif — la même distinction de portée, colonne par colonne. */
const CAMPAIGN_CELL: Record<CampaignScope, string> = {
  none: 'Non incluses',
  own: 'Sur vos signatures',
  team: 'Poussées à toute l’équipe',
};

/**
 * `planFeatures` formule désormais la ligne « campagnes » par portée (`limits.campaigns`),
 * comme `require_paid_plan` côté serveur. Le `map` sur le libellé exact qui vivait ici
 * n'a plus lieu d'être : un couplage par chaîne de moins.
 */
const features = (plan: PlanInfo): PlanFeature[] => planFeatures(plan.limits);

function PlanPrice({ plan, cycle }: { plan: PlanInfo; cycle: Cycle }) {
  if (plan.price_eur_month === 0) {
    return (
      <>
        <p className={s.amount}>
          {formatPrice(0)} <span className={s.unit}>pour toujours</span>
        </p>
        <p className={s.sub}>Sans carte bancaire.</p>
      </>
    );
  }

  const yearly = cycle === 'yearly';
  const amount = yearly ? plan.price_eur_month * MONTHS_BILLED_YEARLY : plan.price_eur_month;
  const per = plan.per_seat ? ' / membre' : '';
  const unit = yearly ? `${per} / an` : `${per} / mois`;

  // L'économie en euros, dérivée du même 10 : « 2 mois offerts » ne dit pas combien.
  const saved = plan.price_eur_month * MONTHS_FREE_YEARLY;
  const seatFloor = plan.per_seat ? ` ${plan.min_seats} membres minimum.` : '';
  const sub = yearly
    ? `Soit ${formatPrice(amount / 12)} par mois${plan.per_seat ? ' et par membre' : ''}. Vous économisez ${formatPrice(saved)}${plan.per_seat ? ' par membre' : ''} sur l’année.${seatFloor}`
    : plan.per_seat
      ? `À partir de ${formatPrice(plan.price_eur_month * plan.min_seats)} par mois (${plan.min_seats} membres minimum).`
      : 'Résiliable à tout moment.';

  return (
    <>
      <p className={s.amount}>
        {formatPrice(amount)} <span className={s.unit}>{unit}</span>
      </p>
      <p className={s.sub}>{sub}</p>
    </>
  );
}

function PlanCard({ plan, cycle }: { plan: PlanInfo; cycle: Cycle }) {
  const { status } = useSession();
  const free = plan.price_eur_month === 0;
  const featured = plan.plan === 'pro';

  // Déjà connecté : inutile de repasser par /login, la page de facturation suffit.
  const href = free
    ? status === 'authenticated'
      ? '/app'
      : START_HREF
    : status === 'authenticated'
      ? `/app/billing?${billingQuery(plan.plan, cycle)}`
      : planHref(plan.plan, cycle);

  return (
    <div className={`${s.card} ${featured ? s.featured : ''}`}>
      {featured && <span className={s.badge}>Recommandé</span>}
      <h3 className={s.name}>{plan.name}</h3>
      <PlanPrice plan={plan} cycle={cycle} />
      <p className={s.pitch}>{PITCH[plan.plan] ?? ''}</p>
      <ul className={s.features}>
        {features(plan).map((f) => (
          <li key={f.label} className={f.on ? undefined : s.off}>
            {f.label}
          </li>
        ))}
      </ul>
      <Link className={`${s.cta} ${featured ? s.ctaPrimary : ''}`} to={href}>
        {free ? 'Commencer gratuitement' : `Choisir ${plan.name}`}
      </Link>
    </div>
  );
}

function CycleSwitch({ value, onChange }: { value: Cycle; onChange: (c: Cycle) => void }) {
  const name = useId();
  return (
    <fieldset className={s.cycle}>
      <legend className={s.sr}>Période de facturation</legend>
      <label className={s.cycleOption}>
        <input
          type="radio"
          name={name}
          checked={value === 'monthly'}
          onChange={() => onChange('monthly')}
        />
        <span>Mensuel</span>
      </label>
      <label className={s.cycleOption}>
        <input
          type="radio"
          name={name}
          checked={value === 'yearly'}
          onChange={() => onChange('yearly')}
        />
        <span>
          Annuel <em className={s.save}>2 mois offerts</em>
        </span>
      </label>
    </fieldset>
  );
}

/** Message unique quand /api/config n'a rien renvoyé : pas de prix inventé. */
function PlansUnavailable({ loading }: { loading: boolean }) {
  if (loading) {
    return (
      <p className={s.loading}>
        <Spinner size={18} label="Chargement des tarifs" />
        Chargement des tarifs…
      </p>
    );
  }
  return (
    <div className={s.empty}>
      <p>
        Les tarifs ne sont pas disponibles pour l’instant. Rechargez la page dans un instant — la
        création de compte, elle, fonctionne.
      </p>
      <p style={{ marginTop: 16 }}>
        <Link to={START_HREF}>Commencer gratuitement</Link>
      </p>
    </div>
  );
}

/**
 * Cartes tarifaires. `withCycle` : afficher le sélecteur mensuel / annuel
 * (page /pricing). La landing montre le mensuel, plus court à lire.
 */
export function PlanCards({ withCycle = false }: { withCycle?: boolean }) {
  const { plans, status } = useSession();
  const [cycle, setCycle] = useState<Cycle>('monthly');

  if (plans.length === 0) return <PlansUnavailable loading={status === 'loading'} />;

  return (
    <>
      {withCycle && <CycleSwitch value={cycle} onChange={setCycle} />}
      <div className={s.cards}>
        {plans.map((p) => (
          <PlanCard key={p.plan} plan={p} cycle={withCycle ? cycle : 'monthly'} />
        ))}
      </div>
      <p className={s.footnote}>
        Prix en euros, servis par notre API. Le régime de TVA applicable est calculé et affiché au
        moment du paiement. Le paiement est traité par Stripe ; nous ne voyons jamais votre numéro
        de carte.
      </p>
    </>
  );
}

/** Comparatif détaillé — /pricing uniquement. */
export function PlanComparison() {
  const { plans } = useSession();
  if (plans.length === 0) return null;

  const yes = (
    <>
      <span aria-hidden="true" className={s.yes}>
        ✓
      </span>
      <span className={s.sr}>Inclus</span>
    </>
  );
  const no = (
    <>
      <span aria-hidden="true" className={s.no}>
        ✕
      </span>
      <span className={s.sr}>Non inclus</span>
    </>
  );

  const rows: { label: string; cell: (p: PlanInfo) => ReactNode }[] = [
    { label: 'Prix mensuel', cell: (p) => (p.price_eur_month === 0 ? 'Gratuit' : `${formatPrice(p.price_eur_month)}${p.per_seat ? ' / membre' : ''}`) },
    {
      label: 'Prix annuel (2 mois offerts)',
      cell: (p) =>
        p.price_eur_month === 0
          ? 'Gratuit'
          : `${formatPrice(p.price_eur_month * MONTHS_BILLED_YEARLY)}${p.per_seat ? ' / membre' : ''}`,
    },
    { label: 'Membres minimum facturés', cell: (p) => (p.per_seat ? `${p.min_seats}` : '1') },
    { label: 'Signatures', cell: (p) => formatSignatures(p.limits.signatures) },
    { label: 'GIF animé hébergé sur une URL', cell: (p) => (p.limits.hosted_gif ? yes : no) },
    { label: 'Campagnes datées', cell: (p) => (p.limits.campaigns === 'none' ? no : CAMPAIGN_CELL[p.limits.campaigns]) },
    { label: 'Historique des ouvertures et des clics', cell: (p) => cap(formatRetention(p.limits.analytics_days)) },
    { label: 'Espace pour les images et les GIF', cell: (p) => formatBytes(p.limits.assets_bytes) },
    { label: 'Modèles d’organisation et déploiement en masse', cell: (p) => (p.limits.org_templates ? yes : no) },
    // Formulé à l'endroit : une coche verte doit toujours vouloir dire « c'est bien ».
    { label: 'Export sans la marque Siglair', cell: (p) => (p.limits.branding ? no : yes) },
  ];

  return (
    <div className={s.tableWrap} tabIndex={0} role="region" aria-label="Comparatif des plans">
      <table className={s.table}>
        <caption>Ce que contient chaque plan</caption>
        <thead>
          <tr>
            <th scope="col">Fonctionnalité</th>
            {plans.map((p) => (
              <th scope="col" key={p.plan}>
                {p.name}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <tr key={row.label}>
              <th scope="row">{row.label}</th>
              {plans.map((p) => (
                <td key={p.plan}>{row.cell(p)}</td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
