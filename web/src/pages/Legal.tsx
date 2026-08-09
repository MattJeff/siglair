/**
 * Pages légales — /legal/mentions, /legal/confidentialite, /legal/cgu.
 *
 * ┌──────────────────────────────────────────────────────────────────────────┐
 * │ À COMPLÉTER PAR L'ÉDITEUR AVANT L'OUVERTURE COMMERCIALE :                │
 * │   • raison sociale, forme juridique, capital social                      │
 * │   • adresse du siège social                                              │
 * │   • SIRET / SIREN, RCS, numéro de TVA intracommunautaire                 │
 * │   • directeur de la publication                                          │
 * │   • adresse email et téléphone de contact                                │
 * │   • hébergeur : raison sociale, adresse, téléphone, pays des serveurs    │
 * │   • DPO (délégué à la protection des données) si désigné                 │
 * │   • médiateur de la consommation, si des particuliers sont clients       │
 * │   • ressort du tribunal compétent                                        │
 * │                                                                          │
 * │ Ces emplacements sont rendus à l'écran avec <Todo> : ils sont visibles,  │
 * │ pas seulement en commentaire. Un SIRET inventé serait pire que pas de    │
 * │ mentions légales du tout.                                                │
 * │                                                                          │
 * │ Ces textes sont un modèle de travail sérieux, pas un avis juridique :    │
 * │ ils DOIVENT être relus par un juriste avant la première vente.           │
 * └──────────────────────────────────────────────────────────────────────────┘
 */
import type { ReactNode } from 'react';
import { Link, useParams } from 'react-router-dom';
import { SiteFooter, SiteHeader, usePageMeta } from '../components/marketing/Chrome';
import s from './marketing.module.css';

const SLUGS = ['mentions', 'confidentialite', 'cgu'] as const;
type Slug = (typeof SLUGS)[number];

const TITLES: Record<Slug, string> = {
  mentions: 'Mentions légales',
  confidentialite: 'Politique de confidentialité',
  cgu: 'Conditions générales d’utilisation et de vente',
};

const DESCRIPTIONS: Record<Slug, string> = {
  mentions: 'Identification de l’éditeur du service Siglair et de son hébergeur.',
  confidentialite:
    'Quelles données Siglair traite, pourquoi, combien de temps, avec qui, et comment exercer vos droits.',
  cgu: 'Conditions d’utilisation du service Siglair, abonnements, résiliation et responsabilité.',
};

const isSlug = (v: string): v is Slug => (SLUGS as readonly string[]).includes(v);

/** Emplacement que l'éditeur doit renseigner. Volontairement voyant. */
function Todo({ children }: { children: ReactNode }) {
  return <mark className={s.todo}>À compléter : {children}</mark>;
}

function Updated() {
  return (
    <p className={s.updated}>
      Version&nbsp;: brouillon de travail. Dernière mise à jour&nbsp;: <Todo>date de mise en ligne</Todo>
    </p>
  );
}

function Disclaimer() {
  return (
    <aside className={s.warning} role="note">
      <strong>Document à faire relire.</strong> Ce texte est un modèle rédigé au plus près du
      fonctionnement réel du service. Il doit être relu et validé par un juriste avant l’ouverture
      commerciale, et les emplacements signalés en jaune doivent être renseignés.
    </aside>
  );
}

/* ------------------------------------------------------------------ */
/* Mentions légales — LCEN art. 6-III                                  */
/* ------------------------------------------------------------------ */

function Mentions() {
  return (
    <>
      <h2 id="editeur">Éditeur du service</h2>
      <dl>
        <dt>Raison sociale et forme juridique</dt>
        <dd>
          <Todo>dénomination sociale et forme (SAS, SASU, EI…)</Todo>
        </dd>
        <dt>Capital social</dt>
        <dd>
          <Todo>montant du capital, si société</Todo>
        </dd>
        <dt>Siège social</dt>
        <dd>
          <Todo>adresse postale complète</Todo>
        </dd>
        <dt>Immatriculation</dt>
        <dd>
          <Todo>SIREN / SIRET, ville du RCS</Todo>
        </dd>
        <dt>Numéro de TVA intracommunautaire</dt>
        <dd>
          <Todo>numéro de TVA, ou mention « non assujetti »</Todo>
        </dd>
        <dt>Directeur de la publication</dt>
        <dd>
          <Todo>nom et prénom du représentant légal</Todo>
        </dd>
      </dl>

      <h2 id="contact">Nous contacter</h2>
      <dl>
        <dt>Adresse électronique</dt>
        <dd>
          <Todo>adresse email de contact</Todo>
        </dd>
        <dt>Téléphone</dt>
        <dd>
          <Todo>numéro de téléphone</Todo>
        </dd>
      </dl>
      <p>
        Pour toute question relative à vos données personnelles, voir la{' '}
        <Link to="/legal/confidentialite">politique de confidentialité</Link>.
      </p>

      <h2 id="hebergeur">Hébergeur</h2>
      <dl>
        <dt>Raison sociale</dt>
        <dd>
          <Todo>nom de l’hébergeur</Todo>
        </dd>
        <dt>Adresse</dt>
        <dd>
          <Todo>adresse postale de l’hébergeur</Todo>
        </dd>
        <dt>Téléphone</dt>
        <dd>
          <Todo>téléphone de l’hébergeur</Todo>
        </dd>
        <dt>Localisation des serveurs</dt>
        <dd>
          <Todo>pays d’hébergement des données</Todo>
        </dd>
      </dl>

      <h2 id="propriete">Propriété intellectuelle</h2>
      <p>
        La marque Siglair, le nom de domaine, l’interface, les modèles fournis, le code source et
        la documentation sont protégés. Toute reproduction ou réutilisation, totale ou partielle,
        sans autorisation écrite est interdite.
      </p>
      <p>
        <strong>Vos contenus restent les vôtres.</strong> Les textes, logos, images et signatures
        que vous importez ou composez demeurent votre propriété ou celle de leurs ayants droit.
        Vous nous accordez uniquement le droit technique de les stocker, de les transformer (rendu
        du GIF et de l’image fixe) et de les diffuser depuis nos URL publiques, pour la seule
        exécution du service et pour la durée de votre compte.
      </p>

      <h2 id="signalement">Signaler un contenu</h2>
      <p>
        Une signature publiée par un utilisateur vous semble abusive, trompeuse ou contrefaisante&nbsp;?
        Écrivez-nous à l’adresse ci-dessus en indiquant l’URL concernée. Nous traitons les
        signalements et pouvons suspendre une signature publiée sans préavis.
      </p>

      <h2 id="mediation">Médiation de la consommation</h2>
      <p>
        Si le service est vendu à des particuliers, l’éditeur doit adhérer à un dispositif de
        médiation de la consommation et en indiquer ici les coordonnées&nbsp;:{' '}
        <Todo>nom et coordonnées du médiateur, ou mention « offre réservée aux professionnels »</Todo>
      </p>

      <h2 id="droit">Droit applicable</h2>
      <p>
        Le présent site est soumis au droit français. En cas de litige et à défaut de résolution
        amiable, compétence est attribuée aux tribunaux du ressort de{' '}
        <Todo>ressort du tribunal compétent</Todo>, sous réserve des règles impératives applicables
        aux consommateurs.
      </p>
    </>
  );
}

/* ------------------------------------------------------------------ */
/* Confidentialité — RGPD                                              */
/* ------------------------------------------------------------------ */

function Privacy() {
  return (
    <>
      <p>
        Cette page décrit ce que Siglair fait des données personnelles, dans le détail et sans
        formule creuse. Elle couvre deux situations différentes&nbsp;: vos données de client (vous
        qui utilisez le service) et les données de mesure d’audience des emails que vous envoyez à
        vos destinataires.
      </p>

      <h2 id="responsable">Qui est responsable ?</h2>
      <p>
        Le responsable du traitement de vos données de client est l’éditeur du service&nbsp;:{' '}
        <Todo>raison sociale et adresse (identiques aux mentions légales)</Todo>
      </p>
      <p>
        Délégué à la protection des données&nbsp;:{' '}
        <Todo>coordonnées du DPO, ou mention « aucun DPO désigné » avec le contact référent</Todo>
      </p>
      <p>
        <strong>Cas particulier de la mesure d’audience des emails.</strong> Lorsque vous publiez
        une signature et que vos destinataires ouvrent vos emails, c’est <em>vous</em> (ou votre
        organisation) qui décidez de cette mesure et de sa finalité&nbsp;: vous en êtes le
        responsable de traitement. Siglair agit comme <strong>sous-traitant</strong> et n’exploite
        ces événements pour aucune autre finalité que de vous les restituer.
      </p>

      <h2 id="donnees">Quelles données, pour quoi faire</h2>

      <h3>Compte et authentification</h3>
      <p>
        Adresse email, nom et image de profil s’ils sont fournis par Google ou Apple lors d’une
        connexion, date de création et date de dernière visite, et une empreinte de votre adresse
        IP pour vos sessions. <strong>Base légale&nbsp;: exécution du contrat.</strong> Les liens
        de connexion par email sont stockés hachés, valables quinze minutes et à usage unique.
      </p>

      <h3>Contenu que vous créez</h3>
      <p>
        Signatures, documents de mise en page, images et GIF importés, profils de membres (nom,
        fonction, email, téléphone, liens) utilisés pour remplir les modèles d’équipe.{' '}
        <strong>Base légale&nbsp;: exécution du contrat.</strong> Si ces profils contiennent les
        données de vos collaborateurs, c’est vous qui en êtes responsable&nbsp;; nous les
        conservons pour vous.
      </p>

      <h3>Mesure d’ouverture et de clics</h3>
      <p>
        Une signature publiée est servie depuis nos serveurs sous forme d’image. Chaque affichage
        de cette image est un <strong>traitement de données personnelles</strong>, et nous le
        traitons comme tel. Pour chaque événement nous enregistrons&nbsp;: la date et l’heure, le
        type d’événement (ouverture ou clic), l’identifiant de l’élément cliqué, une{' '}
        <strong>empreinte tronquée et salée de l’adresse IP</strong> (jamais l’adresse elle-même,
        et l’opération n’est pas réversible) et une <strong>famille de client mail</strong> déduite
        (par exemple «&nbsp;gmail&nbsp;», «&nbsp;outlook&nbsp;»), jamais l’en-tête technique
        complet du navigateur.
      </p>
      <p>
        <strong>Base légale&nbsp;: intérêt légitime du responsable de traitement</strong>, c’est-à-dire
        le vôtre, à mesurer l’efficacité de sa communication. Il vous appartient d’en informer vos
        destinataires dans votre propre politique de confidentialité et de recueillir, le cas
        échéant, leur consentement lorsque la réglementation applicable l’exige.
      </p>
      <p>
        <strong>Vous pouvez tout couper.</strong> Une option dans les réglages de l’organisation
        désactive entièrement l’enregistrement des ouvertures et des clics. Les signatures
        continuent de fonctionner à l’identique&nbsp;; plus aucun événement n’est enregistré.
      </p>

      <h3>Facturation</h3>
      <p>
        Plan souscrit, nombre de sièges, statut et échéance de l’abonnement, identifiants client et
        abonnement chez notre prestataire de paiement.{' '}
        <strong>Base légale&nbsp;: exécution du contrat et obligation légale</strong> (conservation
        comptable). Nous ne recevons ni ne stockons aucun numéro de carte bancaire.
      </p>

      <h3>Journaux techniques</h3>
      <p>
        Journaux d’erreurs et de disponibilité, nécessaires à la sécurité et au bon fonctionnement
        du service. <strong>Base légale&nbsp;: intérêt légitime.</strong>
      </p>

      <h2 id="durees">Combien de temps</h2>
      <ul>
        <li>
          <strong>Compte&nbsp;:</strong> tant que le compte existe, puis suppression à la
          fermeture.
        </li>
        <li>
          <strong>Événements d’ouverture et de clic&nbsp;:</strong> selon votre plan, l’historique
          consultable est limité dans le temps (voir la page <Link to="/pricing">Tarifs</Link>).
          Au-delà, les événements sont supprimés.
        </li>
        <li>
          <strong>Liens de connexion&nbsp;:</strong> quinze minutes, usage unique.
        </li>
        <li>
          <strong>Sessions&nbsp;:</strong> jusqu’à expiration ou déconnexion.
        </li>
        <li>
          <strong>Pièces comptables&nbsp;:</strong> durée légale de conservation applicable.
        </li>
      </ul>
      <p>
        <strong>La suppression d’un compte purge les événements associés.</strong> Ce n’est pas une
        promesse d’intention&nbsp;: c’est le comportement du produit.
      </p>

      <h2 id="sous-traitants">Avec qui nous travaillons</h2>
      <p>
        Nous ne vendons aucune donnée et ne faisons aucune publicité. Nous faisons appel aux
        prestataires suivants, chacun pour une finalité précise&nbsp;:
      </p>
      <ul>
        <li>
          <strong>Stripe</strong> — paiement et facturation.
        </li>
        <li>
          <strong>Resend</strong> — envoi des emails transactionnels (lien de connexion,
          invitations).
        </li>
        <li>
          <strong>Google</strong> et <strong>Apple</strong> — connexion par compte tiers,
          uniquement si vous choisissez ce mode de connexion.
        </li>
        <li>
          <strong>Anthropic</strong> — assistance à la composition d’une signature à partir de
          votre site, uniquement lorsque vous utilisez cette fonctionnalité.
        </li>
        <li>
          <strong>Hébergeur&nbsp;:</strong> <Todo>nom, pays et localisation des serveurs</Todo>
        </li>
      </ul>
      <p>
        Transferts hors Union européenne&nbsp;:{' '}
        <Todo>
          liste des prestataires concernés et garanties applicables (clauses contractuelles types,
          décision d’adéquation)
        </Todo>
      </p>

      <h2 id="cookies">Cookies et traceurs</h2>
      <p>
        Le site n’utilise <strong>aucun cookie publicitaire ni aucune mesure d’audience tierce</strong>.
        Un seul cookie est déposé, strictement nécessaire au fonctionnement&nbsp;: le cookie de
        session, qui vous maintient connecté. Il est <code>HttpOnly</code>, <code>Secure</code> et
        limité à notre domaine. Aucune police d’écriture n’est chargée depuis un serveur tiers.
      </p>

      <h2 id="securite">Sécurité</h2>
      <p>
        Connexions chiffrées, mots de passe inexistants (connexion par lien ou par compte tiers,
        donc rien à voler), jetons de connexion stockés hachés, sessions révocables
        immédiatement, adresses IP jamais conservées en clair, contrôle des URL distantes pour
        éviter qu’un contenu importé ne serve à sonder notre réseau.
      </p>

      <h2 id="droits">Vos droits</h2>
      <p>
        Vous disposez d’un droit d’accès, de rectification, d’effacement, de limitation, de
        portabilité et d’opposition sur vos données. Vous pouvez également définir des directives
        relatives à leur sort après votre décès.
      </p>
      <p>
        Pour les exercer, écrivez à <Todo>adresse email dédiée aux demandes RGPD</Todo>. Nous
        répondons dans un délai d’un mois. Si vos destinataires souhaitent exercer leurs droits sur
        les événements d’ouverture, ils doivent s’adresser à l’expéditeur de l’email&nbsp;: c’est
        lui le responsable de ce traitement, et nous n’avons aucun moyen de relier une empreinte
        d’adresse IP à une personne.
      </p>
      <p>
        Vous pouvez enfin introduire une réclamation auprès de la CNIL (<a
          href="https://www.cnil.fr"
          rel="noreferrer noopener"
          target="_blank"
        >
          cnil.fr
        </a>
        ).
      </p>

      <h2 id="modifications">Modifications</h2>
      <p>
        Toute évolution substantielle de cette politique vous sera signalée par email ou dans
        l’application avant son entrée en vigueur.
      </p>
    </>
  );
}

/* ------------------------------------------------------------------ */
/* CGU / CGV                                                           */
/* ------------------------------------------------------------------ */

function Terms() {
  return (
    <>
      <h2 id="objet">1. Objet</h2>
      <p>
        Les présentes conditions régissent l’utilisation du service Siglair, qui permet de
        composer des signatures email, de les faire rendre en image animée par nos serveurs, de les
        héberger sur une URL stable et d’en mesurer les ouvertures et les clics. Créer un compte
        vaut acceptation de ces conditions.
      </p>

      <h2 id="compte">2. Compte</h2>
      <p>
        L’accès nécessite un compte, créé par lien de connexion envoyé par email ou via un compte
        Google ou Apple. Vous êtes responsable de l’accès à votre boîte email et des actions
        réalisées depuis votre compte. Un compte est personnel&nbsp;; le partage d’identifiants
        entre plusieurs personnes n’est pas autorisé — c’est à cela que servent les membres d’une
        organisation.
      </p>

      <h2 id="service">3. Ce que fait le service, et ce qu’il ne fait pas</h2>
      <p>
        Une signature publiée est servie sous forme d’image animée, accompagnée d’une image fixe
        correspondant à sa première frame. <strong>Les clients de messagerie n’ont pas tous le
        même comportement&nbsp;:</strong> certains, dont l’application Outlook pour Windows,
        affichent uniquement l’image fixe. C’est une limite des logiciels de messagerie, connue,
        annoncée, et sur laquelle nous n’avons aucune prise. Elle ne peut donner lieu ni à
        remboursement ni à réclamation.
      </p>
      <p>
        Les liens présents dans une signature passent par nos URL de redirection afin de compter
        les clics. La cible d’une redirection est toujours celle enregistrée dans la signature
        publiée&nbsp;: le service ne peut pas être détourné en redirecteur ouvert.
      </p>

      <h2 id="usage">4. Ce que vous vous engagez à ne pas faire</h2>
      <ul>
        <li>
          Usurper l’identité d’une personne ou d’une organisation, ou diffuser une signature
          trompeuse&nbsp;;
        </li>
        <li>utiliser le service dans une campagne d’hameçonnage, de spam ou de logiciel malveillant&nbsp;;</li>
        <li>
          importer un contenu sur lequel vous n’avez pas les droits, ou un contenu illicite,
          haineux ou pornographique&nbsp;;
        </li>
        <li>
          tenter de contourner les quotas, de sonder notre infrastructure ou de dégrader le
          service&nbsp;;
        </li>
        <li>
          revendre ou redistribuer le service en tant que tel sans accord écrit (l’usage en agence
          pour le compte de vos clients, lui, est prévu et autorisé).
        </li>
      </ul>
      <p>
        En cas de manquement, nous pouvons suspendre une signature publiée ou un compte sans
        préavis, et résilier après mise en demeure restée sans effet.
      </p>

      <h2 id="prix">5. Plans, prix et paiement</h2>
      <p>
        Les plans, leurs prix et leurs quotas sont ceux affichés sur la page{' '}
        <Link to="/pricing">Tarifs</Link> au moment de la souscription. Le paiement est traité par
        Stripe. Les abonnements sont reconduits automatiquement à échéance, mensuellement ou
        annuellement selon la formule choisie, jusqu’à résiliation.
      </p>
      <p>
        Le plan Team est facturé par membre, avec un nombre minimum de membres indiqué sur la page
        Tarifs. Les évolutions de prix sont annoncées au moins trente jours à l’avance et ne
        s’appliquent qu’à la période suivante.
      </p>

      <h2 id="retractation">6. Rétractation</h2>
      <p>
        Si vous êtes un consommateur au sens du code de la consommation, vous disposez d’un délai
        de quatorze jours pour vous rétracter. En souscrivant, vous demandez l’exécution immédiate
        du service et reconnaissez perdre ce droit une fois le service pleinement exécuté&nbsp;;
        en cas de rétractation en cours de période, le montant dû est calculé au prorata de
        l’usage.
      </p>

      <h2 id="resiliation">7. Résiliation et fin d’abonnement</h2>
      <p>
        Vous résiliez à tout moment depuis votre espace de facturation. Le service payant reste
        actif jusqu’à la fin de la période déjà réglée, puis l’organisation repasse au plan
        gratuit.
      </p>
      <p>
        <strong>Conséquence concrète&nbsp;:</strong> vos signatures, vos images et vos réglages
        restent accessibles et exportables en HTML statique, mais l’hébergement du GIF animé sur
        une URL cesse. Les signatures déjà collées dans les clients mail de vos destinataires
        n’affichent plus l’image hébergée.
      </p>
      <p>
        La suppression de votre compte entraîne la suppression de vos données, y compris des
        événements d’ouverture et de clic, sous réserve des durées de conservation légales
        applicables aux pièces comptables.
      </p>

      <h2 id="disponibilite">8. Disponibilité et responsabilité</h2>
      <p>
        Nous mettons en œuvre les moyens raisonnables pour assurer la disponibilité du service,
        sans garantie d’absence totale d’interruption&nbsp;: des maintenances, des incidents et des
        défaillances de prestataires tiers peuvent survenir. Notre responsabilité, hors faute
        lourde ou dolosive, est limitée aux sommes que vous avez effectivement versées au titre des
        douze derniers mois. Nous ne sommes pas responsables des dommages indirects, notamment
        d’une perte d’exploitation ou d’image.
      </p>

      <h2 id="donnees-perso">9. Données personnelles</h2>
      <p>
        Le traitement des données personnelles est décrit dans la{' '}
        <Link to="/legal/confidentialite">politique de confidentialité</Link>, qui fait partie intégrante
        des présentes conditions. Vous êtes responsable de l’information de vos destinataires quant
        à la mesure d’ouverture de vos emails, que vous pouvez désactiver entièrement.
      </p>

      <h2 id="evolution">10. Évolution des conditions</h2>
      <p>
        Ces conditions peuvent évoluer. Toute modification substantielle est notifiée au moins
        trente jours avant son entrée en vigueur&nbsp;; la poursuite de l’utilisation vaut
        acceptation.
      </p>

      <h2 id="droit-applicable">11. Droit applicable et litiges</h2>
      <p>
        Les présentes conditions sont soumises au droit français. En cas de litige, les parties
        rechercheront une solution amiable avant toute action. À défaut, compétence est attribuée
        aux tribunaux du ressort de <Todo>ressort du tribunal compétent</Todo>, sous réserve des
        règles impératives protégeant les consommateurs.
      </p>
    </>
  );
}

const BODIES: Record<Slug, () => ReactNode> = {
  mentions: Mentions,
  confidentialite: Privacy,
  cgu: Terms,
};

export default function Legal() {
  // App.tsx monte /legal/* : le reste du chemin est dans le paramètre '*'.
  const rest = (useParams()['*'] ?? '').replace(/\/+$/, '');
  const slug: Slug = isSlug(rest) ? rest : 'mentions';
  const Body = BODIES[slug];

  usePageMeta(`${TITLES[slug]} — Siglair`, DESCRIPTIONS[slug]);

  return (
    <div className={s.page}>
      <SiteHeader />

      <main id="contenu" className={s.main}>
        <article className={`${s.wrap} ${s.legal}`}>
          <h1>{TITLES[slug]}</h1>
          <Updated />

          <nav aria-label="Pages légales">
            <ul className={s.legalNav}>
              {SLUGS.map((k) => (
                <li key={k}>
                  <Link to={`/legal/${k}`} aria-current={k === slug ? 'page' : undefined}>
                    {TITLES[k]}
                  </Link>
                </li>
              ))}
            </ul>
          </nav>

          <Disclaimer />
          <Body />
        </article>
      </main>

      <SiteFooter />
    </div>
  );
}
