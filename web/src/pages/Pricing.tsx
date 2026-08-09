/**
 * Tarifs détaillés. Les trois plans, leurs prix et leurs quotas viennent de
 * /api/config (contrat §6) : rien n'est écrit deux fois dans le dépôt.
 */
import { Link } from 'react-router-dom';
import { Faq } from '../components/marketing/Faq';
import type { QA } from '../components/marketing/Faq';
import { START_HREF, SiteFooter, SiteHeader, usePageMeta } from '../components/marketing/Chrome';
import { PlanCards, PlanComparison } from '../components/marketing/PlanCards';
import s from './marketing.module.css';

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
    a: 'Votre organisation repasse au plan gratuit à la fin de la période réglée. Vos signatures, vos images et vos réglages restent accessibles et exportables en HTML statique. Ce qui s’arrête, c’est l’hébergement du GIF animé sur une URL — la fonctionnalité payante.',
  },
  {
    q: 'Comment se passe le paiement, et où sont mes factures ?',
    a: 'Le paiement est traité par Stripe : nous ne voyons ni ne stockons votre numéro de carte. Vos factures, votre moyen de paiement et votre résiliation se gèrent depuis le portail Stripe, accessible en un clic depuis votre espace de facturation.',
  },
  {
    q: 'Le plan gratuit est-il limité dans le temps ?',
    a: 'Non. Il n’expire pas et ne demande pas de carte bancaire. Il permet de composer une signature complète et de l’exporter en HTML statique ; c’est l’hébergement du GIF animé qui distingue les plans payants.',
  },
];

export default function Pricing() {
  usePageMeta(
    'Tarifs — Siglair',
    'Trois plans : gratuit pour composer et exporter, Pro pour héberger votre GIF animé sur une URL stable, Team pour déployer une signature sur toute une équipe.',
  );

  return (
    <div className={s.page}>
      <SiteHeader />

      <main id="contenu" className={s.main}>
        <section className={`${s.wrap} ${s.section}`}>
          <div className={s.sectionHead}>
            <span className={s.kicker}>Tarifs</span>
            <h1>Ce que vous payez, c’est l’URL hébergée.</h1>
            <p className={s.lead}>
              Composer une signature est gratuit, sans limite de temps ni carte bancaire. Ce qui se
              facture, c’est le rendu du GIF par nos serveurs, son hébergement sur une URL stable
              et la mesure des ouvertures et des clics.
            </p>
          </div>

          <PlanCards withCycle />
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
              Le plan gratuit vous donne une signature complète et son export. Vous saurez en dix
              minutes si le produit vous convient.
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
