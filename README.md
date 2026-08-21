# Siglair

Signatures email animées hébergées.

L'utilisateur compose sa signature dans un éditeur, publie, et colle dans Gmail ou Outlook
un simple `<img src="https://siglair.com/s/{slug}.gif">`. Le serveur rend le GIF, le sert,
compte les ouvertures, et redirige les clics en les comptant. Changer sa signature = republier,
sans jamais recoller de HTML dans le client mail.

Le produit n'est pas l'éditeur, c'est l'URL hébergée. Le contrat technique complet —
schéma, routes, quotas, règles de sécurité — est dans **[docs/CONTRACT.md](docs/CONTRACT.md)**.
En cas de désaccord entre ce README et le contrat, le contrat gagne.

## Architecture

```
web/            React 19 + Vite + TypeScript          →  SPA, servie par nginx
src/bin/api     axum : /api, /s/{slug}.gif, /c/{slug}/{el}, /f/{key}
src/bin/renderer  worker : dépile render_jobs, Chromium + ffmpeg → GIF + PNG
src/render/html.rs  UNE seule implémentation doc → HTML (export, aperçu, capture)
PostgreSQL 16   tout l'état, y compris la file de rendu (FOR UPDATE SKIP LOCKED)
STORAGE_DIR     disque partagé api ↔ renderer, derrière le trait Storage
```

Un seul crate Rust, deux binaires, une seule image Docker : ils partagent la lib et la
stack de rendu, les construire séparément doublerait le temps de build pour rien.
Pas de Redis, pas de S3, pas de broker : à trois rendus par minute, une table Postgres
et un volume suffisent. On changera le jour où il y aura deux machines.

## Démarrage local

```bash
cp .env.example .env && openssl rand -base64 32   # collez la clé dans SIGLAIR_SECRET_KEY
docker compose up -d --build
scripts/smoke.sh
```

`smoke.sh` déroule le parcours réel : connexion, création, publication, téléchargement du
GIF (en-tête `GIF89a` et taille vérifiés), clic redirigé, événements relus en base. Il sort
en code non nul au premier écart. C'est le seul verdict qui compte.

Ensuite : API sur <http://localhost:8080>, front sur <http://localhost:3000>.

Pour développer avec rechargement à chaud (Postgres en Docker, Rust en natif) :

```bash
scripts/dev.sh          # api + renderer, cargo watch si installé
cd web && npm run dev   # front sur :5173, proxy /api /s /c vers :8080
```

Il faut alors `chromium` et `ffmpeg` sur la machine, et `CHROME_PATH` renseigné dans `.env`.

## Clés tierces

Aucune n'est obligatoire. Le serveur démarre sans, `GET /api/config` le signale, et le front
masque le bouton correspondant. On branche les clés une par une, quand on en a besoin.

| Variable(s) | Ce que ça active | Où l'obtenir |
|---|---|---|
| `SIGLAIR_SECRET_KEY` | **obligatoire** — signature des cookies | `openssl rand -base64 32` |
| `GOOGLE_CLIENT_ID` / `_SECRET` | « Continuer avec Google » | <https://console.cloud.google.com/apis/credentials> |
| `APPLE_*` | « Continuer avec Apple » (exige HTTPS, donc pas en local) | <https://developer.apple.com/account/resources/identifiers/list/serviceId> |
| `BREVO_API_KEY` / `BREVO_FROM` | lien magique, invitations | <https://app.brevo.com/settings/keys/api> |
| `BREVO_LIST_ID` | inscrits poussés dans une liste marketing Brevo | <https://app.brevo.com/contact/list-listing> |
| `STRIPE_*` | abonnements Pro et Team | <https://dashboard.stripe.com/apikeys> |
| `AI_MODEL` / `AI_API_KEY` (+ `AI_BASE_URL`) | affine « colle ton site » (repli déterministe sans clé) | <https://console.x.ai> |

**La procédure détaillée est dans [.env.example](.env.example)** : pour chaque clé, la page
exacte, les champs à remplir, les URI de redirection au caractère près et les événements de
webhook à cocher. Suivez ce fichier de haut en bas, il n'y a rien à deviner ailleurs.

## Déploiement sur une VM

Une machine à 2 vCPU / 4 Go suffit largement au démarrage — Chromium est le seul poste
gourmand et il ne tourne que pendant les captures.

```bash
# sur la VM, avec Docker installé
git clone <dépôt> siglair && cd siglair
cp .env.example .env
```

Dans `.env` :

```
SIGLAIR_SECRET_KEY=<openssl rand -base64 32>
SIGLAIR_IP_SALT=<openssl rand -hex 16>
APP_URL=https://siglair.com
PUBLIC_URL=https://siglair.com
SITE_ADDRESS=siglair.com
# En production Docker, la base PostgreSQL est le service local `db` du compose.
SIGLAIR_DATABASE_URL=postgres://siglair:<mot-de-passe>@db:5432/siglair
```

Faites pointer l'enregistrement DNS `A` du domaine vers l'IP de la VM, ouvrez 80 et 443,
puis :

```bash
docker compose --profile proxy up -d --build
API=https://siglair.com scripts/smoke.sh
```

Caddy obtient le certificat Let's Encrypt tout seul au premier appel — c'est pour ça que le
DNS doit être en place avant. Il route `/api`, `/s`, `/c`, `/f`, `/health` et `/ready` vers
l'API, et tout le reste vers le SPA.

Ensuite seulement, revenez brancher les clés tierces (`.env`, puis
`docker compose up -d`), y compris l'URL du webhook Stripe qui doit pointer vers
`https://siglair.com/api/stripe/webhook`.

Sauvegardes : le volume `siglair_pgdata` (la base) et `siglair_storage` (les GIF rendus).
Le second est reconstructible en republiant, le premier ne l'est pas.

```bash
docker compose exec -T db pg_dump -U siglair siglair | gzip > siglair-$(date +%F).sql.gz
```

## Ce qui reste à faire

- **Migrations en production** : elles sont appliquées par l'API au démarrage. Correct à une
  instance, à revoir le jour où il y en a deux.
- **Rate limiting en mémoire** : compté par instance (contrat §8). À passer en table Postgres
  au moment où l'API est répliquée.
- **`events` non partitionné** : à découper par mois quand le volume l'exigera, pas avant.
- **Stockage disque** : passer à S3 le jour où il y a deux machines, l'interface `Storage`
  est déjà là pour ça.
- **Import CSV pour le rollout d'équipe** : la route existe déjà, il ne manque que le parsing.
  À faire quand un vrai client le demande.
- **Sauvegardes automatiques** : la commande `pg_dump` ci-dessus n'est pas encore planifiée.
