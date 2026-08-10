import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { join } from 'node:path';

const dist = new URL('../dist/', import.meta.url);
const site = 'https://siglair.com';

const indexablePages = [
  '/',
  '/pricing',
  '/campagnes-signature-email',
  '/signature-email-animee',
  '/signature-email-outlook',
  '/signature-email-gmail',
];

const legalPages = ['/legal/cgu', '/legal/confidentialite', '/legal/mentions'];

const htmlPath = (pathname) =>
  pathname === '/'
    ? new URL('index.html', dist)
    : new URL(`${pathname.slice(1)}/index.html`, dist);

const count = (text, pattern) => text.match(pattern)?.length ?? 0;

for (const pathname of [...indexablePages, ...legalPages]) {
  const html = await readFile(htmlPath(pathname), 'utf8');
  const canonical = pathname === '/' ? `${site}/` : `${site}${pathname}`;

  assert.equal(count(html, /<title>/g), 1, `${pathname}: un seul title`);
  assert.equal(count(html, /<h1>/g), 1, `${pathname}: un seul H1 initial`);
  assert.equal(count(html, /rel="canonical"/g), 1, `${pathname}: une seule canonical`);
  assert.match(html, new RegExp(`rel="canonical" href="${canonical}"`));
  assert.match(html, /<meta name="description" content="[^"]+"/);

  const robots = html.match(/<meta name="robots" content="([^"]+)"/i)?.[1] ?? '';
  if (legalPages.includes(pathname)) {
    assert.match(robots, /noindex/);
  } else {
    assert.doesNotMatch(robots, /noindex/);
  }

  const jsonLd = html.match(/<script type="application\/ld\+json">([\s\S]+?)<\/script>/)?.[1];
  assert.ok(jsonLd, `${pathname}: JSON-LD absent`);
  assert.doesNotThrow(() => JSON.parse(jsonLd), `${pathname}: JSON-LD invalide`);
}

const sitemap = await readFile(new URL('sitemap.xml', dist), 'utf8');
for (const pathname of indexablePages) {
  const canonical = pathname === '/' ? `${site}/` : `${site}${pathname}`;
  assert.match(sitemap, new RegExp(`<loc>${canonical}</loc>`));
}
for (const pathname of [...legalPages, '/login', '/app']) {
  assert.doesNotMatch(sitemap, new RegExp(`<loc>${site}${pathname}`));
}

const robots = await readFile(new URL('robots.txt', dist), 'utf8');
for (const agent of ['*', 'OAI-SearchBot', 'PerplexityBot']) {
  const group = robots.match(new RegExp(`User-agent: ${agent === '*' ? '\\*' : agent}([\\s\\S]*?)(?=\\nUser-agent:|\\nSitemap:)`))?.[1] ?? '';
  assert.ok(group, `groupe robots absent: ${agent}`);
  for (const pathname of ['/app', '/onboarding', '/login', '/invite/', '/api/', '/s/', '/c/', '/f/']) {
    assert.match(group, new RegExp(`Disallow: ${pathname.replace('/', '\\/')}`));
  }
}
assert.match(robots, /Sitemap: https:\/\/siglair\.com\/sitemap\.xml/);

const spa = await readFile(new URL('spa.html', dist), 'utf8');
assert.match(spa, /name="robots" content="noindex,nofollow,noarchive"/);

const llms = await readFile(new URL('llms.txt', dist), 'utf8');
assert.match(llms, /résumé factuel/i);
assert.match(llms, /restent sous leur contrôle/i);

const pricing = await readFile(htmlPath('/pricing'), 'utf8');
assert.match(pricing, /<title>Tarifs des signatures et campagnes email \| Siglair<\/title>/);
assert.match(pricing, /<h1>Gratuit pour signer\. Payant pour diffuser\.<\/h1>/);
// La grille servie aux crawlers doit être celle de src/plans.rs : Free héberge le GIF,
// le verrou Pro est la marque et les campagnes. L'ancienne assertion décrivait la grille
// abandonnée (« Free exporte du HTML statique ») et la verrouillait dans les résultats.
assert.match(pricing, /Pro enlève la marque, ouvre les campagnes datées/);
assert.match(pricing, /Free héberge une signature animée/);

console.log(`SEO smoke: ${indexablePages.length} pages indexables et ${legalPages.length} pages noindex validées.`);
