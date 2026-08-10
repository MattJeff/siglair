import { Link, useLocation } from 'react-router-dom';
import { START_HREF, SiteFooter, SiteHeader, usePageMeta } from '../components/marketing/Chrome';
import s from './seo-pages.module.css';

type PageKind = 'campaign' | 'generator' | 'team' | 'motion' | 'outlook' | 'gmail';
type SeoPath =
  | '/campagnes-signature-email'
  | '/generateur-signature-email'
  | '/gestion-signatures-email-entreprise'
  | '/signature-email-animee'
  | '/signature-email-outlook'
  | '/signature-email-gmail';

type TextBlock = {
  title: string;
  text: string;
};

type FaqItem = {
  question: string;
  answer: string;
};

type OfficialLink = {
  href: string;
  label: string;
};

type SeoPageConfig = {
  kind: PageKind;
  metaTitle: string;
  metaDescription: string;
  eyebrow: string;
  title: string;
  accent: string;
  directAnswer: string;
  heroPoints: string[];
  preview: {
    app: string;
    status: string;
    name: string;
    role: string;
    message: string;
    primaryCta: string;
    secondaryCta: string;
  };
  benefitTitle: string;
  benefits: TextBlock[];
  methodTitle: string;
  methodIntro: string;
  steps: TextBlock[];
  limitTitle: string;
  limitText: string;
  officialLinks: OfficialLink[];
  useCaseTitle: string;
  useCases: TextBlock[];
  faq: FaqItem[];
};

const GMAIL_SIGNATURE_HELP = 'https://support.google.com/mail/answer/8395?hl=fr';
const GMAIL_TROUBLESHOOTING = 'https://support.google.com/mail/answer/11468381?hl=fr';
const OUTLOOK_SIGNATURE_HELP =
  'https://support.microsoft.com/fr-fr/office/cr%C3%A9er-et-ajouter-une-signature-%C3%A9lectronique-dans-outlook-8ee5d4f4-68fd-464a-a1c1-0e1c80bb27f2';

const PAGES: Record<SeoPath, SeoPageConfig> = {
  '/campagnes-signature-email': {
    kind: 'campaign',
    metaTitle: 'Campagnes de signature email et bannières | Siglair',
    metaDescription:
      'Transformez vos signatures email en actif marketing : bannière, CTA, calendrier de campagne, republication sur URL stable et clics mesurés séparément.',
    eyebrow: 'Campagnes de signature email',
    title: 'Lancez une campagne sous chaque conversation.',
    accent: 'Sans nouvelle installation.',
    directAnswer:
      'Une campagne de signature email ajoute un message, une bannière ou un CTA sous les coordonnées de vos équipes. Dans Siglair, vous programmez sa période : le rendu hébergé l’active puis la retire automatiquement, sans changer son URL, et chaque bouton conserve sa propre mesure de clics.',
    heroPoints: ['URL stable', 'CTA mesurés séparément', 'Activation automatique'],
    preview: {
      app: 'Campagne active',
      status: 'Publiée',
      name: 'Camille Martin',
      role: 'Direction commerciale · Siglair',
      message: 'Nouveau guide : transformer chaque email en point de conversion.',
      primaryCta: 'Lire le guide',
      secondaryCta: 'Prendre rendez-vous',
    },
    benefitTitle: 'Un espace marketing déjà présent dans vos emails.',
    benefits: [
      {
        title: 'Diffusez un temps fort',
        text: 'Webinar, lancement, contenu, recrutement ou rendez-vous : la campagne apparaît dans les échanges déjà ouverts par vos équipes.',
      },
      {
        title: 'Republiez sur la même URL',
        text: 'La signature installée ne change pas d’adresse. À l’ouverture et à la fin de la période, Siglair republie automatiquement le rendu attendu.',
      },
      {
        title: 'Lisez les bons signaux',
        text: 'La bannière, le site, LinkedIn et la prise de rendez-vous sont suivis comme des CTA distincts, pour éviter un compteur global difficile à interpréter.',
      },
    ],
    methodTitle: 'De l’idée à une campagne programmée.',
    methodIntro:
      'Une campagne utile reste courte, liée à une intention précise et lisible même lorsque le mouvement ne se joue pas.',
    steps: [
      {
        title: 'Choisissez un seul objectif',
        text: 'Définissez le clic attendu : s’inscrire, découvrir un lancement, lire un contenu ou réserver un créneau.',
      },
      {
        title: 'Préparez le message et ses dates',
        text: 'Ajoutez une bannière ou un bloc texte, un CTA explicite et une période cohérente avec votre calendrier marketing.',
      },
      {
        title: 'Programmez et laissez Siglair synchroniser',
        text: 'Vérifiez la première image puis programmez la campagne. Siglair publie son ouverture et son retrait sans nouveau bloc HTML à distribuer.',
      },
    ],
    limitTitle: 'Ce que la campagne ne remplace pas.',
    limitText:
      'Une signature n’est ni une newsletter ni un espace publicitaire illimité. Elle accompagne une conversation existante : le message doit rester bref, pertinent et secondaire par rapport à l’email. La mesure des clics indique un intérêt, pas une vente attribuée à elle seule.',
    officialLinks: [],
    useCaseTitle: 'Quatre campagnes faciles à activer.',
    useCases: [
      { title: 'Webinar', text: 'Une date, un bénéfice et un bouton d’inscription.' },
      { title: 'Lancement', text: 'Une proposition claire vers la page du nouveau produit.' },
      { title: 'Contenu', text: 'Un guide ou un cas d’usage lié aux conversations du moment.' },
      { title: 'Rendez-vous', text: 'Un CTA direct vers le calendrier de la bonne personne.' },
    ],
    faq: [
      {
        question: 'Faut-il réinstaller la signature à chaque campagne ?',
        answer:
          'Non. Lorsque la signature utilise l’URL hébergée Siglair, l’ouverture et la fermeture de la campagne sont republiées automatiquement sur cette même URL.',
      },
      {
        question: 'Peut-on suivre plusieurs boutons ?',
        answer:
          'Oui. Les clics sont comptés séparément pour chaque CTA, par exemple la bannière, le site et la prise de rendez-vous.',
      },
      {
        question: 'Une campagne peut-elle être datée ?',
        answer:
          'Oui. Sa date de début déclenche l’activation du contenu et sa date de fin déclenche son retrait. Un balayage serveur synchronise les signatures concernées.',
      },
    ],
  },
  '/generateur-signature-email': {
    kind: 'generator',
    metaTitle: 'Générateur de signature email gratuit | Siglair',
    metaDescription:
      'Créez gratuitement une signature email professionnelle à partir de votre site : logo, couleurs, coordonnées, CTA, aperçu Gmail et Outlook, puis édition complète.',
    eyebrow: 'Générateur de signature email',
    title: 'Collez votre site.',
    accent: 'Repartez avec une vraie signature.',
    directAnswer:
      'Le générateur Siglair analyse les éléments publics de votre site, propose une signature aux couleurs de votre marque et ouvre le résultat dans un éditeur visuel. Vous pouvez modifier chaque bloc avant de publier puis installer la signature dans Gmail ou Outlook.',
    heroPoints: ['Première création gratuite', 'Résultat entièrement modifiable', 'Aucune carte bancaire'],
    preview: {
      app: 'Génération IA',
      status: 'Prête à modifier',
      name: 'Camille Martin',
      role: 'Fondatrice · Atelier Nord',
      message: 'Une identité cohérente, déjà composée à partir du site.',
      primaryCta: 'Voir le site',
      secondaryCta: 'LinkedIn',
    },
    benefitTitle: 'Un point de départ utile, pas un formulaire interminable.',
    benefits: [
      {
        title: 'Votre marque comme matière première',
        text: 'Siglair recherche le nom, le logo, les couleurs et les liens publics disponibles. Une information absente reste modifiable dans l’éditeur.',
      },
      {
        title: 'Une composition déjà exploitable',
        text: 'Le résultat contient une hiérarchie, des coordonnées et des CTA. Vous ne partez pas d’un canvas vide et gardez la main sur chaque élément.',
      },
      {
        title: 'Un rendu fait pour l’email',
        text: 'La signature est publiée comme une ressource hébergée, avec une première image lisible pour les clients qui ne jouent pas l’animation.',
      },
    ],
    methodTitle: 'De votre URL à votre client mail.',
    methodIntro:
      'La génération accélère le premier rendu. La vérification humaine reste indispensable pour les coordonnées, les liens et la compatibilité de votre environnement.',
    steps: [
      {
        title: 'Collez l’adresse publique de votre site',
        text: 'Le serveur analyse uniquement les informations accessibles publiquement. Un site inaccessible peut être remplacé par une création manuelle.',
      },
      {
        title: 'Choisissez puis ajustez la proposition',
        text: 'Corrigez le profil, changez le logo, déplacez les blocs, ajoutez un CTA et adaptez les couleurs ou les animations.',
      },
      {
        title: 'Publiez, installez et envoyez un test',
        text: 'Contrôlez l’aperçu fixe et animé, publiez la signature puis vérifiez-la dans un véritable email Gmail ou Outlook.',
      },
    ],
    limitTitle: 'Ce que l’analyse automatique ne devine pas.',
    limitText:
      'Un site peut bloquer les robots, servir un logo dans un format inutilisable ou ne publier aucun contact. Siglair n’invente pas ces données : vous pouvez continuer sans elles, les ajouter dans l’éditeur et vérifier le résultat avant toute installation.',
    officialLinks: [
      { href: GMAIL_SIGNATURE_HELP, label: 'Consulter l’aide officielle Gmail sur les signatures' },
      { href: OUTLOOK_SIGNATURE_HELP, label: 'Consulter l’aide officielle Outlook sur les signatures' },
    ],
    useCaseTitle: 'Un générateur adapté à quatre départs fréquents.',
    useCases: [
      { title: 'Indépendant', text: 'Nom, activité, site et prise de rendez-vous dans un format compact.' },
      { title: 'Commercial', text: 'Coordonnées, preuve de marque et CTA vers le bon prochain geste.' },
      { title: 'Fondateur', text: 'Identité personnelle, lancement et lien vers le produit.' },
      { title: 'Équipe', text: 'Une première signature qui peut devenir le modèle partagé de l’organisation.' },
    ],
    faq: [
      {
        question: 'Le générateur de signature email est-il gratuit ?',
        answer:
          'Oui. Le plan Free permet de créer et d’héberger une signature avec une discrète mention Siglair. Le plan Pro retire cette mention et ajoute notamment les campagnes et leurs analytics.',
      },
      {
        question: 'Dois-je laisser l’IA choisir le design final ?',
        answer:
          'Non. La proposition ouvre le même éditeur que les créations manuelles : textes, images, tailles, couleurs, liens et animations restent modifiables.',
      },
      {
        question: 'Que se passe-t-il si mon site ne peut pas être analysé ?',
        answer:
          'Vous pouvez continuer avec le nom du domaine ou partir d’un modèle vierge, puis importer votre logo et renseigner les informations manquantes dans l’éditeur.',
      },
    ],
  },
  '/gestion-signatures-email-entreprise': {
    kind: 'team',
    metaTitle: 'Gestion des signatures email d’entreprise | Siglair',
    metaDescription:
      'Centralisez les signatures email de votre entreprise : modèle partagé, identité cohérente, campagnes datées, publication sans réinstallation et mesure par CTA.',
    eyebrow: 'Gestion des signatures email',
    title: 'Une marque cohérente dans chaque boîte mail.',
    accent: 'Un canal piloté par l’équipe marketing.',
    directAnswer:
      'Siglair centralise le modèle, les profils et les campagnes de signature d’une équipe. Chaque signature est hébergée sur une URL stable : le marketing peut republier un message ou une bannière sans demander aux collaborateurs de modifier à nouveau leur client mail.',
    heroPoints: ['Modèles partagés', 'Campagnes d’équipe', 'Historique analytics Team'],
    preview: {
      app: 'Espace Team',
      status: 'Campagne publiée',
      name: 'Camille Martin',
      role: 'Direction commerciale · Atelier Nord',
      message: 'Webinar produit · Jeudi à 11 h · Inscription ouverte.',
      primaryCta: 'Réserver une place',
      secondaryCta: 'Découvrir l’offre',
    },
    benefitTitle: 'La signature d’entreprise devient un système maintenable.',
    benefits: [
      {
        title: 'Réduisez les versions divergentes',
        text: 'Le logo, les couleurs et la structure partent d’un modèle partagé au lieu de circuler dans des fichiers et copier-coller différents.',
      },
      {
        title: 'Diffusez le message du moment',
        text: 'Une campagne peut porter un lancement, un événement, une offre ou un contenu dans les conversations quotidiennes de l’équipe.',
      },
      {
        title: 'Mesurez sans confondre les CTA',
        text: 'Les clics de la bannière, du site et des autres boutons restent séparés, avec un historique adapté au plan Team.',
      },
    ],
    methodTitle: 'Déployer sans transformer chaque collègue en intégrateur.',
    methodIntro:
      'La centralisation ne supprime pas l’étape d’installation initiale. Elle évite surtout les nouvelles interventions à chaque changement de marque ou campagne.',
    steps: [
      {
        title: 'Préparez le modèle de référence',
        text: 'Définissez la structure, les zones personnalisables, les liens et la première image attendue dans vos principaux clients mail.',
      },
      {
        title: 'Invitez l’équipe et complétez les profils',
        text: 'Chaque membre conserve ses coordonnées dans une composition cohérente avec la marque et le modèle de l’organisation.',
      },
      {
        title: 'Programmez puis mesurez les campagnes',
        text: 'Choisissez la période et le CTA, laissez la signature hébergée se mettre à jour, puis comparez les clics par élément et par membre.',
      },
    ],
    limitTitle: 'Une gestion centralisée reste dépendante des clients mail.',
    limitText:
      'Une politique informatique peut bloquer les images distantes ou imposer une méthode d’installation particulière. Testez le modèle sur le parc réel, documentez l’installation initiale et ne présentez pas les ouvertures d’image comme des lectures exactes : les clics sont un signal plus fiable.',
    officialLinks: [
      { href: OUTLOOK_SIGNATURE_HELP, label: 'Consulter la configuration officielle des signatures Outlook' },
      { href: GMAIL_SIGNATURE_HELP, label: 'Consulter la configuration officielle des signatures Gmail' },
    ],
    useCaseTitle: 'Des usages Team reliés au travail réel.',
    useCases: [
      { title: 'Lancement produit', text: 'Un message et un CTA cohérents dans les emails commerciaux.' },
      { title: 'Événement', text: 'Une campagne datée qui cesse lorsque la période se termine.' },
      { title: 'Recrutement', text: 'Une bannière vers les postes ouverts dans les échanges quotidiens.' },
      { title: 'Gouvernance de marque', text: 'Une structure commune même quand les profils et rôles diffèrent.' },
    ],
    faq: [
      {
        question: 'Faut-il réinstaller les signatures à chaque changement ?',
        answer:
          'Non après l’installation initiale de la ressource hébergée. Une republication remplace le rendu servi sur la même URL.',
      },
      {
        question: 'Une campagne peut-elle être appliquée à toute l’équipe ?',
        answer:
          'Le plan Team est conçu pour diffuser une campagne et un modèle partagés dans l’organisation, avec des profils propres à chaque membre.',
      },
      {
        question: 'Comment mesurer la performance des signatures ?',
        answer:
          'Siglair distingue les clics par CTA et par signature. Les ouvertures d’images restent indicatives à cause des préchargements et des protections de confidentialité des clients mail.',
      },
    ],
  },
  '/signature-email-animee': {
    kind: 'motion',
    metaTitle: 'Signature email animée pour Gmail et Outlook | Siglair',
    metaDescription:
      'Créez une signature email animée rendue côté serveur, avec GIF optimisé, image fixe de secours et première frame conçue pour rester lisible.',
    eyebrow: 'Signature email animée',
    title: 'Ajoutez du mouvement.',
    accent: 'Gardez le message lisible partout.',
    directAnswer:
      'Siglair rend votre composition en GIF côté serveur et produit aussi une image fixe. Le mouvement attire l’œil dans les clients compatibles ; une première frame autonome conserve le nom, le rôle et les CTA lorsque le GIF est figé ou bloqué.',
    heroPoints: ['GIF rendu côté serveur', 'Image fixe de secours', 'Première frame autonome'],
    preview: {
      app: 'Aperçu animé',
      status: 'Lecture 6 s',
      name: 'Camille Martin',
      role: 'Fondatrice · Atelier North',
      message: 'Identité en mouvement, informations toujours lisibles.',
      primaryCta: 'Voir le portfolio',
      secondaryCta: 'LinkedIn',
    },
    benefitTitle: 'Le mouvement sert la hiérarchie, pas la décoration.',
    benefits: [
      {
        title: 'Composez visuellement',
        text: 'Position, taille, couleurs, médias, CTA et animations se règlent dans le même éditeur, avec un aperçu du rendu final.',
      },
      {
        title: 'Rendez une seule ressource',
        text: 'Le serveur transforme la scène en GIF et en image fixe : le destinataire n’exécute ni script ni mini-application dans son email.',
      },
      {
        title: 'Préservez l’essentiel',
        text: 'Le premier instant doit déjà montrer l’identité et l’action principale. L’animation enrichit cette base au lieu de la remplacer.',
      },
    ],
    methodTitle: 'Une animation email en trois décisions.',
    methodIntro:
      'La compatibilité commence dans le design. Une scène sobre, courte et autonome au premier instant résiste mieux aux différences entre clients mail.',
    steps: [
      {
        title: 'Construisez d’abord l’image fixe',
        text: 'Placez le nom, la fonction, la marque et le CTA avant d’ajouter le moindre mouvement.',
      },
      {
        title: 'Animez un signal à la fois',
        text: 'Faites apparaître un message ou attirez l’attention sur un CTA sans mettre tout le document en mouvement.',
      },
      {
        title: 'Testez les deux sorties',
        text: 'Contrôlez le GIF et l’image de secours, puis envoyez un email réel vers les clients mail importants pour votre audience.',
      },
    ],
    limitTitle: 'La compatibilité honnête.',
    limitText:
      'Gmail et Outlook sur le web affichent généralement les GIF animés. Certaines applications Outlook pour Windows peuvent rester sur la première frame. Les politiques d’images distantes, les réglages du destinataire et les versions du logiciel peuvent aussi modifier le résultat : testez toujours votre propre parc.',
    officialLinks: [
      { href: OUTLOOK_SIGNATURE_HELP, label: 'Consulter l’aide officielle Outlook sur les signatures' },
      { href: GMAIL_TROUBLESHOOTING, label: 'Consulter le dépannage officiel des signatures Gmail' },
    ],
    useCaseTitle: 'Des mouvements qui ont une fonction.',
    useCases: [
      { title: 'Révélation', text: 'Faire apparaître une proposition après l’identité.' },
      { title: 'Accent CTA', text: 'Donner un signal discret autour du bouton principal.' },
      { title: 'Campagne', text: 'Introduire une bannière sans masquer les coordonnées.' },
      { title: 'Identité', text: 'Animer un motif ou un média fidèle à la marque.' },
    ],
    faq: [
      {
        question: 'Le destinataire doit-il installer quelque chose ?',
        answer:
          'Non. Il reçoit une image GIF ou sa version fixe, hébergée à une URL. Aucun script Siglair ne s’exécute dans le message.',
      },
      {
        question: 'Pourquoi la première frame est-elle importante ?',
        answer:
          'Parce qu’elle peut devenir l’unique image affichée dans un client qui ne joue pas le GIF. Elle doit donc rester complète et compréhensible seule.',
      },
      {
        question: 'L’animation fonctionne-t-elle dans toutes les versions d’Outlook ?',
        answer:
          'Non. Outlook sur le web anime généralement le GIF, tandis que certaines applications Outlook pour Windows peuvent afficher uniquement sa première frame.',
      },
    ],
  },
  '/signature-email-outlook': {
    kind: 'outlook',
    metaTitle: 'Créer une signature Outlook professionnelle | Siglair',
    metaDescription:
      'Créez et installez une signature Outlook professionnelle, avec une première frame lisible lorsque l’application Windows fige le GIF.',
    eyebrow: 'Signature email Outlook',
    title: 'Une signature Outlook professionnelle.',
    accent: 'Animée quand le client le permet.',
    directAnswer:
      'Siglair fournit une signature à installer dans Outlook sous forme de ressource hébergée et cliquable. Outlook sur le web anime généralement le GIF ; certaines applications Outlook pour Windows peuvent n’afficher que sa première frame, conçue pour rester complète et lisible.',
    heroPoints: ['Outlook web animé', 'Première frame Windows', 'Liens cliquables'],
    preview: {
      app: 'Outlook',
      status: 'Première frame prête',
      name: 'Camille Martin',
      role: 'Partnerships · Siglair',
      message: 'Réservez vingt minutes pour parler de votre prochaine campagne.',
      primaryCta: 'Choisir un créneau',
      secondaryCta: 'Voir le site',
    },
    benefitTitle: 'Un design pensé pour les variantes d’Outlook.',
    benefits: [
      {
        title: 'Une base fixe complète',
        text: 'Identité, coordonnées et CTA principal restent présents dès la première frame, même lorsque le mouvement est absent.',
      },
      {
        title: 'Une animation progressive',
        text: 'Sur Outlook web et les environnements compatibles, le GIF peut enrichir la signature sans conditionner sa compréhension.',
      },
      {
        title: 'Une mise à jour centralisée',
        text: 'Après installation de l’URL hébergée, vous modifiez et republiez la signature dans Siglair sans redistribuer un nouveau fichier.',
      },
    ],
    methodTitle: 'Installer puis vérifier dans votre Outlook.',
    methodIntro:
      'Les menus varient entre le nouvel Outlook, Outlook classique et Outlook sur le web. Le principe reste le même : créer une signature, insérer le rendu Siglair, puis choisir quand l’appliquer.',
    steps: [
      {
        title: 'Publiez votre signature Siglair',
        text: 'Finalisez le premier instant, testez les liens, puis utilisez l’export ou l’URL fournie par votre espace.',
      },
      {
        title: 'Ouvrez les réglages de signature',
        text: 'Dans Outlook, accédez aux signatures depuis les paramètres de compte ou le menu Signature d’un nouveau message, selon votre version.',
      },
      {
        title: 'Envoyez un vrai message de test',
        text: 'Vérifiez un nouveau message et une réponse, sur ordinateur et web. Si le GIF est figé, contrôlez que la première frame porte tout l’essentiel.',
      },
    ],
    limitTitle: 'Outlook n’est pas un seul logiciel.',
    limitText:
      'Le nouvel Outlook, Outlook classique, Outlook sur le web et les versions administrées ne rendent pas toujours les images de la même manière. Microsoft indique aussi qu’une signature peut devoir être créée séparément dans Outlook et Outlook sur le web. Siglair ne peut pas neutraliser une politique d’entreprise qui bloque les images distantes.',
    officialLinks: [
      { href: OUTLOOK_SIGNATURE_HELP, label: 'Suivre les instructions officielles de Microsoft pour Outlook' },
    ],
    useCaseTitle: 'Dépannage rapide.',
    useCases: [
      { title: 'GIF immobile', text: 'Validez la première frame : ce comportement peut venir de la version Windows.' },
      { title: 'Image absente', text: 'Vérifiez l’autorisation des images distantes et l’accès public à l’URL.' },
      { title: 'Signature non ajoutée', text: 'Contrôlez les choix pour nouveaux messages, réponses et transferts.' },
      { title: 'Rendu trop large', text: 'Réduisez le canvas et testez à nouveau dans une conversation réelle.' },
    ],
    faq: [
      {
        question: 'Le GIF s’anime-t-il dans Outlook pour Windows ?',
        answer:
          'Cela dépend de la version. Certaines applications Windows peuvent figer le GIF sur sa première frame ; Outlook sur le web l’anime généralement.',
      },
      {
        question: 'Dois-je créer la signature deux fois ?',
        answer:
          'Microsoft précise que, selon le compte et les produits utilisés, Outlook et Outlook sur le web peuvent demander une configuration séparée.',
      },
      {
        question: 'Pourquoi les images ne s’affichent-elles pas ?',
        answer:
          'Une politique d’entreprise ou le réglage du destinataire peut bloquer les images distantes. Vérifiez l’URL, les autorisations d’image et votre configuration Outlook.',
      },
    ],
  },
  '/signature-email-gmail': {
    kind: 'gmail',
    metaTitle: 'Créer une signature Gmail professionnelle | Siglair',
    metaDescription:
      'Créez une signature Gmail professionnelle avec image hébergée, animation généralement prise en charge, CTA cliquables et guide de dépannage.',
    eyebrow: 'Signature email Gmail',
    title: 'Une signature Gmail claire, animée et cliquable.',
    accent: 'Sans casser votre mise en page.',
    directAnswer:
      'Siglair produit une signature hébergée à insérer dans les réglages Gmail. Gmail prend généralement en charge les GIF et les liens ; l’affichage final dépend toutefois du chargement des images distantes, des réglages du compte et du client utilisé par le destinataire.',
    heroPoints: ['GIF généralement animé', 'CTA cliquables', 'Image hébergée'],
    preview: {
      app: 'Gmail',
      status: 'Images chargées',
      name: 'Camille Martin',
      role: 'Marketing · Siglair',
      message: 'Notre prochain atelier est ouvert aux inscriptions.',
      primaryCta: 'S’inscrire',
      secondaryCta: 'Découvrir Siglair',
    },
    benefitTitle: 'Une signature conçue pour la boîte de réception.',
    benefits: [
      {
        title: 'Une identité lisible',
        text: 'Nom, rôle, marque et contact restent regroupés dans un format compact qui accompagne le message sans le dominer.',
      },
      {
        title: 'Un média hébergé',
        text: 'Le GIF et son image fixe sont servis depuis une URL publique, au lieu d’ajouter un fichier lourd à chaque email.',
      },
      {
        title: 'Des destinations explicites',
        text: 'Chaque CTA peut pointer vers votre site, un contenu, LinkedIn ou un calendrier, avec une mesure séparée dans Siglair.',
      },
    ],
    methodTitle: 'Installer la signature dans Gmail.',
    methodIntro:
      'L’installation se fait dans les paramètres Gmail sur ordinateur. L’application mobile possède ses propres réglages de signature et peut produire un résultat différent.',
    steps: [
      {
        title: 'Préparez la version publiée',
        text: 'Vérifiez l’image fixe, l’animation et chaque lien avant d’ouvrir les réglages de Gmail.',
      },
      {
        title: 'Ajoutez-la dans les paramètres',
        text: 'Dans Gmail sur ordinateur, ouvrez Tous les paramètres, trouvez la section Signature, créez une entrée et insérez le rendu Siglair.',
      },
      {
        title: 'Choisissez les usages puis testez',
        text: 'Définissez la signature par défaut pour les nouveaux messages et les réponses, enregistrez, puis envoyez un email vers une autre boîte.',
      },
    ],
    limitTitle: 'Les images restent sous le contrôle du client mail.',
    limitText:
      'Gmail peut masquer une signature derrière les trois points, modifier certains styles collés ou rencontrer un problème d’image. Le destinataire peut aussi bloquer les images distantes. Une signature doit donc garder un texte d’email compréhensible sans dépendre de son affichage.',
    officialLinks: [
      { href: GMAIL_SIGNATURE_HELP, label: 'Suivre l’aide officielle Gmail pour créer une signature' },
      { href: GMAIL_TROUBLESHOOTING, label: 'Résoudre un problème de signature avec l’aide Gmail' },
    ],
    useCaseTitle: 'Dépannage rapide.',
    useCases: [
      { title: 'Image introuvable', text: 'Vérifiez que l’URL publiée est accessible sans connexion.' },
      { title: 'Signature masquée', text: 'Gmail peut la replier derrière les trois points dans un fil.' },
      { title: 'Rendu différent sur mobile', text: 'La signature de l’application Gmail possède ses propres paramètres.' },
      { title: 'Mise en forme altérée', text: 'Évitez de recomposer le rendu après insertion et refaites un test envoyé.' },
    ],
    faq: [
      {
        question: 'Gmail affiche-t-il les signatures animées ?',
        answer:
          'Gmail prend généralement en charge les GIF animés. Le résultat peut varier si les images distantes sont bloquées ou dans un autre client utilisé par le destinataire.',
      },
      {
        question: 'Pourquoi ma signature apparaît-elle derrière trois points ?',
        answer:
          'Gmail peut replier le contenu répété d’une conversation. Google documente ce comportement dans son guide de dépannage des signatures.',
      },
      {
        question: 'La signature est-elle identique dans l’application mobile ?',
        answer:
          'Pas nécessairement. Gmail mobile dispose d’un réglage de signature propre ; testez séparément les envois depuis ordinateur et mobile.',
      },
    ],
  },
};

const RELATED: { path: SeoPath; label: string; description: string }[] = [
  {
    path: '/generateur-signature-email',
    label: 'Générateur de signature email',
    description: 'URL, marque, IA et édition complète.',
  },
  {
    path: '/gestion-signatures-email-entreprise',
    label: 'Gestion des signatures d’entreprise',
    description: 'Modèles, équipes et campagnes centralisées.',
  },
  {
    path: '/campagnes-signature-email',
    label: 'Campagnes de signature email',
    description: 'Bannières, CTA, dates et republication.',
  },
  {
    path: '/signature-email-animee',
    label: 'Signature email animée',
    description: 'GIF, image fixe et première frame.',
  },
  {
    path: '/signature-email-outlook',
    label: 'Signature Outlook',
    description: 'Installation et limites selon les versions.',
  },
  {
    path: '/signature-email-gmail',
    label: 'Signature Gmail',
    description: 'Installation, images et dépannage.',
  },
];

function SignaturePreview({ page }: { page: SeoPageConfig }) {
  return (
    <div className={s.preview} data-kind={page.kind} aria-label={`Aperçu de ${page.eyebrow.toLowerCase()}`}>
      <div className={s.previewBar}>
        <span className={s.appMark} aria-hidden="true">
          {page.preview.app.slice(0, 1)}
        </span>
        <strong>{page.preview.app}</strong>
        <span className={s.previewStatus}>
          <i aria-hidden="true" />
          {page.preview.status}
        </span>
      </div>

      <div className={s.mailMeta} aria-hidden="true">
        <span>À : vous@entreprise.fr</span>
        <span>Objet : Notre prochaine étape</span>
      </div>

      <div className={s.signatureCanvas}>
        <div className={s.logoMotion} aria-hidden="true">
          <span>S</span>
        </div>
        <div className={s.identity}>
          <strong>{page.preview.name}</strong>
          <span>{page.preview.role}</span>
          <a href="mailto:camille@siglair.com">camille@siglair.com</a>
        </div>
        <div className={s.campaignMessage}>
          <small>{page.kind === 'campaign' ? 'Campagne en cours' : 'Message de signature'}</small>
          <p>{page.preview.message}</p>
        </div>
        <div className={s.previewActions}>
          <span>{page.preview.primaryCta} →</span>
          <span>{page.preview.secondaryCta}</span>
        </div>
      </div>

      <div className={s.previewFoot}>
        <span>Rendu hébergé</span>
        <span>GIF + image fixe</span>
      </div>
    </div>
  );
}

function SeoContentPage({ page, currentPath }: { page: SeoPageConfig; currentPath: SeoPath }) {
  usePageMeta(page.metaTitle, page.metaDescription);

  const relatedPages = RELATED.filter((item) => item.path !== currentPath);

  return (
    <div className={s.page} data-page={page.kind}>
      <SiteHeader />

      <main id="contenu" className={s.main}>
        <section className={`${s.wrap} ${s.hero}`}>
          <div className={s.heroCopy}>
            <p className={s.eyebrow}>{page.eyebrow}</p>
            <h1>
              {page.title} <span>{page.accent}</span>
            </h1>
            <p className={s.directAnswer}>{page.directAnswer}</p>
            <div className={s.actions}>
              <Link className={`${s.button} ${s.buttonPrimary}`} to={START_HREF}>
                Créer ma signature
              </Link>
              <Link className={s.button} to="/pricing">
                Voir les tarifs
              </Link>
            </div>
            <ul className={s.heroPoints}>
              {page.heroPoints.map((point) => (
                <li key={point}>{point}</li>
              ))}
            </ul>
          </div>
          <SignaturePreview page={page} />
        </section>

        <section className={`${s.band} ${s.bandBorder}`}>
          <div className={s.wrap}>
            <header className={s.sectionHead}>
              <p className={s.kicker}>Bénéfices</p>
              <h2>{page.benefitTitle}</h2>
            </header>
            <div className={s.benefitGrid}>
              {page.benefits.map((benefit, index) => (
                <article className={s.benefit} key={benefit.title}>
                  <span>{String(index + 1).padStart(2, '0')}</span>
                  <h3>{benefit.title}</h3>
                  <p>{benefit.text}</p>
                </article>
              ))}
            </div>
          </div>
        </section>

        <section className={`${s.wrap} ${s.section}`}>
          <div className={s.methodIntro}>
            <header className={s.sectionHead}>
              <p className={s.kicker}>Méthode</p>
              <h2>{page.methodTitle}</h2>
            </header>
            <p>{page.methodIntro}</p>
          </div>
          <ol className={s.steps}>
            {page.steps.map((step, index) => (
              <li key={step.title}>
                <span>{index + 1}</span>
                <div>
                  <h3>{step.title}</h3>
                  <p>{step.text}</p>
                </div>
              </li>
            ))}
          </ol>
        </section>

        <section className={`${s.band} ${s.bandBorder}`}>
          <div className={`${s.wrap} ${s.honestGrid}`}>
            <div className={s.limitBlock}>
              <p className={s.kicker}>À savoir</p>
              <h2>{page.limitTitle}</h2>
              <p>{page.limitText}</p>
              {page.officialLinks.length > 0 && (
                <div className={s.officialLinks}>
                  {page.officialLinks.map((link) => (
                    <a href={link.href} target="_blank" rel="noreferrer" key={link.href}>
                      {link.label} ↗
                    </a>
                  ))}
                </div>
              )}
            </div>
            <div>
              <h2 className={s.useCaseTitle}>{page.useCaseTitle}</h2>
              <div className={s.useCases}>
                {page.useCases.map((useCase) => (
                  <article key={useCase.title}>
                    <h3>{useCase.title}</h3>
                    <p>{useCase.text}</p>
                  </article>
                ))}
              </div>
            </div>
          </div>
        </section>

        <section className={`${s.wrap} ${s.section} ${s.faqSection}`}>
          <header className={s.sectionHead}>
            <p className={s.kicker}>Questions fréquentes</p>
            <h2>Avant de publier.</h2>
          </header>
          <div className={s.faqList}>
            {page.faq.map((item, index) => (
              <details key={item.question} open={index === 0}>
                <summary>{item.question}</summary>
                <p>{item.answer}</p>
              </details>
            ))}
          </div>
        </section>

        <section className={`${s.wrap} ${s.relatedSection}`}>
          <header className={s.sectionHead}>
            <p className={s.kicker}>Guides liés</p>
            <h2>Continuez avec le bon format.</h2>
          </header>
          <div className={s.relatedGrid}>
            {relatedPages.map((item) => (
              <Link to={item.path} key={item.path}>
                <span>{item.label}</span>
                <p>{item.description}</p>
                <i aria-hidden="true">→</i>
              </Link>
            ))}
          </div>
        </section>

        <section className={`${s.wrap} ${s.finalSection}`}>
          <div>
            <p className={s.kicker}>Première signature gratuite</p>
            <h2>Créez le rendu. Testez-le dans votre vraie boîte mail.</h2>
          </div>
          <Link className={`${s.button} ${s.buttonPrimary}`} to={START_HREF}>
            Commencer gratuitement
          </Link>
        </section>
      </main>

      <SiteFooter />
    </div>
  );
}

export default function SeoPages() {
  const { pathname } = useLocation();
  const currentPath = pathname as SeoPath;
  const page = PAGES[currentPath] ?? PAGES['/campagnes-signature-email'];

  return <SeoContentPage page={page} currentPath={currentPath} />;
}
