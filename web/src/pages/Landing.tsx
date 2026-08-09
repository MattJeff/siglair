/**
 * Vitrine. Portage de docs/reference/landing-viral.html : sa voix et sa structure
 * sont validées, on ne les réinvente pas (docs/DESIGN.md §Landing).
 *
 * Aucune preuve sociale : pas de logo client, pas de témoignage, pas de compteur.
 * Tant qu'il n'y a pas de vrais clients, la section n'existe pas.
 */
import { Link } from 'react-router-dom';
import { BrandHero } from '../components/marketing/BrandHero';
import { Faq } from '../components/marketing/Faq';
import type { QA } from '../components/marketing/Faq';
import { START_HREF, SiteFooter, SiteHeader, usePageMeta } from '../components/marketing/Chrome';
import { PlanCards } from '../components/marketing/PlanCards';
import { SignatureDemo } from '../components/marketing/SignatureDemo';
import s from './marketing.module.css';

/**
 * Domaine public du produit (contrat §0), illustratif dans les exemples de code.
 * Lu depuis l'environnement Vite (DESIGN.md §9) : le domaine n'est pas encore acheté,
 * l'écrire en dur garantirait de l'oublier le jour où il change.
 */
const PUBLIC_HOST = import.meta.env.VITE_PUBLIC_HOST ?? 'siglair.app';

const FEATURES: { icon: string; title: string; text: string }[] = [
  {
    icon: '◉',
    title: 'Signature email animée, rendue côté serveur',
    text: 'Siglair génère un GIF optimisé et une image fixe de secours. Vous obtenez une signature professionnelle qui bouge, sans installer d’outil ni exporter de fichier compliqué.',
  },
  {
    icon: '▣',
    title: 'Compatibilité Gmail, Outlook et Apple Mail',
    text: 'La même URL sert l’animation quand le client mail l’accepte, et une première image lisible quand il la bloque. Votre signature reste propre, même dans Outlook Windows.',
  },
  {
    icon: '↗',
    title: 'Clics et ouvertures par bouton',
    text: 'Vous voyez les ouvertures et les clics, élément par élément. Le lien “Prendre rendez-vous”, le site et LinkedIn sont comptés séparément, sans adresse IP conservée en clair.',
  },
  {
    icon: '⬒',
    title: 'Signatures d’équipe centralisées',
    text: 'Un modèle d’organisation, un profil par membre, et chacun garde une signature email cohérente. Le jour où la marque change, vous republiez : l’équipe n’a rien à recoller.',
  },
  {
    icon: '◷',
    title: 'Campagnes et bannières datées',
    text: 'Ajoutez un lancement, un événement, une offre ou un lien de prise de rendez-vous sous chaque email. La campagne commence et s’arrête dans Siglair, pas dans les réglages mail.',
  },
];

const CLIENTS: { name: string; note: string; warn?: boolean }[] = [
  { name: 'Gmail', note: 'Animation complète' },
  { name: 'Apple Mail', note: 'Animation complète' },
  { name: 'Outlook web', note: 'Animation complète' },
  { name: 'Outlook (application Windows)', note: 'Première frame figée', warn: true },
];

const STORY_STEPS: { title: string; text: string }[] = [
  {
    title: 'Votre marque paraît cohérente.',
    text: 'Logo, couleurs, rôle, liens et mentions restent alignés d’une personne à l’autre, même quand l’équipe grandit.',
  },
  {
    title: 'Vos informations restent à jour.',
    text: 'Changement de poste, nouveau logo, campagne terminée : vous republiez la signature au lieu de renvoyer du HTML à tout le monde.',
  },
  {
    title: 'Chaque email devient plus utile.',
    text: 'Un CTA clair peut guider vers votre site, votre calendrier, une démo, un contenu ou une page de contact sans surcharger le message.',
  },
  {
    title: 'Vous gardez le contrôle.',
    text: 'Vous savez ce qui est cliqué, vous gardez un repli Outlook lisible, et vous évitez les signatures bricolées dans chaque client mail.',
  },
];

const FAQ_ITEMS: QA[] = [
  {
    q: 'Pourquoi utiliser une signature email professionnelle hébergée ?',
    a: 'Parce qu’une signature copiée-collée dans Gmail ou Outlook vieillit mal : logo périmé, mauvais poste, lien cassé, bannière oubliée. Avec une signature hébergée, vous collez une URL une fois, puis vous mettez à jour le design, les liens et les campagnes depuis Siglair.',
  },
  {
    q: 'Est-ce que ça marche dans Outlook ?',
    a: 'Oui, avec une nuance que nous préférons dire tout de suite : l’application Outlook pour Windows n’anime pas les GIF, elle affiche la première image et s’arrête là. Votre signature reste donc parfaitement lisible et cliquable, simplement immobile. Nous composons la première frame pour qu’elle tienne debout seule. Sur Gmail, Apple Mail et Outlook sur le web, l’animation se joue normalement.',
  },
  {
    q: 'Est-ce que je peux changer ma signature après l’envoi ?',
    a: 'Oui, et c’est tout l’intérêt. Ce que vous collez dans votre client mail est une image pointant vers une URL que nous hébergeons. Quand vous republiez, cette URL sert la nouvelle version — y compris dans les emails déjà partis, tant qu’ils n’ont pas encore été ouverts. Vous n’avez plus jamais à recoller du HTML dans Outlook.',
  },
  {
    q: 'Mes données sont-elles en Europe ?',
    a: 'Le produit est conçu pour : aucune police ni aucun script n’est chargé depuis un tiers dans votre navigateur, il n’y a pas de traceur publicitaire, et aucune adresse IP n’est conservée en clair — seulement une empreinte tronquée et salée. La liste complète de nos hébergeurs et sous-traitants, avec leur localisation, figure dans la politique de confidentialité.',
    more: { to: '/legal/confidentialite', label: 'Lire la politique de confidentialité' },
  },
  {
    q: 'Que se passe-t-il si j’arrête de payer ?',
    a: 'Votre compte repasse au plan gratuit à la fin de la période déjà réglée. Vos signatures, vos images et vos réglages restent en place et vous pouvez toujours les exporter en HTML statique. Ce qui s’arrête, c’est l’hébergement du GIF animé : c’est la fonctionnalité que vous payiez.',
  },
  {
    q: 'Le pixel de suivi est-il légal ?',
    a: 'Compter les ouvertures d’un email est un traitement de données personnelles au sens du RGPD, et nous le traitons comme tel. Concrètement : vous pouvez couper la mesure pour toute votre organisation en une case à cocher, nous ne stockons jamais d’adresse IP en clair, l’historique est limité dans le temps selon votre plan, et la suppression d’un compte efface les événements. En tant qu’expéditeur, il vous revient d’informer vos destinataires dans votre propre politique de confidentialité.',
    more: { to: '/legal/confidentialite', label: 'Ce que nous mesurons exactement' },
  },
];

export default function Landing() {
  usePageMeta(
    'Siglair — générateur de signature email professionnelle animée',
    'Créez une signature email professionnelle, animée et hébergée. Siglair centralise vos signatures Gmail, Outlook et Apple Mail avec mise à jour sans copier-coller, repli Outlook et analytics.',
  );

  return (
    <div className={s.page}>
      <SiteHeader />

      <main id="contenu" className={s.main}>
        {/*
          ----------------------------------------------------------- héros
          La promesse du contrat §6bis : une barre d'URL, un bouton, rien d'autre au-dessus de
          la ligne de flottaison. Le champ est fonctionnel sans compte — voir sa propre marque
          apparaître avant de s'inscrire, c'est le moment qui convertit (DESIGN.md).
          L'ancienne accroche reste, en sous-titre.
        */}
        <section className={`${s.wrap} ${s.hero}`}>
          <div className={s.heroCenter}>
            <p className={s.eyebrow}>Votre première signature est gratuite</p>
            {/* Le dégradé porte sur un segment sans jambage : `.gradientText` découpe le
                fond sur la boîte de la ligne, et un « g » y perdrait sa descendante. */}
            <h1>
              Collez votre site.
              <br />
              <span className={s.gradientText}>Votre signature email apparaît.</span>
            </h1>
            <p className={s.heroCopy}>
              <strong>Vos emails finissent. Votre marque, non.</strong> Siglair analyse votre
              identité visuelle, compose une signature email professionnelle et animée, puis vous
              laisse tout modifier dans l’éditeur.
            </p>
            <BrandHero />
            <ul className={`${s.micro} ${s.microList} ${s.microCenter}`}>
              <li>Sans compte pour voir votre marque</li>
              <li>Sans carte bancaire</li>
              <li>Gmail, Outlook et Apple Mail</li>
            </ul>
            <p className={`${s.note} ${s.heroNote}`}>
              <Link to={START_HREF}>Partir plutôt d’un modèle vierge →</Link>
            </p>
            <div className={s.heroStage}>
              <SignatureDemo />
            </div>
          </div>
        </section>

        {/* ---------------------------------------------------- problème */}
        <section className={`${s.wrap} ${s.section}`}>
          <div className={s.problem}>
            <div className={s.problemIntro}>
              <div className={s.sectionHead}>
                <span className={s.kicker}>Le problème</span>
                <h2>Votre signature email devrait inspirer confiance. Trop souvent, elle brouille la marque.</h2>
              </div>
              <ul className={s.problemList}>
                <li>
                  Le logo évolue, une personne change de poste, la campagne du trimestre se
                  termine : la signature est périmée le jour même.
                </li>
                <li>
                  Corriger, c’est renvoyer un bloc de HTML à toute l’équipe et espérer que chacun
                  ouvre les réglages de son client mail. Personne ne le fait.
                </li>
                <li>
                  Six mois plus tard, dix personnes envoient dix signatures différentes, dont trois
                  avec l’ancien logo.
                </li>
              </ul>
            </div>
            <div className={`${s.card} ${s.answer}`}>
              <span className={s.kicker}>La réponse</span>
              <h3 className={s.answerTitle}>On héberge la signature, pas vous.</h3>
              <p>
                Le bloc collé dans le client mail n’est qu’une <strong>image pointant vers une
                URL</strong>. Cette URL, c’est nous. Modifier la signature revient à republier :
                aucun collaborateur n’a rien à refaire, jamais.
              </p>
            </div>
            <div className={s.mailCompare} aria-label="Comparaison avant et avec Siglair">
              <div className={`${s.mailCard} ${s.mailBefore}`}>
                <div className={s.mailCardHead}>
                  <span className={s.cardLabel}>Avant</span>
                </div>
                <div className={s.fakeMail}>
                  <strong>Jean Dupont</strong>
                  <span>CEO — Example Company</span>
                  <span>+33 6 00 00 00 00</span>
                  <span>contact@example.com</span>
                  <div className={s.fakeIcons} aria-hidden="true">
                    <i />
                    <i />
                    <i />
                  </div>
                </div>
              </div>
              <div className={`${s.mailCard} ${s.mailAfter}`}>
                <div className={s.mailCardHead}>
                  <span className={s.cardLabel}>Avec Siglair</span>
                  <span className={s.motionLive}>Motion live</span>
                </div>
                <div className={s.hostedMail}>
                  <span className={s.hostedLogo} aria-hidden="true" />
                  <div>
                    <strong>Jean Dupont</strong>
                    <span>CEO — Example Company</span>
                    <small>contact@example.com · +33 6 00 00 00 00</small>
                    <div className={s.hostedButtons}>
                      <i>Book a call</i>
                      <i>LinkedIn</i>
                    </div>
                  </div>
                </div>
                <p className={s.animationHint}>Animation subtile, première image prête pour Outlook.</p>
              </div>
            </div>
          </div>
        </section>

        {/* -------------------------------------------------- bénéfices SEO */}
        <section className={`${s.wrap} ${s.section} ${s.story}`}>
          <div className={s.storyCopy}>
            <span className={s.kicker}>Confiance et crédibilité</span>
            <h2>
              Donnez confiance en votre marque à chaque email.
              <span className={s.gradientText}> Même avant le premier clic.</span>
            </h2>
            <p>
              Une signature email professionnelle rassure un prospect, clarifie qui écrit et
              donne un accès direct aux bonnes actions. Siglair transforme ce petit bloc en support
              de marque cohérent, compatible et mesurable.
            </p>
          </div>
          <ol className={s.storySteps}>
            {STORY_STEPS.map((step, index) => (
              <li key={step.title} className={s.storyStep}>
                <span>{index + 1}</span>
                <div>
                  <h3>{step.title}</h3>
                  <p>{step.text}</p>
                </div>
              </li>
            ))}
          </ol>
        </section>

        {/* -------------------------------------------- comment ça marche */}
        <section className={`${s.wrap} ${s.section}`}>
          <div className={s.sectionHead}>
            <span className={s.kicker}>Comment ça marche</span>
            <h2>Trois étapes, dont une seule à refaire.</h2>
            <p className={s.lead}>
              Composer et publier, autant de fois que vous voulez. Coller : une fois pour toutes.
            </p>
          </div>

          <ol className={`${s.grid3} ${s.plainList}`}>
            <li className={`${s.card} ${s.step}`}>
              <div className={s.stepArt} aria-hidden="true">
                <div className={s.artBlocks}>
                  <span />
                  <span />
                  <span />
                </div>
              </div>
              <span className={s.stepNum}>1</span>
              <h3>Composer</h3>
              <p>
                Vous partez d’un modèle et vous déplacez ce que vous voulez : texte, logo, boutons,
                bannière. Chaque élément peut recevoir une animation, ou aucune.
              </p>
            </li>
            <li className={`${s.card} ${s.step}`}>
              <div className={s.stepArt} aria-hidden="true">
                <p className={s.artUrl}>{PUBLIC_HOST}/s/atelier-nord.gif</p>
              </div>
              <span className={s.stepNum}>2</span>
              <h3>Publier</h3>
              <p>
                Nos serveurs rendent le GIF et la première image fixe, puis les servent sur une URL
                stable. Republier ne change jamais cette URL.
              </p>
            </li>
            <li className={`${s.card} ${s.step}`}>
              <div className={s.stepArt} aria-hidden="true">
                <div className={s.artPaste}>
                  <span>Gmail</span>
                  <span>Apple Mail</span>
                  <span>Outlook</span>
                </div>
              </div>
              <span className={s.stepNum}>3</span>
              <h3>Coller une fois</h3>
              <p>
                Vous copiez le bloc dans les réglages de votre client mail. C’est la dernière fois
                que vous y touchez.
              </p>
            </li>
          </ol>

          <pre className={s.code}>
            <code>
              {'<a href="https://'}
              {PUBLIC_HOST}
              {'/c/atelier-nord/site">\n  <img src="https://'}
              {PUBLIC_HOST}
              {'/s/atelier-nord.gif" width="620" alt="Camille Roussel — Atelier Nord">\n</a>'}
            </code>
          </pre>
          <p className={s.note}>
            Voilà tout ce qui est collé dans votre client mail. Rien à réinstaller, rien à
            redéployer : le contenu de cette URL change quand vous republiez.
          </p>
        </section>

        {/* ---------------------------------------------- fonctionnalités */}
        <section id="fonctionnalites" className={`${s.wrap} ${s.section}`}>
          <div className={s.sectionHead}>
            <span className={s.kicker}>Ce qui compte vraiment</span>
            <h2>Les bénéfices d’une signature email professionnelle, sans usine à gaz.</h2>
          </div>
          <div className={s.grid3}>
            {FEATURES.map((f) => (
              <div className={`${s.card} ${s.featureCard}`} key={f.title}>
                <div className={s.icon} aria-hidden="true">
                  {f.icon}
                </div>
                <h3>{f.title}</h3>
                <p>{f.text}</p>
              </div>
            ))}
          </div>
        </section>

        {/* ---------------------------------------------- compatibilité */}
        <section id="compatibilite" className={`${s.wrap} ${s.section}`}>
          <div className={s.sectionHead}>
            <span className={s.kicker}>Compatibilité</span>
            <h2>Ce qui s’anime, et ce qui ne s’anime pas.</h2>
            <p className={s.lead}>
              Nous préférons vous le dire ici plutôt que vous laisser le découvrir dans l’Outlook
              d’un client.
            </p>
          </div>
          <div className={s.grid4}>
            {CLIENTS.map((c) => (
              <div className={`${s.card} ${s.compatCard}`} key={c.name}>
                <div className={s.compat}>
                  <div>
                    <strong>{c.name}</strong>
                    <small>{c.note}</small>
                  </div>
                  <span className={`${s.dot} ${c.warn ? s.dotWarn : ''}`} aria-hidden="true" />
                </div>
              </div>
            ))}
          </div>
          <p className={s.note}>
            L’application Outlook pour Windows utilise le moteur de rendu de Word : elle affiche la
            première image d’un GIF et n’anime pas. C’est une limite du client mail, pas un réglage
            que quiconque peut contourner. Siglair produit cette première image en même temps que
            l’animation, pour qu’elle soit lisible et cliquable telle quelle.
          </p>
        </section>

        {/* ---------------------------------------------------- tarifs */}
        <section id="tarifs" className={`${s.wrap} ${s.section}`}>
          <div className={s.sectionHead}>
            <span className={s.kicker}>Tarifs</span>
            <h2>Commencez gratuitement. Payez quand la signature travaille.</h2>
            <p className={s.lead}>
              Le plan gratuit crée une signature complète, animée dans l’éditeur, exportable en
              HTML. Ce qui se paie, c’est l’URL hébergée.
            </p>
          </div>
          <PlanCards />
          <p className={s.note}>
            <Link to="/pricing">Comparer les plans en détail →</Link>
          </p>
        </section>

        {/* ------------------------------------------------------- FAQ */}
        <section id="faq" className={`${s.wrap} ${s.section}`}>
          <div className={s.sectionHead}>
            <span className={s.kicker}>Questions fréquentes</span>
            <h2>Les questions avant de cliquer.</h2>
          </div>
          <Faq items={FAQ_ITEMS} />
        </section>

        {/* -------------------------------------------------- CTA final */}
        <section className={`${s.wrap} ${s.section}`}>
          <div className={s.final}>
            <p className={s.eyebrow}>1 email. Puis 10. Puis 1 000.</p>
            <h2>Votre prochaine preuve de sérieux est en bas de votre prochain email.</h2>
            <p>
              Créez une signature email professionnelle, collez-la une fois dans Gmail, Outlook ou
              Apple Mail, puis mettez-la à jour depuis Siglair quand votre marque évolue.
            </p>
            <div className={s.actions}>
              <Link className={`${s.btn} ${s.btnPrimary} ${s.btnBig}`} to={START_HREF}>
                Créer ma signature — gratuit
              </Link>
              <Link className={`${s.btn} ${s.btnBig}`} to="/pricing">
                Voir les tarifs
              </Link>
            </div>
          </div>
        </section>
      </main>

      <SiteFooter />
    </div>
  );
}
