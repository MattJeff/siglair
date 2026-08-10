# Siglair — Contrat technique (source unique de vérité)

> Tout code écrit par un agent DOIT respecter ce document. En cas de doute, ce fichier gagne.
> Ne pas inventer de champ, de route ou de table absent d'ici. Si un ajout est nécessaire,
> l'implémenter au plus près de ce qui existe et le signaler dans le rapport final.

## 0. Produit

Siglair héberge des signatures email animées.

Le vrai produit n'est pas l'éditeur (l'éditeur est le hameçon) : c'est l'**URL hébergée**.
L'utilisateur colle dans Gmail/Outlook un `<img src="https://siglair.com/s/{slug}.gif">`.
Le serveur rend le GIF, le sert, compte les ouvertures, redirige et compte les clics.
Changer sa signature = republier ; aucun besoin de recoller le HTML dans le client mail.

C'est ça qu'on facture.

## 1. Stack — verrouillée

| Couche | Choix | Pourquoi |
|---|---|---|
| API | Rust, `axum` 0.8, `tokio` | demandé |
| DB | PostgreSQL 16, `sqlx` 0.8 (requêtes **runtime**, pas de macro `query!`) | pas de cache `.sqlx` à maintenir en CI |
| Migrations | `sqlx::migrate!("./migrations")` embarqué | zéro outil externe au runtime |
| Sessions | cookie opaque `sig_session`, HttpOnly, Secure, SameSite=Lax, table `sessions` | révocation immédiate, pas de JWT à révoquer |
| OAuth | Google OIDC + Apple Sign In, code flow + PKCE | demandé |
| Email | Resend, API REST via `reqwest` | demandé |
| Paiement | Stripe, API REST via `reqwest` (**pas** `async-stripe`) | `async-stripe` = +4 min de compilation pour 6 endpoints |
| Rendu | `chromiumoxide` (CDP) + `ffmpeg` en sous-processus | seule façon fiable de rasteriser du CSS animé |
| File d'attente | table `render_jobs` + `FOR UPDATE SKIP LOCKED` | ponytail : pas de Redis pour 3 jobs/minute |
| Stockage | disque (`STORAGE_DIR`), derrière le trait `Storage` | S3 le jour où il y a 2 machines, pas avant |
| Front | React 19 + Vite + TypeScript, React Router | demandé |
| CSS front | CSS modules + variables, **pas** de framework UI | la charte existe déjà dans le HTML source |

**Un seul crate Rust**, deux binaires : `src/bin/api.rs` et `src/bin/renderer.rs`, lib partagée dans `src/lib.rs`.
Pas de workspace multi-crates : rien à y gagner ici, et ça double le temps de compilation.

## 2. Règle anti-divergence n°1 — le rendu HTML est écrit UNE fois, en Rust

Le document de signature (`doc`) se transforme en HTML à trois endroits :
l'export utilisateur, la page rendue par Chromium pour le GIF, et l'aperçu dans l'éditeur.

**Une seule implémentation : `src/render/html.rs` en Rust.**

- Export → `GET /api/signatures/{id}/export?mode=...` renvoie ce HTML.
- GIF → le renderer charge ce même HTML dans Chromium.
- Aperçu éditeur → le front appelle `POST /api/preview` et affiche le HTML dans une `<iframe srcdoc>`.

Le canvas d'**édition** React (drag, poignées, sélection) est du DOM React normal et n'a pas à
produire un HTML identique — il sert à manipuler, pas à rendre. Toute tentative de réimplémenter
le rendu en TypeScript est un bug : deux moteurs finissent toujours par diverger, et le client
découvrira l'écart dans son Outlook, pas dans l'aperçu.

## 3. Le document de signature (`doc`) — JSON, versionné

Repris tel quel de l'éditeur d'origine pour que le port soit mécanique. Stocké en `jsonb`.

```jsonc
{
  "v": 1,
  "canvas": {
    "width": 620, "height": 250,
    "bg": "#07111f",
    "bgImage": "",            // URL http(s) ou "" — jamais de data: en base
    "overlay": 0.8,           // 0..1, voile noir au-dessus de bgImage
    "radius": 18
  },
  "elements": [ /* Element[] — l'ordre du tableau EST le z-index, index 0 = derrière */ ],
  "timelineDuration": 6.0     // secondes ; borne la durée du GIF
}
```

`canvas.bg` et `elements[].background` acceptent une couleur `#rrggbb` ou un dégradé
linéaire, radial ou conique généré par l’éditeur. La grammaire est volontairement fermée :
aucune URL ni fonction CSS arbitraire ne peut entrer dans le document. En mode email sûr,
la première couleur du dégradé sert de repli pour les clients qui ne savent pas l’afficher.

### Element

```jsonc
{
  "id": "a1b2c3d",            // [a-z0-9]{7}, unique dans le document
  "type": "text",             // text | button | image | video | badge | shape | divider | banner
  "x": 20, "y": 20, "w": 160, "h": 32,   // px, repère canvas, origine haut-gauche
  "rotation": 0,              // degrés
  "opacity": 1,               // 0..1
  "content": "Texte",         // texte affiché ; supporte les jetons {{...}}
  "href": "",                 // URL cible ; supporte les jetons ; "" = non cliquable
  "assetId": null,            // uuid d'un asset, pour type image|video — remplace l'ancien `src`
  "fontSize": 13,
  "fontWeight": "500",
  "color": "#ffffff",
  "background": "#2563eb",
  "radius": 0,
  "align": "left",            // left | center | right
  "locked": false,
  "hidden": false,
  "anim": {
    "preset": "none",         // cf. §3.2
    "duration": 2.4,          // s, 0.1..20
    "delay": 0,               // s, 0..20
    "iterations": "infinite", // "infinite" | "1".."10"
    "easing": "ease-in-out",  // linear | ease | ease-in | ease-out | ease-in-out
    "intensity": 1,           // 0.1..2 — multiplicateur d'amplitude
    "direction": "normal"     // normal | reverse | alternate
  }
}
```

**Changement volontaire vs l'original : `src` (data-URL) devient `assetId`.**
Des data-URL dans le document, c'était acceptable en localStorage ; en base ça donne des lignes
de plusieurs Mo, un `jsonb` illisible, aucune déduplication et aucun moyen de servir une image
au client mail. Les médias vivent dans la table `assets` et sont servis par URL.

### 3.1 Jetons de profil

`{{name}} {{role}} {{email}} {{phone}} {{website}} {{linkedin}} {{whatsapp}} {{tagline}} {{company}} {{portfolio}} {{github}} {{cv}} {{availability}} {{location}} {{university}} {{graduation}}`

Résolus depuis `signature.profile` (jsonb, mêmes clés). Un jeton inconnu est remplacé par `""`.
La résolution se fait **côté serveur** au rendu. Un jeton inconnu ne doit jamais laisser
`{{...}}` visible dans un email envoyé à un client.

C'est ce qui rend le plan Team vendable : un modèle d'organisation + le profil de chaque membre
= N signatures cohérentes générées d'un coup.

### 3.2 Presets d'animation — liste fermée

`none pulse glow float rotate bounce zoom fade reveal shimmer flicker swing slide draw`

Le CSS de chaque preset est défini **une seule fois**, dans `src/render/anim.css`, inclus par
`html.rs` dans le `<style>` du document rendu. Chaque preset lit `--intensity` et `--duration`.
Le front n'a pas sa propre copie : l'aperçu passe par `/api/preview`.

## 4. Schéma PostgreSQL

Fichiers `migrations/NNNN_nom.sql`, jamais modifiés après commit — on ajoute une migration.
Tous les identifiants sont des `uuid` v4 générés côté application, sauf mention contraire.
Toutes les dates sont `timestamptz`, stockées en UTC.

```
users            id, email citext unique, email_verified bool, name, avatar_url,
                 created_at, last_seen_at
identities       id, user_id→users, provider (google|apple), provider_uid,
                 unique(provider, provider_uid)
magic_links      id, email citext, token_hash bytea unique, expires_at, used_at,
                 requested_ip inet
sessions         id uuid (= valeur du cookie), user_id→users, created_at, expires_at,
                 last_seen_at, user_agent, ip_hash bytea
orgs             id, name, slug citext unique, plan (free|pro|team), seats int,
                 stripe_customer_id, stripe_subscription_id, subscription_status,
                 current_period_end, trial_ends_at, created_at
org_members      org_id→orgs, user_id→users, role (owner|admin|member),
                 profile jsonb, created_at, primary key(org_id, user_id)
invites          id, org_id→orgs, email citext, role, token_hash bytea unique,
                 invited_by→users, expires_at, accepted_at
signatures       id, org_id→orgs, owner_user_id→users null, name,
                 doc jsonb, profile jsonb, kind (personal|org_template),
                 public_slug citext unique null, published_render_id→renders null,
                 created_at, updated_at, deleted_at
assets           id, org_id→orgs, kind (image|video|document), filename, content_type,
                 bytes bigint, sha256 bytea, storage_key, width int, height int,
                 created_at, unique(org_id, sha256)
render_jobs      id, signature_id→signatures, status (queued|running|done|failed),
                 attempts int, error text, doc_hash bytea,
                 created_at, started_at, finished_at
renders          id, signature_id→signatures, gif_key, png_key, width, height,
                 frames int, fps int, bytes bigint, doc_hash bytea, created_at
events           id bigserial, signature_id→signatures, kind (open|click),
                 element_id text null, target_host text null,
                 ip_hash bytea, ua_family text, occurred_at
                 -- partitionné par mois si le volume l'exige ; pas avant
stripe_events    id text primary key (evt_...), received_at   -- idempotence webhook
```

Index obligatoires : `signatures(org_id)`, `signatures(public_slug)`,
`events(signature_id, occurred_at desc)`, `render_jobs(status, created_at)`,
`sessions(expires_at)`, `org_members(user_id)`.

### 4.1 Vie privée — non négociable

- Aucune IP en clair : `ip_hash = sha256(ip || SIGLAIR_IP_SALT)`, tronqué à 16 octets.
- `ua_family` = famille de client mail devinée (`gmail`, `outlook`, `apple-mail`, `other`),
  jamais l'User-Agent brut.
- Un pixel de tracking dans un email est une donnée personnelle en RGPD. Le champ
  `orgs.analytics_enabled` (défaut `true`) doit permettre de tout couper, et la suppression
  d'un compte purge `events`. Ne pas court-circuiter.

## 5. Routes HTTP

### 5.1 Public — pas d'authentification, servi aux clients mail

| Méthode | Chemin | Rôle |
|---|---|---|
| GET | `/s/{slug}.gif` | GIF publié. Enregistre un `open`. `Cache-Control: public, max-age=300`. |
| GET | `/s/{slug}.png` | Première frame (repli Outlook legacy). Enregistre un `open`. |
| GET | `/c/{slug}/{element_id}` | 302 vers la cible. Enregistre un `click`. |
| GET | `/health` | `200 {"ok":true}` — liveness. |
| GET | `/ready` | 200 si la DB répond, 503 sinon. |

`/c/...` **doit** résoudre la cible dans `renders.doc` — l'instantané du document au moment
du rendu (migration 0002) — et jamais dans `signatures.doc`, qui est le brouillon en cours
d'édition. Sinon modifier un brouillon change silencieusement la destination de liens déjà
partis dans des emails.
Rediriger vers une URL fournie en query string transformerait Siglair en redirecteur ouvert,
utilisable pour du phishing depuis notre domaine. La cible vient de la base, jamais de la requête.

### 5.2 Auth

| Méthode | Chemin | Rôle |
|---|---|---|
| GET | `/api/auth/{google\|apple}/start` | 302 vers le fournisseur. `state` + PKCE en cookie court. |
| GET/POST | `/api/auth/{google\|apple}/callback` | Apple poste en `form_post`. Crée user+session, 302 vers `/app`. |
| POST | `/api/auth/magic/request` | `{email}` → envoie le lien Resend. **Réponse toujours 204**, même email inconnu. |
| GET | `/api/auth/magic/consume?token=` | Consomme (usage unique), crée la session, 302 `/app`. |
| POST | `/api/auth/logout` | Supprime la session, efface le cookie. |
| GET | `/api/me` | `{user, orgs[], current_org, plan, limits, usage}`. 401 si non connecté. |

Règles dures : `state` vérifié, PKCE obligatoire, `id_token` Apple vérifié contre les JWKS
Apple (signature + `iss` + `aud` + `exp`), token magic stocké **haché** (sha256), usage unique,
TTL 15 min, 5 demandes/heure/email et /IP.

### 5.3 Application (session requise)

```
GET    /api/signatures                    liste de l'org courante
POST   /api/signatures                    {name, doc?, kind?} → 201
GET    /api/signatures/{id}
PATCH  /api/signatures/{id}               {name?, doc?, profile?}
DELETE /api/signatures/{id}               soft delete
POST   /api/signatures/{id}/publish       → crée un render_job, renvoie {slug, job_id}
GET    /api/signatures/{id}/status        → {job: {status, error?}, render?}
GET    /api/signatures/{id}/export?mode=  mode = hosted | freeform | safe → {html}
GET    /api/signatures/{id}/analytics     → séries opens/clicks 30 j + top éléments
POST   /api/signatures/{id}/duplicate

POST   /api/preview                       {doc, profile} → {html}  (aperçu éditeur)

GET    /api/assets                        liste
POST   /api/assets                        multipart, ≤ 10 Mo → {id, url, width, height}
DELETE /api/assets/{id}                   refuse 409 si l'asset est utilisé

GET    /api/orgs/{id}/members
POST   /api/orgs/{id}/invites             {email, role} → email Resend
DELETE /api/orgs/{id}/members/{user_id}
PATCH  /api/orgs/{id}                     {name?, analytics_enabled?}
POST   /api/orgs/{id}/rollout             {template_id} → génère/republie une signature
                                          par membre à partir de son `profile`

POST   /api/billing/checkout              {plan, interval?, seats?} → {url} (Stripe Checkout)
                                          interval = month (défaut) | year ; valeur inconnue → 422
POST   /api/billing/portal                → {url} (Customer Portal)
GET    /api/billing/subscription          → état courant
POST   /api/stripe/webhook                signature vérifiée ; PAS de session
```

### 5.4 Forme des erreurs — identique partout

```json
{ "error": { "code": "quota_exceeded", "message": "Plan Free limité à 1 signature." } }
```

Codes : `unauthorized` 401, `forbidden` 403, `not_found` 404, `validation` 422,
`quota_exceeded` 402, `conflict` 409, `rate_limited` 429, `internal` 500.
Le `message` est affichable tel quel à l'utilisateur, en français. Aucun détail interne
(SQL, chemin, backtrace) ne sort de l'API.

## 6. Plans et quotas — GRILLE DÉFINITIVE

| | Free | Pro | Team |
|---|---|---|---|
| Prix mensuel | 0 € | **7,90 €** | **5,90 € / membre**, min. 3 sièges |
| Prix annuel (2 mois offerts) | — | 79 € | 59 € / membre |
| Signatures | 1 | illimité | illimité |
| **GIF hébergé + URL** | ✅ avec marque | ✅ | ✅ |
| Presets d'animation | 4 de base | les 14 | les 14 |
| Analytics | ❌ | 30 jours | 365 jours |
| Assets | 10 Mo | 500 Mo | 5 Go / org |
| Campagnes datées | ❌ | sur ses signatures | poussées à toute l'équipe |
| Membres, rôles, modèle verrouillé | ❌ | ❌ | ✅ |
| Déploiement en masse (rollout) | ❌ | ❌ | ✅ |
| Marque Siglair dans l'export | ✅ imposée | ❌ | ❌ |

Logique de la grille, à ne pas défaire sans y réfléchir :

- Le verrou Free → Pro est la **marque imposée**, pas l'accès au produit. Free héberge
  une vraie signature animée à son URL : sans ça l'utilisateur gratuit n'a jamais vécu ce
  qu'on lui vend, et chaque email gratuit qui porte la marque est le seul canal viral du
  produit. Une seule signature, complète et parfaitement fonctionnelle.
  (Écart assumé avec la version initiale de ce tableau, qui coupait le GIF hébergé sur
  Free ; `src/plans.rs` fait foi et le dit dans son commentaire.)
- Le verrou Pro → Team côté campagnes est la **portée** (`CampaignScope`), pas un quota :
  Pro programme une bannière sur SES signatures, Team la pousse sur celles de tous les
  membres. C'est cette diffusion qui fait du produit un canal marketing.
- La marque imposée sur Free n'est pas une punition, c'est le canal d'acquisition : chaque
  email gratuit porte le produit vers un public déjà qualifié. C'est le seul canal viral
  gratuit dont dispose ce produit.
- Le verrou Pro → Team est le **multi-utilisateur**, pas un quota. Team est moins cher par
  personne que Pro (remise de volume, standard du secteur) mais démarre à 17,70 € et suit
  la taille du client : une agence de 40 personnes paie 236 €/mois. Un forfait unique lui
  ferait payer le prix d'un trio, or c'est le segment qui a du budget.

Source unique : `src/plans.rs`, une `const` par plan, prix inclus. Le front lit tout via
`/api/me` et `/api/config` et ne code JAMAIS un prix ni un quota en dur — sinon un
changement de tarif demande deux déploiements et finit par diverger de Stripe.

Le quota se vérifie **côté serveur à l'écriture**. Un bouton grisé n'est pas un quota.

Ne jamais afficher une fonctionnalité qui n'existe pas dans le code (le brouillon de landing
mentionnait un « Brand Kit » : il n'est pas construit, donc il ne s'affiche pas). Vendre une
case non implémentée, c'est un remboursement et un avis négatif.

## 6bis. « Colle ton site, récupère ta signature » — le moment magique

C'est la fonctionnalité d'entrée du produit et l'accroche de la vitrine. Elle supprime la
page blanche : l'utilisateur ne part pas d'un modèle vide, il part de **sa marque**.

```
URL  →  extraction déterministe  →  3 propositions  →  choix  →  ÉDITEUR NORMAL
```

Le dernier maillon n'est pas négociable : **l'IA ne remplace pas l'éditeur, elle supprime
la page blanche.** Tout ce qu'elle produit doit être déplaçable, recolorable, réanimable.

### 6bis.1 La règle qui rend la feature sûre : l'IA ne produit aucune géométrie

L'IA ne renvoie **jamais** de HTML, ni de CSS, ni de coordonnées. Elle renvoie une
**recette** : un modèle de base, une palette, des choix d'animation, des libellés de CTA.
C'est `src/ai/compose.rs` — du Rust ordinaire — qui transforme cette recette en `Doc`.

Conséquences, et c'est tout l'intérêt :

- Le résultat est **toujours** un `Doc` valide : impossible de casser la mise en page.
- Le résultat est **toujours** compatible email : il sort de nos modèles éprouvés.
- Aucune injection possible : les libellés sont échappés comme tout autre contenu (§8).
- Le rendu est **reproductible** : même recette = même signature, testable sans appeler l'IA.
- Notre moteur peut s'améliorer sans retoucher un seul prompt.

Une IA à qui l'on demande de produire du HTML de signature produira, tôt ou tard, du HTML
cassé dans l'Outlook d'un client. Une IA à qui l'on demande de choisir parmi douze presets
ne le peut pas.

### 6bis.2 Extraction de marque — déterministe, sans IA

`src/ai/brand.rs`. C'est du parsing, pas de l'intelligence : n'appelle pas un modèle pour ça.

Depuis l'URL, on récupère : nom de marque (`og:site_name`, `<title>`, schema.org), logo
(`apple-touch-icon` → icônes du manifest → `og:image` → favicon, dans cet ordre de qualité),
couleurs dominantes (quantification du logo + `theme-color` + couleurs récurrentes du CSS),
accroche (`og:description`), réseaux sociaux (liens vers linkedin/x/instagram/github),
et la police si elle est déclarée.

**C'est ici que se trouve le plus gros risque de sécurité du produit** : on récupère une URL
fournie par un inconnu, depuis notre serveur. Réutiliser `doc::check_remote_url` (§8) —
ne pas réécrire un second garde SSRF. En plus : plafond de 5 Mo, 10 s de timeout, 3
redirections maximum revalidées **une par une** (un 302 vers `169.254.169.254` contourne un
garde qui ne vérifie que la première URL), et pas de cookies renvoyés.

### 6bis.3 L'appel au modèle — un seul adaptateur, compatible OpenAI

`src/ai/generate.rs`. REST direct via le `reqwest::Client` de `AppState`.

**Fournisseur retenu : xAI (Grok)**, `https://api.x.ai/v1/chat/completions`, dont l'API est
compatible OpenAI. On n'écrit donc **qu'un seul client**, paramétré par trois variables :

```
AI_BASE_URL   # défaut https://api.x.ai/v1
AI_MODEL      # nom exact du modèle — aucune valeur codée en dur
AI_API_KEY    # Bearer
```

Ce n'est pas une abstraction spéculative : c'est la forme réelle de l'API xAI. Le jour où
l'offre gratuite disparaît ou se dégrade, on bascule vers OpenAI, OpenRouter, Groq ou un
modèle local **en changeant deux variables d'environnement**, sans toucher au code. Il n'y
a ni trait, ni enum de fournisseurs, ni deuxième implémentation : un seul chemin.

Requête : `POST {AI_BASE_URL}/chat/completions`, `Authorization: Bearer {AI_API_KEY}`,
`model: {AI_MODEL}`, `messages: [system, user]`, et la sortie contrainte :

```jsonc
"response_format": { "type": "json_schema",
  "json_schema": { "name": "signature_variants", "strict": true, "schema": { ... } } }
```

**Dégradation en deux temps, obligatoire** — on ne connaît pas à l'avance les capacités
exactes du modèle configuré :

1. Tentative avec `response_format: json_schema`.
2. Si le fournisseur le refuse (400 sur `response_format`, ou modèle qui ne le gère pas) :
   nouvelle tentative avec `response_format: {"type":"json_object"}` et le schéma décrit
   **en toutes lettres dans le prompt**, puis désérialisation tolérante.
3. Si la sortie reste inexploitable : `compose::fallback_variants` (§6bis.5).

Chaque étape est plus faible que la précédente, mais aucune ne casse l'inscription.
C'est ce qui permet de brancher n'importe quel modèle sans audit préalable.

Autres règles :

- Un **seul appel** pour les trois variantes. Trois appels paieraient trois fois le prompt
  et donneraient des propositions moins différenciées entre elles.
- `temperature: 0.8` est ici **autorisé et utile** (l'API xAI l'accepte, contrairement à
  l'API Anthropic récente) — mais la diversité des trois directions doit venir d'abord du
  prompt, qui les nomme et les oppose explicitement. Le sampling n'est qu'un appoint.
- `max_tokens` généreux (8000) : une sortie tronquée est un JSON invalide, donc un repli.
- Timeout 30 s, une seule reprise sur 429/5xx en respectant `retry-after`.
- **Le schéma JSON ne porte aucune borne numérique** (`minimum`/`maximum` sont mal
  supportés selon les fournisseurs, et le modèle peut de toute façon les ignorer). Un
  `duration: 9999` est une réponse *plausible* : c'est `compose.rs` qui ramène en borne,
  jamais le validateur qui rejette.
- La clé n'apparaît dans aucun log, aucune trace, aucun message d'erreur renvoyé au client.

#### Aucune donnée personnelle n'est envoyée au fournisseur — règle dure

Le modèle reçoit **uniquement la marque extraite** : nom de l'entreprise, couleurs,
accroche publique, type de secteur. C'est de l'information déjà publique sur le site
analysé.

Il ne reçoit **jamais** le profil : ni nom, ni email, ni téléphone, ni LinkedIn. Il n'en a
aucun besoin — il choisit un modèle, une palette, une animation et des libellés de CTA ;
c'est `compose.rs` qui injecte les coordonnées ensuite, localement.

Ce n'est pas de la prudence excessive, c'est trois gains d'un coup : le prompt est plus
court et moins cher, aucune donnée personnelle ne quitte l'infrastructure, et la
sous-traitance au sens RGPD porte sur des données publiques plutôt que sur les
coordonnées de tous les clients.

Un fournisseur d'IA gratuit l'est rarement sans contrepartie, et cette contrepartie est
souvent le droit d'entraîner sur les requêtes envoyées. En n'envoyant que des données déjà
publiques, la question devient sans objet.

**À faire quand même avant l'ouverture commerciale** : le fournisseur d'IA effectivement
retenu doit être nommé comme sous-traitant dans la politique de confidentialité, avec sa
localisation. Un sous-traitant non déclaré est un manquement, même sur données publiques.

**Risque assumé et tracé ici** : bâtir l'entonnoir d'acquisition sur une offre gratuite de
tiers est un pari. Il est acceptable uniquement parce que les trois garde-fous existent —
bascule de fournisseur par variable d'environnement, aucune donnée personnelle exposée, et
repli déterministe qui maintient l'inscription fonctionnelle même fournisseur éteint.

### 6bis.4 Forme de la recette — enums fermés uniquement

```jsonc
{ "variants": [ {
  "name": "Corporate",
  "template": "founder-motion",       // enum fermé : nos 4 modèles
  "palette": { "bg": "#0A2540", "surface": "#0E2E4E", "text": "#FFFFFF",
               "muted": "#9DB4CC", "accent": "#635BFF", "accent2": "#00D4FF" },
  "logo":   { "placement": "left",    // left | top
              "shape": "circle",      // circle | rounded | square
              "anim": "glow",         // enum : les 14 presets du §3.2
              "duration": 2.8 },
  "accent_anim": "shimmer",
  "ctas": [ { "label": "Book a demo", "target": "website", "style": "solid" } ],
                                      // target enum : website|linkedin|whatsapp|email|calendar
  "rationale": "Bleu profond et violet de marque, mouvement discret : lisible en B2B."
} ] }   // exactement 3 entrées
```

`rationale` s'affiche sous chaque proposition. Ça n'est pas décoratif : l'utilisateur
comprend que le système a *lu sa marque*, et c'est ce qui rend le moment convaincant.

### 6bis.5 Repli sans clé API — obligatoire

Si `AI_API_KEY` est absent, la fonctionnalité **ne se désactive pas** : le composeur
applique la palette extraite à trois modèles différents et renvoie trois propositions.
Moins finement adaptées, parfaitement utilisables.

Ce repli n'est pas de la charité : il rend `compose.rs` testable sans réseau ni budget, il
garantit un démarrage sans clé tierce (§9), et il évite que la panne d'un fournisseur
casse l'inscription — c'est-à-dire l'entonnoir d'acquisition tout entier.

### 6bis.6 Routes et quota

```
POST /api/onboarding/analyze    {url}              → marque extraite (aperçu avant génération)
POST /api/onboarding/generate   {brand, profile}   → 3 variantes + docs composés
POST /api/onboarding/pick       {variant_index}    → crée la signature, renvoie son id
```

`analyze` est gratuit et sans compte (c'est l'accroche de la vitrine : on colle une URL et
on voit sa marque reconnue avant même de s'inscrire) — mais **strictement limité en débit**
par IP, puisqu'il déclenche une requête sortante.

`generate` exige un compte. Quota : **1 génération gratuite**, puis Pro. Compteur
`orgs.ai_generations_used`, remis à zéro chaque mois pour les plans payants.

| | Free | Pro | Team |
|---|---|---|---|
| Générations IA | 1 au total | 30 / mois | 100 / mois |

C'est l'entonnoir d'activation : on ne fait pas payer pour *essayer* l'IA, on fait payer
celui qui a déjà vu ce qu'elle sait faire de sa marque. Le message d'atteinte de quota le
dit dans ce sens — « Envie d'une autre direction ? » — jamais « quota dépassé ».

### 6bis.7 Parcours recherche d’emploi

`/signature-email-recherche-emploi` est une entrée publique spécialisée, pas un produit séparé.
Elle compose une signature déterministe avec les vrais documents de l’éditeur et les jetons de
profil candidat : métier recherché, CV, LinkedIn, portfolio, GitHub, disponibilité, localisation,
université et diplôme.

```
POST /api/onboarding/job-preview JSON {doc, profile} → {html}
POST /api/onboarding/job-draft   multipart {payload: {doc, profile, name}, cv?: PDF}
                                 → {handoff, source}
```

- Les deux routes sont publiques et limitées par IP. `job-draft` accepte au plus un PDF de 5 Mo.
- `job-preview` utilise le moteur Rust canonique, ne résout aucun média et force la mention Free.
- Le document et le profil sont validés par des listes fermées. Aucune URL autre que HTTP(S),
  aucun `assetId` arbitraire et aucun jeton inconnu ne sont acceptés.
- Le brouillon et l’éventuel PDF expirent après 24 heures si aucun compte ne les réclame.
- Le handoff signé traverse la connexion magic link ou OAuth. Une fois authentifié, le document
  exact est créé dans l’organisation et le PDF temporaire lui est rattaché.
- Le profil candidat et le CV ne sont jamais envoyés au fournisseur d’IA. Ce parcours utilise les
  modèles déterministes du produit et ne consomme pas le quota de générations IA.
- Un PDF lié depuis une signature est public et servi en téléchargement. Le produit doit le dire
  avant l’import.
- Free conserve la mention « Powered by siglair.com ». Les analytics restent soumis au plan :
  aucun historique sur Free, 30 jours sur Pro, 12 mois sur Team.

### 6bis.8 Plus tard, pas maintenant

`acme.com` + un CSV de 34 collaborateurs → 34 signatures cohérentes. La brique existe déjà
(`POST /api/orgs/{id}/rollout`, §5.3) : il ne manquera qu'un import CSV. À faire quand un
vrai client le demande, pas avant.

## 7. Pipeline de rendu

1. `POST /publish` : hash de (doc, profil). Si un `render` existe déjà avec ce `doc_hash`,
   on le republie directement — pas de nouveau job. (Le cas « republier sans avoir rien changé »
   est le plus fréquent.)
2. Sinon insertion `render_jobs (queued)`.
3. Le worker prend un job (`FOR UPDATE SKIP LOCKED`), passe `running`.
4. `html.rs` produit la page ; Chromium l'ouvre en viewport `canvas.width × canvas.height`,
   `deviceScaleFactor: 2`.
5. Capture : `fps = 12`, `durée = min(timelineDuration, 10 s)` → ≤ 120 frames.
   Avant chaque capture, l'horloge d'animation est positionnée via
   `Animation.currentTime` sur toutes les animations — **pas** de `sleep` entre les captures :
   dormir donne un timing irrégulier et un GIF qui saccade.
6. `ffmpeg` : `palettegen` + `paletteuse` (2 passes) → GIF. Frame 0 → PNG.
7. Si le GIF dépasse 1 Mo : retomber à 8 fps, puis réduire les couleurs à 128, puis 64.
   Gmail coupe autour de 1–2 Mo et beaucoup de serveurs d'entreprise sont plus stricts.
8. Insertion `renders` **avec l'instantané `doc` et `profile` de ce qui a été rasterisé**
   (migration 0002), `signatures.published_render_id` mis à jour, job `done`.
   Cet instantané est la source de vérité de `/c/{slug}/{element_id}` : `signatures.doc`
   continue d'évoluer pendant que l'utilisateur édite, et une cible de clic ne doit
   jamais changer dans un email déjà envoyé sans republication explicite.
9. Échec : `attempts += 1`, retour en `queued` avec backoff (30 s, 2 min, 10 min),
   `failed` définitif à 3 tentatives, avec un message d'erreur lisible par l'utilisateur.

Le renderer est **idempotent** : rejouer un job doit produire le même résultat.
Un job `running` depuis plus de 5 minutes est considéré mort et remis en file.

## 8. Sécurité — les points où l'on ne simplifie pas

- **SSRF** : `canvas.bgImage` et toute URL distante sont chargées par Chromium *dans notre
  réseau*. Bloquer par défaut : seuls `http(s)` et les hôtes publics. Refuser explicitement
  `localhost`, `127.0.0.0/8`, `10/8`, `172.16/12`, `192.168/16`, `169.254.169.254` (metadata
  cloud), `::1`, `fc00::/7`. À valider à l'enregistrement **et** au rendu — un DNS peut
  changer de réponse entre les deux.
- **XSS dans l'export** : tout `content` et tout `href` passent par un échappement HTML, et
  les `href` sont limités aux schémas `http https mailto tel`. Un `javascript:` dans une
  signature d'entreprise, c'est du XSS stocké distribué par email.
- **Upload** : type déduit du contenu (magic bytes), jamais de `Content-Type` client. SVG
  refusé (vecteur de XSS). Servi avec `Content-Disposition: inline` et
  `X-Content-Type-Options: nosniff` depuis un domaine/chemin distinct de l'app.
- **Webhook Stripe** : vérifier `Stripe-Signature` (HMAC-SHA256, tolérance 5 min) **avant**
  de désérialiser. Idempotence via `stripe_events`. Ne jamais faire confiance au montant
  ou au plan envoyé par le client : le plan vient de l'objet Stripe.
- **Chromium** : `--no-sandbox` seulement si le conteneur tourne déjà isolé ; préférer
  `--disable-dev-shm-usage`, pas de réseau vers l'hôte, timeout dur de 30 s par job.
- **Cookies** : `Secure` + `HttpOnly` + `SameSite=Lax`, `__Host-` en production.
- **Rate limit** : magic link, checkout, upload, publish. En mémoire par instance suffit
  aujourd'hui ; passer en table PG le jour où il y a deux instances.

## 9. Variables d'environnement

L'utilisateur fournira les vraies valeurs à la fin. Le code doit démarrer sans les clés
tierces et **désactiver proprement** la fonctionnalité correspondante (bouton masqué via
`/api/config`, jamais un crash au boot).

```
DATABASE_URL, APP_URL, PUBLIC_URL, STORAGE_DIR, PORT
SIGLAIR_SECRET_KEY          # 32 octets base64, signature des cookies
SIGLAIR_IP_SALT
GOOGLE_CLIENT_ID, GOOGLE_CLIENT_SECRET
APPLE_CLIENT_ID, APPLE_TEAM_ID, APPLE_KEY_ID, APPLE_PRIVATE_KEY   # .p8, PEM
RESEND_API_KEY, RESEND_FROM
STRIPE_SECRET_KEY, STRIPE_WEBHOOK_SECRET,
STRIPE_PRICE_PRO, STRIPE_PRICE_TEAM                # plusieurs ids séparés par une virgule
STRIPE_PRICE_PRO_YEARLY, STRIPE_PRICE_TEAM_YEARLY  # facultatif ; absent → Checkout annuel 501
SIGLAIR_LEGAL_NAME, SIGLAIR_LEGAL_SIREN, SIGLAIR_LEGAL_VAT   # pied de facture, défauts fournis
AI_BASE_URL                  # défaut https://api.x.ai/v1 (xAI Grok, compatible OpenAI)
AI_MODEL                     # nom exact du modèle — AUCUNE valeur par défaut codée en dur
AI_API_KEY                   # absent → repli déterministe du §6bis.5, jamais une panne
CHROME_PATH, FFMPEG_PATH
```

`AI_MODEL` n'a volontairement pas de valeur par défaut : un nom de modèle inventé donne une
404 à la première génération, en production, sur la fonctionnalité d'acquisition. Mieux vaut
un démarrage qui bascule proprement sur le repli déterministe et le dit dans les logs.

Le même trio de variables couvre xAI, OpenAI, OpenRouter, Groq ou un modèle local : ce sont
toutes des API compatibles OpenAI. Changer de fournisseur ne demande aucune modification
de code.

`GET /api/config` (public) renvoie `{google, apple, magic, billing, ai: bool}`
pour que le front n'affiche pas un bouton « Continuer avec Apple » qui renverra une 500.
`ai` reste `true` même sans `AI_API_KEY` : le repli déterministe fonctionne, et la
fonctionnalité ne doit pas disparaître de l'interface.

## 10. Tests attendus

- `cargo test` : rendu HTML (échappement, jetons, presets), garde SSRF, calcul des quotas,
  vérification de signature Stripe, extraction du doc→HTML sur un document connu.
- Un test d'intégration par flux critique, sur une vraie DB éphémère (`sqlx::test`) :
  inscription magic link → création signature → publish → GIF présent → clic compté.
- Front : Vitest sur la réduction d'état de l'éditeur (undo/redo, déplacement, quotas).
- `docker compose up` doit aboutir à un parcours complet réussi, vérifié par un script
  `scripts/smoke.sh` qui échoue avec un code non nul.

Pas de suite de tests exhaustive par fonction. Un test par comportement risqué.


## 11. Croissance — la boucle virale et sa mesure

Le produit n'a qu'un canal d'acquisition gratuit : les signatures gratuites, vues par des
gens qui lisent des signatures. Cette section décrit comment il tourne et comment on le
mesure. Elle n'existe pas pour faire joli : sans mesure, on ne sait pas si le canal marche,
et on optimise au doigt mouillé.

### 11.1 Ce qu'on mesure — six événements, pas cinquante

| Événement | Quand | Question à laquelle il répond |
|---|---|---|
| `signature_generated` | signature créée puis publiée | Combien arrivent au bout de l'éditeur ? |
| `signature_installed` | première ouverture depuis une IP différente du propriétaire | Combien l'ont vraiment collée dans leur client mail ? |
| `powered_by_click` | clic sur le badge d'une signature Free | Combien de destinataires mordent ? |
| `upgrade_started` | session Stripe créée | Combien atteignent la caisse ? |
| `upgrade_completed` | abonnement actif | Combien paient ? |
| `team_invite` | invitation envoyée | La boucle interne d'une organisation tourne-t-elle ? |

**On n'en ajoute pas d'autres sans une question précise à laquelle ils répondent.**
« Tracker tout ce qui est possible » a un coût réel : chaque événement est une donnée à
conserver, à sécuriser, à purger sur demande RGPD et à justifier — pour un signal que
personne ne lira. Quarante événements ne valent pas mieux que six : ils les noient.

**Les ouvertures ne sont PAS une métrique de pilotage.** Apple Mail Privacy Protection
précharge les images depuis ses propres serveurs, et le proxy d'images de Gmail met en
cache : une « ouverture » peut n'être qu'une machine. On continue de les enregistrer parce
qu'elles donnent une tendance, mais aucune décision ne s'appuie dessus. Le tableau de bord
doit le dire à l'utilisateur, sinon on lui vend un chiffre faux.

### 11.2 Le badge — du texte HTML, jamais dans le GIF

```html
<a href="{public_url}/r/{slug}">Powered by siglair.com</a>
```

Il reste **hors de l'image**, et c'est structurant :

- un badge gravé dans le GIF disparaît quand le client mail bloque les images — c'est-à-dire
  exactement quand on a le plus besoin qu'il reste visible ;
- un badge dans l'image n'est pas cliquable séparément : tout le GIF pointerait au même
  endroit, et on perdrait la distinction entre « il a cliqué sur le CTA du client » et
  « il a cliqué sur notre badge » ;
- du texte pèse zéro octet et se rend partout, Outlook 2016 compris.

### 11.3 Attribution — par l'URL, sans aucun cookie

```
signature de X → /r/{slug} → /?ref={code} → /login?ref={code} → users.referred_by
```

Le clic sur `/r/{slug}` est compté côté serveur puis redirigé en 302 vers la landing, en
conservant le paramètre. Le code traverse ensuite la connexion **dans l'URL** et n'est
inscrit en base qu'à la création du compte.

**Aucun cookie n'est posé sur le destinataire.** Ce n'est pas de la coquetterie : le
destinataire d'un e-mail n'est pas notre utilisateur, il n'a rien accepté. Un cookie
d'attribution sur lui relève d'ePrivacy et impose un bandeau de consentement — lequel
ferait chuter le taux de conversion qu'on cherche précisément à mesurer. Le paramètre
d'URL donne la même information, sans bandeau, et se supprime en fermant l'onglet.

Le clic lui-même est journalisé avec un `ip_hash` tronqué et une famille de client
(§4.1), jamais une IP ni un User-Agent brut.

### 11.4 Le tableau de bord de croissance

Une page, une phrase par ligne, la même que celle qu'on veut pouvoir dire à voix haute :

```
Mathis · 870 vues · 14 clics sur le badge · 3 comptes créés · 1 Pro
```

Ordre imposé : **clics et conversions en gros, vues en petit et grisées**, avec la mention
de leur imprécision. Mettre les ouvertures en avant serait donner du crédit au chiffre le
moins fiable du tableau.

Taux affichés : clic / vue (indicatif), création / clic, payant / création. C'est le
dernier qui décide si le canal vaut quelque chose.

### 11.5 Ce qu'on ne fait pas

- Pas de pixel tiers, pas de Google Analytics, pas de Segment. Tout est en première partie,
  dans notre base. C'est moins de travail que de brancher un tiers, et ça supprime la
  question du transfert hors UE.
- Pas d'empreinte de navigateur, pas de recoupement entre sessions d'un destinataire.
- Pas de conservation au-delà de la rétention du plan (§6). La suppression d'un compte
  emporte ses événements de croissance comme le reste (§4.1).

### 11.6 Télémétrie produit — distincte des événements de croissance

Les six événements de `growth_events` restent les signaux métier de référence du canal
viral. `product_events` répond à une autre question : à quel endroit précis du parcours
landing → génération → inscription → édition → publication les utilisateurs bloquent-ils ?

Cette télémétrie est first-party, bornée par une liste fermée d'événements et de propriétés,
et ne contient ni saisie libre, ni contenu de signature, ni prompt IA, ni adresse IP ou
User-Agent brut. Les identifiants navigateur sont des UUID aléatoires, le navigateur et
l'appareil sont réduits à des catégories grossières, et les chemins sensibles sont masqués.
Elle ne fait ni session replay, ni empreinte de navigateur, ni publicité comportementale.

Le navigateur peut la désactiver depuis la page de confidentialité. `Sec-GPC: 1` et
`DNT: 1` sont également honorés par le client et par l'API. Les données sont supprimées
après 13 mois au maximum ; supprimer un compte ou une organisation supprime les événements
authentifiés associés.
