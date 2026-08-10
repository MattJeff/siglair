/**
 * Tarifs détaillés. Les trois plans, leurs prix et leurs quotas viennent de
 * /api/config (contrat §6) : rien n'est écrit deux fois dans le dépôt.
 */
import { Link } from 'react-router-dom';
import { Faq } from '../components/marketing/Faq';
import type { QA } from '../components/marketing/Faq';
import { START_HREF, SiteFooter, SiteHeader, usePageMeta } from '../components/marketing/Chrome';
import { PlanCards, PlanComparison } from '../components/marketing/PlanCards';
import { formatPrice } from '../components/app/helpers';
import { useSession } from '../lib/session';
import s from './marketing.module.css';

/** Illustration de l'argument Team. Deux nombres d'exemple, aucun quota produit. */
const EXAMPLE_SEATS = 20;
const EXAMPLE_MAILS_PER_DAY = 40;

const BILLING_FAQ: QA[] = [
  {
    q: 'Puis-je changer de plan à tout moment ?',
    a: 'Oui. Depuis votre espace de facturation, vous passez d’un plan à l’autre quand vous voulez. Le changement prend effet immédiatement et l’écart est ajusté sur votre facture suivante, au prorata du temps déjà payé.',
  },
  {
    q: 'Comment fonctionne la facturation par membre du plan Team ?',
    a: 'Team se facture par membre actif de l’organisation, avec un nombre minimum de membres indiqué sur la carte du plan. Vous ajoutez ou retirez quelqu’un, la facturation suit à la période suivante.',
  },
  {
    q: 'Suis-je engagé sur la durée ?',
    a: 'Non en mensuel : vous résiliez quand vous voulez et le service reste actif jusqu’à la fin de la période déjà réglée. L’offre annuelle, elle, couvre douze mois et c’est ce qui permet d’en offrir deux.',
  },
  {
    q: 'Que se passe-t-il si j’arrête de payer ?',
    a: 'Votre organisation repasse au plan gratuit à la fin de la période réglée. Vos signatures, vos images et vos réglages restent en place, et votre URL hébergée continue de servir votre GIF animé. Ce qui change : la mention Siglair revient dans l’export de la signature, les campagnes datées s’arrêtent et l’historique des ouvertures et des clics n’est plus tenu.',
  },
  {
    q: 'Comment se passe le paiement, et où sont mes factures ?',
    a: 'Le paiement est traité par Stripe : nous ne voyons ni ne stockons votre numéro de carte. Vos factures, votre moyen de paiement et votre résiliation se gèrent depuis le portail Stripe, accessible en un clic depuis votre espace de facturation.',
  },
  {
    q: 'Le plan gratuit est-il limité dans le temps ?',
    a: 'Non. Il n’expire pas et ne demande pas de carte bancaire. Il donne une signature animée, hébergée sur son URL, avec une discrète mention « Signature animée avec Siglair » sous la signature. Passer à Pro enlève cette mention et ouvre les campagnes datées et la mesure des clics.',
  },
  {
    q: 'Que change exactement l’offre annuelle ?',
    a: 'Vous réglez douze mois et vous en payez dix : le montant économisé est affiché sur chaque carte quand vous basculez sur « Annuel ». Les fonctionnalités sont identiques, seule la périodicité de facturation change.',
  },
  {
    q: 'Facturez-vous la TVA ?',
    a: 'Siglair est édité par KAIROS, SAS française, et vend à des professionnels. Le régime applicable est déterminé au paiement par Stripe d’après votre pays et votre numéro de TVA intracommunautaire : TVA française pour un client français, autoliquidation à 0 % pour un assujetti d’un autre État membre ayant fourni un numéro valide, TVA de votre pays sinon. Le numéro de TVA se saisit dans le formulaire de paiement, et la facture porte les mentions correspondantes.',
  },
  {
    q: 'Ai-je un droit de rétractation de quatorze jours ?',
    a: 'Non : le droit de rétractation du code de la consommation vise les consommateurs, et Siglair est vendu à des professionnels pour les besoins de leur activité. C’est aussi pourquoi le plan gratuit existe sans carte bancaire — vous essayez le produit en entier avant de payer, et vous pouvez résilier à tout moment depuis le portail de paiement.',
  },
];

export default function Pricing() {
  usePageMeta(
    'Tarifs des signatures et campagnes email | Siglair',
    'Le plan gratuit héberge votre signature animée. Pro enlève la marque et ouvre les campagnes datées. Team fait des emails de votre équipe un canal marketing piloté.',
  );

  // Le prix de l'exemple Team vient de /api/config comme le reste : aucun montant en dur.
  const { plans } = useSession();
  const team = plans.find((p) => p.plan === 'team') ?? null;
  const teamExample = team ? formatPrice(team.price_eur_month * EXAMPLE_SEATS) : null;

  return (
    <div className={s.page}>
      <SiteHeader />

      <main id="contenu" className={s.main}>
        <section className={`${s.wrap} ${s.section}`}>
          <div className={s.sectionHead}>
            <span className={s.kicker}>Tarifs</span>
            <h1>Gratuit pour signer. Payant pour diffuser.</h1>
            <p className={s.lead}>
              Le plan gratuit ne montre pas le produit, il le donne : une signature animée,
              hébergée sur son URL, avec une discrète mention Siglair. Pro l’enlève et ouvre les
              campagnes datées sur vos signatures. Team pousse une campagne sur les signatures de
              toute l’équipe, en un clic, et compte les clics qu’elle rapporte.
            </p>
          </div>

          <PlanCards withCycle />
        </section>

        <section className={`${s.wrap} ${s.section}`}>
          <div className={s.sectionHead}>
            <span className={s.kicker}>Le plan Team</span>
            <h2>Ce n’est pas de la gestion de signatures.</h2>
            <p className={s.lead}>
              C’est un canal de diffusion que vous payez déjà : les emails que votre équipe envoie
              de toute façon.
            </p>
          </div>
          <div className={`${s.card} ${s.answer}`}>
            <h3 className={s.answerTitle}>
              {EXAMPLE_SEATS} commerciaux, {EXAMPLE_MAILS_PER_DAY} emails par jour chacun :{' '}
              {EXAMPLE_SEATS * EXAMPLE_MAILS_PER_DAY} emails quotidiens qui portent votre campagne.
            </h3>
            <p>
              Et pas vers une liste froide : vers des clients, des prospects et des partenaires qui
              vous connaissent déjà et qui ouvrent parce que la conversation est en cours. Vous
              programmez la bannière une fois, vous la <strong>poussez sur les signatures de
              toute l’organisation en un clic</strong>, et personne n’ouvre son client mail : les
              signatures déjà collées pointent vers l’URL hébergée, c’est le rendu qui change.
            </p>
            <p>
              Chaque campagne porte ses dates, et le serveur compte les ouvertures et les clics,
              CTA par CTA. La question suivante n’est plus « on met quoi ? » mais « laquelle a
              marché ».
            </p>
            {teamExample && (
              <p>
                Pour une équipe de {EXAMPLE_SEATS} personnes, c’est <strong>{teamExample} par
                mois</strong>. C’est ce canal-là que vous achetez, pas des signatures.
              </p>
            )}
          </div>
        </section>

        <section className={`${s.wrap} ${s.section}`}>
          <div className={s.sectionHead}>
            <span className={s.kicker}>Comparatif</span>
            <h2>Ligne par ligne.</h2>
          </div>
          <PlanComparison />
          <p className={s.note}>
            Les quotas affichés sont ceux appliqués par le serveur : ils sont vérifiés à chaque
            écriture, jamais seulement dans l’interface.
          </p>
        </section>

        <section className={`${s.wrap} ${s.section}`}>
          <div className={s.sectionHead}>
            <span className={s.kicker}>Facturation</span>
            <h2>Les questions d’argent, sans détour.</h2>
          </div>
          <Faq items={BILLING_FAQ} />
        </section>

        <section className={`${s.wrap} ${s.section}`}>
          <div className={s.final}>
            <h2>Essayez avant de payer quoi que ce soit.</h2>
            <p>
              Le plan gratuit vous donne une signature animée, hébergée, installée dans votre
              client mail — avec notre mention sous le bloc. Vous saurez en dix minutes si le
              produit vous convient.
            </p>
            <div className={s.actions}>
              <Link className={`${s.btn} ${s.btnPrimary} ${s.btnBig}`} to={START_HREF}>
                Commencer gratuitement
              </Link>
              <Link className={`${s.btn} ${s.btnBig}`} to="/">
                Revoir le produit
              </Link>
            </div>
          </div>
        </section>
      </main>

      <SiteFooter />
    </div>
  );
}
