import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

const dist = new URL('../dist/', import.meta.url);
const site = 'https://siglair.com';

const indexablePages = [
  '/',
  '/pricing',
  '/generateur-signature-email',
  '/gestion-signatures-email-entreprise',
  '/campagnes-signature-email',
  '/signature-email-animee',
  '/signature-email-outlook',
  '/signature-email-gmail',
  '/signature-email-recherche-emploi',
];

const legalPages = ['/legal/cgu', '/legal/confidentialite', '/legal/mentions'];
const privatePaths = [
  '/app',
  '/app/editor/example',
  '/onboarding',
  '/login',
  '/invite/example',
  '/api',
  '/api/me',
  '/s/example.gif',
  '/c/example/button',
  '/f/example.png',
  '/r/example',
  '/health',
  '/ready',
];
const publicBotFiles = ['/llms.txt', '/llms-full.txt', '/robots.txt', '/sitemap.xml'];
const aiAgents = [
  'OAI-SearchBot',
  'GPTBot',
  'ChatGPT-User',
  'Claude-SearchBot',
  'ClaudeBot',
  'Claude-User',
  'PerplexityBot',
  'Perplexity-User',
  'Google-Extended',
  'Applebot',
  'Applebot-Extended',
];

const htmlPath = (pathname) =>
  pathname === '/'
    ? new URL('index.html', dist)
    : new URL(`${pathname.slice(1)}/index.html`, dist);

const count = (text, pattern) => text.match(pattern)?.length ?? 0;
const canonicalFor = (pathname) => (pathname === '/' ? `${site}/` : `${site}${pathname}`);
const escapeRegExp = (value) => value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

const titles = new Set();
const descriptions = new Set();

for (const pathname of [...indexablePages, ...legalPages]) {
  const html = await readFile(htmlPath(pathname), 'utf8');
  const canonical = canonicalFor(pathname);
  const title = html.match(/<title>([^<]+)<\/title>/i)?.[1]?.trim() ?? '';
  const description = html.match(/<meta name="description" content="([^"]+)"/i)?.[1]?.trim() ?? '';

  assert.equal(count(html, /<title>/gi), 1, `${pathname}: un seul title`);
  assert.equal(count(html, /<h1>/gi), 1, `${pathname}: un seul H1 initial`);
  assert.equal(count(html, /rel="canonical"/gi), 1, `${pathname}: une seule canonical`);
  assert.match(html, /property="og:image" content="https:\/\/siglair\.com\/og\/siglair-og\.png"/);
  assert.match(html, /name="twitter:card" content="summary_large_image"/);
  assert.match(html, new RegExp(`rel="canonical" href="${escapeRegExp(canonical)}"`));
  assert.ok(title.length >= 25 && title.length <= 75, `${pathname}: longueur de title incorrecte`);
  const minimumDescriptionLength = legalPages.includes(pathname) ? 55 : 70;
  assert.ok(
    description.length >= minimumDescriptionLength && description.length <= 200,
    `${pathname}: longueur de description incorrecte`,
  );
  assert.ok(!titles.has(title), `${pathname}: title dupliqué`);
  assert.ok(!descriptions.has(description), `${pathname}: description dupliquée`);
  titles.add(title);
  descriptions.add(description);

  const robotsMeta = html.match(/<meta name="robots" content="([^"]+)"/i)?.[1] ?? '';
  if (legalPages.includes(pathname)) {
    assert.match(robotsMeta, /noindex/);
  } else {
    assert.match(robotsMeta, /index/);
    assert.match(robotsMeta, /follow/);
    assert.doesNotMatch(robotsMeta, /noindex/);
  }

  const jsonLd = html.match(/<script type="application\/ld\+json">([\s\S]+?)<\/script>/)?.[1];
  assert.ok(jsonLd, `${pathname}: JSON-LD absent`);
  const schema = JSON.parse(jsonLd);
  assert.equal(schema['@context'], 'https://schema.org', `${pathname}: contexte schema.org absent`);
  assert.ok(Array.isArray(schema['@graph']), `${pathname}: graphe JSON-LD absent`);
  assert.ok(
    schema['@graph'].some((node) => node['@type'] === 'Organization' && node.name === 'Siglair'),
    `${pathname}: organisation Siglair absente du JSON-LD`,
  );
}

const socialPreview = await readFile(new URL('og/siglair-og.png', dist));
assert.deepEqual([...socialPreview.subarray(0, 8)], [137, 80, 78, 71, 13, 10, 26, 10]);
assert.equal(socialPreview.readUInt32BE(16), 1200, 'largeur OG incorrecte');
assert.equal(socialPreview.readUInt32BE(20), 630, 'hauteur OG incorrecte');

const sitemap = await readFile(new URL('sitemap.xml', dist), 'utf8');
assert.match(sitemap, /^<\?xml version="1\.0" encoding="UTF-8"\?>/);
assert.match(sitemap, /<urlset xmlns="http:\/\/www\.sitemaps\.org\/schemas\/sitemap\/0\.9">/);

const sitemapEntries = [...sitemap.matchAll(/<url>\s*<loc>([^<]+)<\/loc>\s*<lastmod>([^<]+)<\/lastmod>\s*<\/url>/g)]
  .map((match) => ({ loc: match[1], lastmod: match[2] }));
const expectedUrls = indexablePages.map(canonicalFor);

assert.deepEqual(
  sitemapEntries.map(({ loc }) => loc).sort(),
  [...expectedUrls].sort(),
  'le sitemap doit contenir exactement les pages indexables',
);
assert.equal(new Set(sitemapEntries.map(({ loc }) => loc)).size, sitemapEntries.length, 'URL sitemap dupliquée');

const today = new Date();
today.setUTCHours(23, 59, 59, 999);
for (const { loc, lastmod } of sitemapEntries) {
  assert.ok(loc.startsWith(`${site}/`), `origine sitemap inattendue: ${loc}`);
  assert.match(lastmod, /^\d{4}-\d{2}-\d{2}$/, `${loc}: lastmod doit être une date W3C`);
  const parsed = new Date(`${lastmod}T00:00:00Z`);
  assert.ok(!Number.isNaN(parsed.valueOf()), `${loc}: lastmod invalide`);
  assert.ok(parsed <= today, `${loc}: lastmod ne peut pas être dans le futur`);
}
for (const pathname of [...legalPages, ...privatePaths]) {
  assert.doesNotMatch(sitemap, new RegExp(`<loc>${escapeRegExp(canonicalFor(pathname))}</loc>`));
}

function parseRobots(source) {
  const groups = [];
  let group = null;

  const flush = () => {
    if (group?.agents.length) groups.push(group);
    group = null;
  };

  for (const rawLine of source.split(/\r?\n/)) {
    const line = rawLine.replace(/\s+#.*$/, '').trim();
    if (!line) {
      flush();
      continue;
    }
    const separator = line.indexOf(':');
    if (separator < 0) continue;
    const key = line.slice(0, separator).trim().toLowerCase();
    const value = line.slice(separator + 1).trim();
    if (key === 'user-agent') {
      if (group?.directives.length) flush();
      group ??= { agents: [], directives: [] };
      group.agents.push(value);
    } else if (group) {
      group.directives.push({ key, value });
    }
  }
  flush();
  return groups;
}

function robotsPattern(pattern) {
  const anchored = pattern.endsWith('$');
  const body = anchored ? pattern.slice(0, -1) : pattern;
  const regex = escapeRegExp(body).replaceAll('\\*', '.*');
  return new RegExp(`^${regex}${anchored ? '$' : ''}`);
}

function pathAllowed(group, pathname) {
  const matches = group.directives
    .filter(({ key, value }) => (key === 'allow' || key === 'disallow') && value)
    .filter(({ value }) => robotsPattern(value).test(pathname))
    .sort((a, b) => b.value.length - a.value.length || (a.key === 'allow' ? -1 : 1));
  return matches[0]?.key !== 'disallow';
}

const robots = await readFile(new URL('robots.txt', dist), 'utf8');
const robotGroups = parseRobots(robots);
const wildcardGroup = robotGroups.find(({ agents }) => agents.includes('*'));
assert.ok(wildcardGroup, 'groupe robots générique absent');

const aiGroup = robotGroups.find(({ agents }) => aiAgents.every((agent) => agents.includes(agent)));
assert.ok(aiGroup, 'groupe explicite des principaux crawlers IA absent');

for (const group of [wildcardGroup, aiGroup]) {
  for (const pathname of privatePaths) {
    assert.equal(pathAllowed(group, pathname), false, `${group.agents.join(', ')} doit bloquer ${pathname}`);
  }
  for (const pathname of [...indexablePages, ...publicBotFiles, '/apple-touch-icon.png']) {
    assert.equal(pathAllowed(group, pathname), true, `${group.agents.join(', ')} doit autoriser ${pathname}`);
  }
}
assert.match(robots, /Sitemap: https:\/\/siglair\.com\/sitemap\.xml/);
assert.doesNotMatch(robots, /^\s*Disallow:\s*\/$/m, 'la racine publique ne doit pas être bloquée');

const spa = await readFile(new URL('spa.html', dist), 'utf8');
assert.match(spa, /name="robots" content="noindex,nofollow,noarchive"/);

const llms = await readFile(new URL('llms.txt', dist), 'utf8');
assert.equal(count(llms, /^# /gm), 1, 'llms.txt doit avoir un seul H1');
assert.match(llms, /^# Siglair\n\n> .+/);
assert.match(llms, /proposition communautaire de \[llmstxt\.org\]/i);
assert.match(llms, /ne garantit ni exploration, ni indexation, ni citation/i);

const llmsLinks = [...llms.matchAll(/^- \[([^\]]+)\]\((https:\/\/[^)]+)\): (.+)$/gm)]
  .map(([, label, url, description]) => ({ label, url, description }));
assert.ok(llmsLinks.length >= 10, 'llms.txt doit fournir des liens décrits');
for (const { label, url, description } of llmsLinks) {
  assert.ok(label.length >= 3, `libellé llms.txt trop court pour ${url}`);
  assert.ok(description.length >= 20, `description llms.txt trop courte pour ${url}`);
  assert.ok(url.startsWith(`${site}/`), `lien llms.txt non canonique: ${url}`);
  assert.doesNotMatch(url, /\/app(?:\/|$)|\/api(?:\/|$)|\/login(?:\/|$)|\/invite(?:\/|$)/);
}
for (const url of [...expectedUrls, `${site}/llms-full.txt`]) {
  assert.ok(llmsLinks.some((link) => link.url === url), `source absente de llms.txt: ${url}`);
}

const llmsFull = await readFile(new URL('llms-full.txt', dist), 'utf8');
assert.match(llmsFull, /^# Siglair : contexte produit complet/);
for (const fact of [
  'signature email',
  'campagnes',
  'première image',
  'Powered by siglair.com',
  '30 jours',
  '12 mois',
  'recherche d’emploi',
]) {
  assert.ok(llmsFull.includes(fact), `contexte détaillé incomplet: ${fact}`);
}
assert.match(llmsFull, /ne garantit pas/i);
assert.match(llmsFull, /ne garantit pas qu'un moteur ou un assistant l'explore, l'indexe, le cite/i);

const pricing = await readFile(htmlPath('/pricing'), 'utf8');
assert.match(pricing, /<title>Tarifs des signatures et campagnes email \| Siglair<\/title>/);
assert.match(pricing, /<h1>Gratuit pour signer\. Payant pour diffuser\.<\/h1>/);
assert.match(pricing, /Pro enlève la marque, ouvre les campagnes datées/);
assert.match(pricing, /Free héberge une signature animée/);

console.log(
  `SEO smoke: ${indexablePages.length} pages indexables, ${legalPages.length} pages noindex, ` +
    `${aiAgents.length} crawlers IA et ${sitemapEntries.length} URL sitemap validés.`,
);
