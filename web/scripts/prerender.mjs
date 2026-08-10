import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const SITE_URL = 'https://siglair.com';
const rootDir = join(dirname(fileURLToPath(import.meta.url)), '..');
const distDir = join(rootDir, 'dist');
const baseTemplate = await readFile(join(distDir, 'index.html'), 'utf8');

const sharedProductLinks = [
  ['/generateur-signature-email', 'Générateur de signature email'],
  ['/gestion-signatures-email-entreprise', 'Gestion des signatures d’entreprise'],
  ['/campagnes-signature-email', 'Campagnes de signature email'],
  ['/signature-email-animee', 'Signature email animée'],
  ['/signature-email-outlook', 'Signature email Outlook'],
  ['/signature-email-gmail', 'Signature email Gmail'],
  ['/pricing', 'Tarifs'],
];

const pages = [
  {
    path: '/',
    title: 'Siglair | Signature email marketing animée',
    description:
      'Transformez chaque email en canal marketing : signature animée, campagnes et CTA, mise à jour sans copier-coller, analytics, Gmail et Outlook.',
    heading: 'Votre signature email. Un canal marketing à part entière.',
    intro:
      'Collez votre site, Siglair compose la signature. Ajoutez ensuite campagnes, CTA, lancements et contenus, puis republiez sans demander un nouveau copier-coller.',
    points: [
      'Créez une signature aux couleurs de votre marque avec un éditeur visuel.',
      'Ajoutez une campagne, une bannière ou un CTA sous vos conversations.',
      'Republiez sur la même URL hébergée sans demander un nouveau copier-coller.',
      'Gardez une première image lisible lorsque le client mail ne joue pas l\u2019animation.',
    ],
    links: sharedProductLinks,
    schema: 'software',
  },
  {
    path: '/pricing',
    title: 'Tarifs des signatures et campagnes email | Siglair',
    description:
      'Le plan gratuit héberge votre signature animée. Pro enlève la marque et ouvre les campagnes datées. Team fait des emails de votre équipe un canal marketing piloté.',
    heading: 'Gratuit pour signer. Payant pour diffuser.',
    intro:
      'Le plan gratuit héberge une vraie signature animée sur son URL, avec une discrète mention Siglair. Pro enlève la marque, ouvre les campagnes datées sur vos signatures et mesure les clics. Team pousse une campagne sur les signatures de toute l\u2019équipe.',
    points: [
      'Free héberge une signature animée sur son URL, avec une mention Siglair.',
      'Pro enlève la marque, ouvre les campagnes datées sur ses signatures et mesure les clics.',
      'Team pousse une campagne sur les signatures de toute l\u2019équipe en un clic.',
    ],
    links: [['/', 'Découvrir Siglair'], ...sharedProductLinks.slice(0, 4)],
    schema: 'webpage',
  },
  {
    path: '/generateur-signature-email',
    title: 'Générateur de signature email gratuit | Siglair',
    description:
      'Créez gratuitement une signature email professionnelle à partir de votre site : logo, couleurs, coordonnées, CTA, aperçu Gmail et Outlook, puis édition complète.',
    heading: 'Collez votre site. Repartez avec une vraie signature.',
    intro:
      'Le générateur Siglair analyse les informations publiques de votre site, prépare une composition aux couleurs de votre marque et l’ouvre dans un éditeur visuel entièrement modifiable.',
    points: [
      'Récupérez le nom, le logo, les couleurs et les liens publics disponibles.',
      'Obtenez une première composition sans partir d’un canvas vide.',
      'Importez les éléments manquants lorsqu’un site bloque l’analyse automatique.',
      'Ajustez chaque texte, image, CTA, couleur, taille et animation.',
      'Contrôlez une image fixe lisible avant de publier le GIF hébergé.',
      'Installez ensuite la signature dans Gmail ou Outlook.',
    ],
    links: [['/', 'Siglair'], ['/signature-email-gmail', 'Installation Gmail'], ['/signature-email-outlook', 'Installation Outlook'], ['/pricing', 'Tarifs']],
    schema: 'service',
  },
  {
    path: '/gestion-signatures-email-entreprise',
    title: 'Gestion des signatures email d’entreprise | Siglair',
    description:
      'Centralisez les signatures email de votre entreprise : modèle partagé, identité cohérente, campagnes datées, publication sans réinstallation et mesure par CTA.',
    heading: 'Une marque cohérente dans chaque boîte mail.',
    intro:
      'Siglair centralise modèles, profils et campagnes sur des URL hébergées stables. Le marketing programme le message du moment sans demander à chaque membre de modifier à nouveau son client mail.',
    points: [
      'Définissez un modèle partagé au lieu de distribuer des copier-coller divergents.',
      'Conservez les coordonnées propres à chaque membre de l’organisation.',
      'Programmez une bannière ou un CTA pour un lancement, un contenu ou un événement.',
      'Activez puis retirez automatiquement la campagne aux dates prévues.',
      'Mesurez séparément les clics de la bannière, du site et des autres CTA.',
      'Testez la première image et le rendu réel sur le parc Gmail et Outlook de l’équipe.',
    ],
    links: [['/', 'Siglair'], ['/campagnes-signature-email', 'Campagnes'], ['/signature-email-animee', 'Signature animée'], ['/pricing', 'Tarifs']],
    schema: 'service',
  },
  {
    path: '/campagnes-signature-email',
    title: 'Campagnes de signature email et bannières | Siglair',
    description:
      'Transformez vos signatures email en actif marketing : bannière, CTA, calendrier de campagne, republication sur URL stable et clics mesurés séparément.',
    heading: 'Lancez une campagne sous chaque conversation',
    intro:
      'La signature email devient un emplacement marketing utile pour promouvoir un rendez-vous, un contenu, un événement ou un lancement.',
    points: [
      'Ajoutez une bannière et un CTA dans la signature.',
      'Préparez le message et ses dates depuis l\u2019éditeur de campagne.',
      'Laissez Siglair activer puis retirer automatiquement le rendu aux dates prévues.',
      'Mesurez séparément les clics sur les appels à l\u2019action.',
    ],
    links: [['/', 'Siglair'], ['/signature-email-animee', 'Signature animée'], ['/pricing', 'Tarifs']],
    schema: 'service',
  },
  {
    path: '/signature-email-animee',
    title: 'Signature email animée pour Gmail et Outlook | Siglair',
    description:
      'Créez une signature email animée rendue côté serveur, avec GIF optimisé, image fixe de secours et première frame conçue pour rester lisible.',
    heading: 'Ajoutez du mouvement. Gardez le message lisible partout.',
    intro:
      'Utilisez le mouvement pour mettre en valeur votre marque, votre message et vos CTA sans sacrifier la lisibilité de la signature.',
    points: [
      'Composez le design, les textes, les images et les animations dans un éditeur visuel.',
      'Conservez une première image autonome lorsque l\u2019animation est bloquée.',
      'Publiez une image hébergée et cliquable, mise à jour depuis Siglair.',
      'Adaptez le rendu à Gmail, Outlook web, Apple Mail et Outlook classique.',
    ],
    links: [['/', 'Siglair'], ['/signature-email-outlook', 'Compatibilité Outlook'], ['/signature-email-gmail', 'Installation Gmail']],
    schema: 'service',
  },
  {
    path: '/signature-email-outlook',
    title: 'Créer une signature Outlook professionnelle | Siglair',
    description:
      'Créez et installez une signature Outlook professionnelle, avec une première frame lisible lorsque l\u2019application Windows fige le GIF.',
    heading: 'Une signature Outlook professionnelle. Animée quand le client le permet.',
    intro:
      'Outlook sur le web peut afficher l\u2019animation, tandis que certaines applications Outlook pour Windows montrent uniquement la première image du GIF.',
    points: [
      'Préparez une première image qui porte seule le nom, le rôle et le message essentiel.',
      'Utilisez l\u2019export simplifié lorsque le moteur de rendu Outlook l\u2019exige.',
      'Gardez les liens et les appels à l\u2019action accessibles dans la signature.',
      'Republiez le rendu hébergé sans changer son URL.',
    ],
    links: [['/', 'Siglair'], ['/signature-email-animee', 'Signature animée'], ['/signature-email-gmail', 'Signature Gmail']],
    schema: 'service',
  },
  {
    path: '/signature-email-gmail',
    title: 'Créer une signature Gmail professionnelle | Siglair',
    description:
      'Créez une signature Gmail professionnelle avec image hébergée, animation généralement prise en charge, CTA cliquables et guide de dépannage.',
    heading: 'Une signature Gmail claire, animée et cliquable.',
    intro:
      'Siglair fournit une signature hébergée que vous pouvez ajouter aux paramètres Gmail et republier lorsque votre marque ou votre campagne change.',
    points: [
      'Créez le visuel, les informations de contact et les CTA dans l\u2019éditeur.',
      'Ajoutez la signature dans les paramètres de rédaction de Gmail.',
      'Profitez de l\u2019animation lorsque Gmail charge les images distantes.',
      'Mettez à jour le rendu sur la même URL hébergée.',
    ],
    links: [['/', 'Siglair'], ['/signature-email-animee', 'Signature animée'], ['/signature-email-outlook', 'Signature Outlook']],
    schema: 'service',
  },
  {
    path: '/legal/cgu',
    title: 'Conditions générales d\u2019utilisation | Siglair',
    description: 'Consultez les conditions générales d\u2019utilisation du service Siglair.',
    heading: 'Conditions générales d\u2019utilisation',
    intro:
      'Cette page présente les conditions applicables à l\u2019accès et à l\u2019utilisation du service Siglair.',
    points: [],
    links: [['/', 'Accueil'], ['/legal/confidentialite', 'Confidentialité'], ['/legal/mentions', 'Mentions légales']],
    schema: 'webpage',
  },
  {
    path: '/legal/confidentialite',
    title: 'Politique de confidentialité | Siglair',
    description: 'Consultez la politique de confidentialité et de traitement des données de Siglair.',
    heading: 'Politique de confidentialité',
    intro:
      'Cette page explique les données traitées par Siglair, leurs finalités et les droits des utilisateurs.',
    points: [],
    links: [['/', 'Accueil'], ['/legal/cgu', 'Conditions d\u2019utilisation'], ['/legal/mentions', 'Mentions légales']],
    schema: 'webpage',
  },
  {
    path: '/legal/mentions',
    title: 'Mentions légales | Siglair',
    description: 'Consultez les mentions légales du site et du service Siglair.',
    heading: 'Mentions légales',
    intro: 'Cette page rassemble les informations légales relatives au site et au service Siglair.',
    points: [],
    links: [['/', 'Accueil'], ['/legal/cgu', 'Conditions d\u2019utilisation'], ['/legal/confidentialite', 'Confidentialité']],
    schema: 'webpage',
  },
];

function escapeHtml(value) {
  return value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;');
}

function absoluteUrl(path) {
  return path === '/' ? `${SITE_URL}/` : `${SITE_URL}${path}`;
}

function metadata(page) {
  const canonical = absoluteUrl(page.path);
  const robots = page.path.startsWith('/legal/')
    ? 'noindex,nofollow,noarchive'
    : 'index,follow,max-image-preview:large,max-snippet:-1,max-video-preview:-1';
  return `
    <title>${escapeHtml(page.title)}</title>
    <meta name="description" content="${escapeHtml(page.description)}" />
    <meta name="robots" content="${robots}" />
    <link rel="canonical" href="${canonical}" />
    <meta property="og:type" content="website" />
    <meta property="og:site_name" content="Siglair" />
    <meta property="og:locale" content="fr_FR" />
    <meta property="og:url" content="${canonical}" />
    <meta property="og:title" content="${escapeHtml(page.title)}" />
    <meta property="og:description" content="${escapeHtml(page.description)}" />
    <meta property="og:image" content="${SITE_URL}/og/siglair-og.png" />
    <meta property="og:image:width" content="1200" />
    <meta property="og:image:height" content="630" />
    <meta property="og:image:alt" content="Aperçu de Siglair, générateur de signature email marketing" />
    <meta name="twitter:card" content="summary_large_image" />
    <meta name="twitter:image" content="${SITE_URL}/og/siglair-og.png" />
    <meta name="twitter:url" content="${canonical}" />
    <meta name="twitter:title" content="${escapeHtml(page.title)}" />
    <meta name="twitter:description" content="${escapeHtml(page.description)}" />`;
}

function schemaFor(page) {
  const canonical = absoluteUrl(page.path);
  const organization = {
    '@type': 'Organization',
    '@id': `${SITE_URL}/#organization`,
    name: 'Siglair',
    url: `${SITE_URL}/`,
    logo: `${SITE_URL}/brand/siglair-mark.png`,
    description:
      'SaaS français de création, d’hébergement et de pilotage de signatures email animées utilisées comme canal marketing.',
  };
  const webSite = {
    '@type': 'WebSite',
    '@id': `${SITE_URL}/#website`,
    url: `${SITE_URL}/`,
    name: 'Siglair',
    inLanguage: 'fr-FR',
    publisher: { '@id': `${SITE_URL}/#organization` },
  };

  if (page.path === '/') {
    return {
      '@context': 'https://schema.org',
      '@graph': [
        organization,
        webSite,
        {
          '@type': 'WebPage',
          '@id': `${SITE_URL}/#webpage`,
          url: `${SITE_URL}/`,
          name: page.title,
          description: page.description,
          inLanguage: 'fr-FR',
          isPartOf: { '@id': `${SITE_URL}/#website` },
        },
        {
          '@type': 'SoftwareApplication',
          '@id': `${SITE_URL}/#software`,
          name: 'Siglair',
          url: `${SITE_URL}/`,
          description: page.description,
          applicationCategory: 'BusinessApplication',
          operatingSystem: 'Web',
          inLanguage: 'fr-FR',
          provider: { '@id': `${SITE_URL}/#organization` },
          offers: {
            '@type': 'Offer',
            price: '0',
            priceCurrency: 'EUR',
            category: 'Free',
            url: `${SITE_URL}/pricing`,
          },
          featureList: [
            'Éditeur visuel de signature email',
            'Animations et première image de repli',
            'Campagnes et appels à l\u2019action',
            'Republication sur une URL hébergée stable',
            'Mesure des clics',
          ],
        },
      ],
    };
  }

  const graph = [
    organization,
    webSite,
    {
      '@type': 'WebPage',
      '@id': `${canonical}#webpage`,
      url: canonical,
      name: page.title,
      description: page.description,
      inLanguage: 'fr-FR',
      isPartOf: { '@id': `${SITE_URL}/#website` },
      about: { '@id': `${SITE_URL}/#organization` },
    },
    {
      '@type': 'BreadcrumbList',
      '@id': `${canonical}#breadcrumb`,
      itemListElement: [
        { '@type': 'ListItem', position: 1, name: 'Accueil', item: `${SITE_URL}/` },
        { '@type': 'ListItem', position: 2, name: page.heading, item: canonical },
      ],
    },
  ];

  if (page.schema === 'service') {
    graph.push({
      '@type': 'Service',
      '@id': `${canonical}#service`,
      name: page.heading,
      description: page.description,
      url: canonical,
      provider: { '@id': `${SITE_URL}/#organization` },
      areaServed: 'FR',
    });
  }

  return { '@context': 'https://schema.org', '@graph': graph };
}

function staticContent(page) {
  const points = page.points.length
    ? `<ul>${page.points.map((point) => `<li>${escapeHtml(point)}</li>`).join('')}</ul>`
    : '';
  const links = page.links
    .map(([href, label]) => `<a href="${href}">${escapeHtml(label)}</a>`)
    .join('');

  return `<div id="root">
      <header class="seo-header"><a class="seo-brand" href="/">Siglair</a><nav aria-label="Navigation principale">${sharedProductLinks
        .slice(0, 3)
        .map(([href, label]) => `<a href="${href}">${escapeHtml(label)}</a>`)
        .join('')}</nav></header>
      <main class="seo-main">
        <p class="seo-kicker">Signature email marketing</p>
        <h1>${escapeHtml(page.heading)}</h1>
        <p class="seo-intro">${escapeHtml(page.intro)}</p>
        ${points}
        <nav class="seo-links" aria-label="Pages associées">${links}</nav>
      </main>
      <footer class="seo-footer"><a href="/legal/cgu">CGU</a><a href="/legal/confidentialite">Confidentialité</a><a href="/legal/mentions">Mentions légales</a></footer>
    </div>`;
}

const fallbackCss = `<style data-prerender-style>
      .seo-header,.seo-main,.seo-footer{width:min(1120px,calc(100% - 40px));margin-inline:auto}.seo-header{min-height:72px;display:flex;align-items:center;justify-content:space-between;gap:24px}.seo-header nav,.seo-links,.seo-footer{display:flex;flex-wrap:wrap;gap:20px}.seo-header a,.seo-links a,.seo-footer a{color:inherit}.seo-brand{font-size:20px;font-weight:800;text-decoration:none}.seo-main{padding:clamp(72px,11vw,150px) 0}.seo-main h1{max-width:850px;margin:12px 0 24px;font-size:clamp(40px,7vw,76px);line-height:1.02}.seo-kicker{text-transform:uppercase;font-weight:800;color:#55d8ff}.seo-intro{max-width:780px;font-size:clamp(19px,2.4vw,26px);line-height:1.5}.seo-main ul{max-width:760px;padding-left:22px;line-height:1.8}.seo-links{margin-top:36px}.seo-footer{padding:28px 0 48px;border-top:1px solid #26324a}@media(max-width:720px){.seo-header nav{display:none}}
    </style>`;

function stripRouteMetadata(html) {
  return html
    .replace(/<title[^>]*>[\s\S]*?<\/title>\s*/gi, '')
    .replace(/<meta\b[^>]*(?:name|property)=["'](?:description|robots|og:[^"']+|twitter:[^"']+)["'][^>]*>\s*/gi, '')
    .replace(/<link\b[^>]*rel=["']canonical["'][^>]*>\s*/gi, '');
}

async function renderPage(page) {
  const schema = JSON.stringify(schemaFor(page)).replaceAll('<', '\\u003c');
  let html = stripRouteMetadata(baseTemplate);
  html = html.replace(
    '</head>',
    `${metadata(page)}\n    <script type="application/ld+json">${schema}</script>\n    ${fallbackCss}\n  </head>`,
  );
  html = html.replace('<div id="root"></div>', staticContent(page));

  const outputPath =
    page.path === '/' ? join(distDir, 'index.html') : join(distDir, page.path.slice(1), 'index.html');
  await mkdir(dirname(outputPath), { recursive: true });
  await writeFile(outputPath, html);
}

await Promise.all(pages.map(renderPage));

const privateShell = stripRouteMetadata(baseTemplate)
  .replace(
    '</head>',
    `    <title>Siglair</title>\n    <meta name="robots" content="noindex,nofollow,noarchive" />\n  </head>`,
  )
  .replace('<div id="root"></div>', '<div id="root"></div>');
await writeFile(join(distDir, 'spa.html'), privateShell);

console.log(`Prerendered ${pages.length} public routes and the private SPA shell.`);
