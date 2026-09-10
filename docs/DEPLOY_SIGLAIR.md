# Déployer `agentos-server` sur le VPS de siglair

Ce dépôt était siglair. Le code est maintenant celui d'InternationalAgent
(`agentos-server`) ; l'infrastructure de siglair est restée en place et c'est
délibéré : le dépôt `siglair`, l'image `ghcr.io/mattjeff/siglair`, le répertoire
`/opt/siglair`, le journal `/var/log/siglair-deploy.log` et le domaine
`siglair.com` gardent leurs noms. La clé SSH de déploiement est **à commande
forcée** sur `/opt/siglair/scripts/deploy.sh` : renommer l'un de ces éléments
obligerait à toucher au serveur, et c'est exactement ce qu'on évite.

**`docs/DEPLOY_VPS.md` décrit une AUTRE machine** — le VPS d'orizn, avec nginx
sur l'hôte, le réseau `backend_data` et aucun port publié. Son raisonnement sur
pgvector, sur les variables d'environnement et sur la clé maîtresse vaut
partout ; sa topologie ne vaut pas ici. Ici, c'est Caddy dans un conteneur qui
tient 80/443, et le domaine est public.

---

## 0. Les modifications de cette section sont APPLIQUÉES

Elles ne le furent pas d'emblée : l'agent qui a fait la transposition a perdu le
droit d'écrire sur ce dépôt en cours de route. C'est fait depuis le 2026-08-30,
et la justification de chaque retouche vit désormais en commentaire à côté du
code plutôt qu'en diff ici — un diff recopié dans un document est une deuxième
version qui diverge.

- `Dockerfile`, étape `runtime` : base `node:22-bookworm-slim` (le CLI `claude`
  est un paquet npm, et c'est le même bookworm que l'étape de build, donc la
  même glibc) ; `@anthropic-ai/claude-code@2.1.231` **épinglé** — `llm_cli.rs`
  analyse `stream-json` et une version flottante est `cli_bad_output` en
  production ; et le compte `agentos` a maintenant un HOME (`--create-home`,
  `/home/agentos`), sans quoi le CLI ne peut pas être connecté du tout.
- `compose.yml` : volume nommé `claude_home` monté sur `/home/agentos`, sinon
  chaque recréation de conteneur déconnecte tous les employés.
- Cosmétique : `ci.yml` ne se déclenche plus sur `push: main` (deux fois la même
  recette à chaque fusion), `.dockerignore` exclut `compose.yml` et `docker/`,
  `.gitignore` couvre `.DS_Store` et `*.log`.

---

## 1. Le modèle : le CLI Claude, pas une clé d'API

`AGENTOS_LLM=cli` sélectionne `crates/providers/src/llm_cli.rs`. **Lire l'en-tête
de ce fichier avant de toucher à quoi que ce soit ici** : il documente chaque
drapeau, avec la mesure qui l'a justifié.

Ce que ça implique, dans l'ordre d'importance opérationnelle.

### 1.1 L'authentification ne se copie pas, elle se refait à la main

Sur le Mac, le CLI garde ses identifiants dans le trousseau macOS. **Il n'y a pas
de trousseau dans un conteneur Linux** : sous Linux le CLI écrit son état dans
`$HOME` (`~/.claude/`, dont le fichier d'identifiants, et `~/.claude.json`).
Les identifiants du Mac ne sont donc ni copiables ni transposables.

La connexion se fait **une fois**, à la main, après le premier démarrage :

```bash
cd /opt/siglair
docker compose exec -it api claude          # suivre la connexion interactive
docker compose exec -T  api claude -p 'dis OK'   # vérification, doit répondre
```

Le volume `claude_home` (§0.2) est ce qui fait que cette connexion survit à un
`docker compose up -d`, à un redéploiement et à un retour arrière. Sans lui, tout
redéploiement déconnecte les employés.

`--bare` ferait presque tout ce que font les cinq drapeaux du module, en un seul
— et il est **rejeté** exprès : il force l'authentification par
`ANTHROPIC_API_KEY` ou `apiKeyHelper`, « OAuth et trousseau ne sont jamais lus ».
Tourner sans clé d'API est la raison d'être de ce backend.

### 1.2 Le jeton expire, et c'est le risque numéro un de ce déploiement

**Un jeton d'abonnement qui expire arrête tous les employés à la fois.** Le
symptôme, côté console, est une flotte dont chaque tour échoue sans que personne
n'ait rien changé et sans qu'aucun déploiement n'ait eu lieu.

Le code d'erreur remonté est **`cli_failed`** (`ProviderError::Terminal`), ou
`cli_error` si le CLI a quand même émis un événement `result`. Il devient un
`TurnError::Llm` dans `app::turn::Turn`, et se lit à deux endroits :

```bash
docker compose logs api | grep -E 'cli_failed|cli_error|cli_spawn_failed'
# et, par l'API, tour par tour :
GET /v1/employees/{id}/turns
```

**La cause exacte est dans les journaux**, depuis le 2026-08-30 seulement : le
CLI tournait sous `.stderr(Stdio::null())` et le message qui dirait « session
expirée, reconnectez-vous » était jeté. `llm_cli.rs` capture maintenant la
stderr et la journalise quand le processus sort non nul en ayant écrit quelque
chose — le tour qui appelle un outil sort 1 lui aussi mais ne dit rien sur
stderr, donc le chemin normal reste silencieux.

```bash
docker compose logs api | grep 'wrote to stderr'
# et, si le doute persiste, le CLI à la main :
docker compose exec -T api claude -p 'dis OK'
```

Le remède est la reconnexion de §1.1. Aucun redéploiement ne la remplace : le
jeton est dans le volume, pas dans l'image.

Les autres codes du même adaptateur, pour lever l'ambiguïté au moment où ça
compte : `cli_spawn_failed` = pas de binaire `claude` sur le PATH (base sans
Node, donc §0.1 défait) ; `cli_bad_output` / `cli_no_result`
= le flux ne se lit plus, donc une version du CLI qui a bougé sous le parseur ;
`cli_mcp_bind_failed` = pas de port loopback disponible dans le conteneur.

### 1.3 Le quota est hebdomadaire et il est partagé

Le CLI tire sur **l'abonnement du fondateur**, pas sur un compteur dédié au
serveur. Le quota est hebdomadaire et **partagé avec son usage interactif de
Claude Code** : une semaine chargée sur le portable est une semaine où la flotte
s'arrête, et l'inverse est vrai aussi. Ce n'est pas une surprise à découvrir un
vendredi — c'est la contrainte de capacité de ce déploiement, à surveiller comme
on surveillerait un solde.

Les événements `rate_limit_event` du flux existent et **cet adaptateur ne les lit
pas** : une limite atteinte ressort comme les autres, en `cli_failed`.

### 1.4 Ce que ce backend n'est pas

Son propre en-tête le dit : « **testing only** ». Le chemin de production est
`llm_anthropic`, un client direct de `POST /v1/messages`. Les écarts qui comptent
en exploitation : le cache de préfixe n'est pas le nôtre, `max_tokens` borne un
fragment et pas l'appel, l'historique des appels d'outils repart en prose à
chaque tour, et `total_cost_usd` est lu puis jeté — donc les chiffres de coût de
ce backend ne ressemblent pas à ceux de la production. C'est un choix assumé du
fondateur, pas un défaut de la transposition.

---

## 2. Les variables d'environnement

Source unique : `apps/server/src/config.rs`, et le fichier le dit lui-même. Rien
d'autre dans le binaire n'appelle `std::env::var`. **Une variable exportée vide
compte comme absente.**

Elles vont dans `/opt/siglair/.env`, en mode 600, jamais dans git.

### Obligatoires — le processus refuse de démarrer sans

| Variable | Valeur ici |
|---|---|
| `DATABASE_URL` | posée par `compose.yml`, pas par `.env` |
| `PUBLIC_HOST` | `https://siglair.com` — **avec le schéma** |
| `AGENT_EMAIL_DOMAIN` | le domaine d'envoi vérifié chez Resend |
| `AGENTOS_MASTER_KEY` | `openssl rand -base64 32`, **générée une fois** |
| `AGENTOS_ALLOW_MOCKS=1` | obligatoire tant qu'un seul adaptateur est un mock |

`PUBLIC_HOST` est interpolée dans les cartes d'agent A2A
(`{PUBLIC_HOST}/a2a/jsonrpc?employee=…`, sans rien y préfixer) et dans la
vérification de signature des webhooks Twilio. Une valeur qui ne correspond pas
à ce qui a été collé dans la console du fournisseur répond 401 à chaque message
authentique.

> ### **`AGENTOS_MASTER_KEY` — à lire avant de la générer**
>
> **La changer plus tard orpheline toute identité déjà émise.** Chaque clé
> privée Ed25519 d'employé est scellée dessous
> (`employee_signing_keys.sealed_private_key`), ainsi que chaque identifiant MCP
> de tenant depuis `0040_mcp_credentials`. `UPDATE` est révoqué sur cette table
> et **il n'existe aucun chemin de rotation** : la récupération consiste à
> supprimer les lignes et à re-provisionner l'identité, ce qui change tous les
> `kid` publiés.
>
> **Elle n'est pas couverte par un `pg_dump`.** Elle se sauvegarde avec la base,
> au même endroit et avec le même soin. Une base restaurée sans sa clé donne
> toutes les clés publiques et aucun moyen de signer avec.
>
> Elle est validée « non vide » et rien de plus : ni décodée en hexadécimal, ni
> contrôlée en longueur, contrairement à ce que `.env.example` laisse croire.
> Elle est portée à 32 octets par SHA-256 et non par une KDF — l'entropie doit
> donc venir de la valeur elle-même.

### Le modèle

| Variable | Valeur ici |
|---|---|
| `AGENTOS_LLM` | **`cli`** — décision du fondateur. `mock`, `anthropic` et `cli` sont les seules valeurs ; une valeur inconnue est un refus au boot qui les liste |
| `ANTHROPIC_API_KEY` | **non posée.** Elle n'est exigée au boot que si `AGENTOS_LLM=anthropic` |

### Optionnelles — ce que coûte le vide

| Variable | Défaut | Conséquence si vide |
|---|---|---|
| `APP_BIND` | `0.0.0.0:8080` | posée par `compose.yml`. `.env.example` et le README disent 8090 : non défini vaut **8080** |
| `RUST_LOG` | `info,agentos_server=debug` | JSON sur stdout dans les deux cas |
| `AGENTOS_API_KEYS` | vide | `label:tenant-uuid:secret[,…]`, secret ≥ 32 caractères. Vide **et** sans ligne dans `api_keys` = tout est 401, et rien ne peut l'y changer |
| `AGENTOS_PLATFORM_KEYS` | vide | `label:secret`, sans uuid, délibérément. Vide = `/v1/platform/*` répond 401 à tout le monde — **et c'est le seul chemin qui crée un tenant** |
| `AGENTOS_WEBHOOK_SECRETS` | vide | tout `/v1/webhooks/{path}` est un 404. **Un seul tenant par provider** : deux entrées sur un même chemin est un refus au boot. C'est aussi ici, dans l'entrée `email`, que vit le `whsec_…` de Resend |
| `AGENTOS_OAUTH_CLIENTS` | vide | aucun connecteur annoncé par `GET /v1/mcp/catalog` |
| `EMAIL_API_KEY` | vide → `MockEmailProvider` | présente = vrai client Resend |
| `TELEPHONY_API_KEY` | vide → `MockTelephony` | format `ACxxxx:auth_token` ; **une moitié seule est un refus au boot nommé** |
| `BROWSER_API_KEY` | vide → `MockBrowser` | format `project-id:api-key`, même refus sur une moitié |
| `BROWSER_CDP_URL` | vide → `BROWSER_FETCH` ou le faux | **`http://browser:9222`** : le Chromium du service `browser` (§5). Une clé Browserbase l'emporte |
| `EMBEDDER_API_KEY` | vide → hash SHA-256 `mock-sha256-1536` | présente = `OpenAiEmbedder`. **La poser ne ré-embarque pas l'existant** : un corpus ingéré sur le hash cesse d'être trouvable jusqu'à réingestion |
| `MCP_BRIDGE_BIND` | non défini | **hébergement MCP éteint. À laisser non défini** : c'est un conteneur par slug qu'un tenant peut inventer, sur cette machine-ci |
| `MCP_BRIDGES_PER_TENANT` / `MCP_BRIDGE_IMAGE` | `0` / `node:22-alpine` | sans effet tant que `MCP_BRIDGE_BIND` est vide |

Le coffre à secrets d'employé (`secrets=MOCK(in-memory)`) est un mock permanent
qu'aucune variable ne corrige. Il est nommé dans **chaque** ligne de boot.

### Lues par docker compose, pas par le binaire

`POSTGRES_PASSWORD` (obligatoire en production — `openssl rand -hex 24`),
`IMAGE_REPO=ghcr.io/mattjeff/siglair`, `IMAGE_TAG=latest`,
`SITE_ADDRESS=siglair.com`, `SITE_WWW_ADDRESS=www.siglair.com`,
`SITE_CANONICAL_URL=https://siglair.com`. `BIND_ADDR`, `API_PORT` et
`POSTGRES_PORT` gardent leurs défauts (loopback, 8080, 5442).

---

## 3. Ce qui est public, et ce qui ne doit pas l'être

`siglair.com` est derrière Cloudflare, donc l'API est joignable publiquement.
`docker/Caddyfile` renvoie **404** sur trois chemins et relaie tout le reste.

**Fermé, et il faut que ça le reste** — ils sont hors de la pile
d'authentification par construction (SPEC.md §20), donc rien d'autre ne les
protège :

| | ce que ça publie à qui le lit |
|---|---|
| `/metrics` | le mélange des raisons de refus, la profondeur de la file d'approbation |
| `/readyz` | le retard de l'outbox, la liste des adaptateurs en mock |
| `/livez` | peu de chose, mais rien qui ait à être joignable de dehors |

**Ouvert par nécessité**, sans credential et il n'y a pas d'alternative :
`POST /v1/webhooks/{path}` (un fournisseur signe, il n'a pas de clé à nous),
`GET /v1/mcp/oauth/callback` (c'est un navigateur qui revient),
`GET /.well-known/agent-card.json` et
`GET /.well-known/http-message-signatures-directory` (un vérificateur qui n'a
jamais entendu parler de nous n'a rien pour s'authentifier, et une clé que
personne ne peut lire ne vérifie rien).

**Ouvert derrière `Authorization: Bearer`** : tout le reste de `/v1/*` et
`/a2a/jsonrpc`. C'est ce que `orizn-web` appellera.

**La limite de débit est à poser dans Cloudflare, pas ici.** Celle du serveur
est de 600 requêtes par tenant et par minute, en mémoire du processus, et la
chaîne de middleware est `auth → rate limit` : le trafic **anonyme** sur les
quatre chemins ci-dessus, et sur les 401, n'est donc limité par rien. Deux
plafonds connus en plus : rafale possible à 2× à cheval sur deux fenêtres, et
budget par réplique et non par grappe.

---

## 4. Premier déploiement, dans l'ordre

Rien de ce qui suit n'a été exécuté.

1. **Fusionner la transposition dans `main`** — §0 est appliqué — et laisser la
   CI construire et publier l'image. Le job `build` la démarre et vérifie que
   son `doctor` refuse proprement : c'est la seule preuve qu'on ait qu'elle
   démarre. **Fusionner ne suffit pas** : le workflow ne synchronise aucun
   fichier, il lance `/opt/siglair/scripts/deploy.sh` par SSH, et ce script,
   ce `compose.yml` et ce Caddyfile sont encore ceux de l'ancien siglair tant
   que l'étape 3 n'est pas faite. Un déploiement lancé avant elle recrée
   l'ancienne pile avec la nouvelle image.
2. **Rendre le paquet `ghcr.io/mattjeff/siglair` accessible** au serveur. Il
   était déjà tiré par ce VPS sous l'ancien code, donc l'authentification est
   probablement en place ; le premier `pull` le dira.
3. **Copier à la main dans `/opt/siglair`** : `compose.yml`, `docker/Caddyfile`,
   `scripts/deploy.sh`. Le workflow ne synchronise rien — il ne fait que lancer
   `deploy.sh` par SSH. Vérifier `jq` sur la machine : `deploy.sh` en dépend.
4. **Réécrire `/opt/siglair/.env`** (§2, et `BROWSER_CDP_URL=http://browser:9222`
   de §5), mode 600. L'ancien contient les
   secrets de siglair et **aucune** des variables ci-dessus.
5. **Vérifier le disque** avant le premier démarrage : 69 fichiers de migration
   s'appliquent au boot, dont une extension et un index HNSW.
6. `docker compose --profile proxy up -d`. Le premier démarrage est long : les
   migrations passent avant que le listener ne se lie, d'où les 120 s d'attente
   de `deploy.sh`.
7. **Connecter le CLI** (§1.1). Tant que ce n'est pas fait, chaque tour échoue.
8. **Installer le plafond de politique.** Sans lui le portail refuse **toute**
   action pour **tout** tenant, et `/readyz` reste 503 `no_platform_policy` :

   ```bash
   docker compose exec api agentos-server policy install
   ```

   Il ne lit que `DATABASE_URL`. Idempotent, pas de redémarrage. Les valeurs par
   défaut (500 $ par transaction, 100 $ de seuil d'approbation, 2 000 $ par
   jour, canaux `email, internal, web`, aucun domaine et aucun outil MCP
   autorisés) sont un **plafond**, pas une recommandation.
9. **Créer le premier tenant** : `POST /v1/platform/tenants` derrière
   `AGENTOS_PLATFORM_KEYS`, qui émet aussi sa première clé d'API. Aucun endpoint
   autorisé par une clé de tenant ne crée un tenant. En ligne de commande :
   `agentos-server policy new-tenant`.
10. **Vérifier depuis dehors** : `/livez`, `/readyz` et `/metrics` doivent
    répondre **404** sur `https://siglair.com`, et `/v1/whoami` sans en-tête
    doit répondre 401.

### L'ancien volume

`siglair_pgdata` contient le cluster PostgreSQL **16** de l'ancien siglair.
`compose.yml` ne le réutilise pas — il déclare `agentos_pgdata`, monté sur
`/var/lib/postgresql` et non `.../data`, parce que pg18 refuse l'ancien chemin
(docker-library/postgres#1259). Le laisser en place ne coûte que du disque ; le
supprimer est la décision du fondateur, et elle est irréversible.

`--remove-orphans` dans `deploy.sh` supprimera au premier passage les conteneurs
`renderer` et `web`, qui n'existent plus dans `compose.yml`.

### Les sauvegardes

Rien ne sauvegarde le nouveau volume. Toute la durabilité du système est cette
base — pièces jointes comprises, elles sont en `bytea` dans `files`, pas dans un
bucket. Qui lance le `pg_dump`, à quelle fréquence, où atterrit-il — et
`AGENTOS_MASTER_KEY` se sauvegarde avec, sous peine d'avoir une base qu'aucune
identité ne peut plus signer. Le volume `claude_home` mérite le même traitement :
il porte la connexion du CLI, qui ne se reconstruit pas depuis l'image.

---

## 5. Le navigateur

Le service `browser` de `compose.yml` est un Chromium sans tête
(`chromedp/headless-shell`, **épinglé par digest**) qui parle CDP. C'est « notre
Browserbase » : `api` y ouvre un contexte par tâche, y rejoue les `BrowserStep`
avec le pilote CDP qu'il utilisait déjà contre Browserbase, et scelle le pot de
cookies de l'employé en base à la fin. Le contrat complet — sémaphore à trois
onglets, cookies scellés, `blocked_by_site` — est `docs/BROWSER.md` côté
InternationalAgent ; ici il n'y a que le conteneur.

**Aucun port publié, et il faut que ça le reste.** Le port CDP n'a pas
d'authentification : qui le joint pilote un navigateur qui sort sur Internet
avec les cookies des employés. Il n'est joignable que par nom (`browser:9222`)
sur le réseau des conteneurs, donc depuis `api`. Dans l'autre sens, Chromium est
lancé avec `--host-resolver-rules` qui envoie `db`, `api`, `web`, `caddy` et
`localhost` sur `~NOTFOUND` : un script de page ne peut pas frapper la base par
le navigateur. `scripts/check-compose.sh`, lancé par la CI, échoue si l'une de
ces deux propriétés disparaît de `compose.yml`.

**La mémoire.** `mem_limit: 1g` sur les ~2,9 Go que laissent `db`, `api` et
`web` : ≈ 400 Mo de base et 100-150 Mo par onglet, d'où trois onglets au plus
côté adaptateur (`BROWSER_MAX_TABS`, défaut 3 ; au-delà on attend
`BROWSER_QUEUE_WAIT`, défaut 60 s, puis l'étape est `Retryable`). Dépasser la
limite tue un renderer, pas la base. `shm_size: 256m` et
`--disable-dev-shm-usage` parce qu'un Chromium à court de `/dev/shm` plante en
silence.

**Ce que l'image contient**, relevé couche par couche : Debian trixie-slim avec
`bash`, `dash` et `grep`, plus `socat` — **ni `curl` ni `wget`**. Et un détail
qui fixe la forme de tout le reste : `headless_shell` n'écoute que sur
`127.0.0.1`, `--remote-debugging-address` est ignoré. C'est `socat` qui expose
9222 et relaie vers 9223. Le `run.sh` de l'image passe ses arguments sans
guillemets, ce qui éclaterait `--host-resolver-rules` en cinq mots ; d'où
l'`entrypoint` réécrit dans `compose.yml`, qui fait la même chose avec `"$@"`
quoté.

**L'en-tête `Host`.** Chromium répond `500 Host header is specified and is not
an IP address or localhost` à toute requête dont le `Host` n'est ni une IP ni
`localhost`, et il recopie ce `Host` tel quel dans le `webSocketDebuggerUrl`
qu'il renvoie. Un client qui envoie `Host: browser:9222` est donc refusé :
l'adaptateur doit résoudre `browser` et envoyer l'IP (c'est ce que fait
chromedp). Le healthcheck parle HTTP/1.1 avec `Host: 127.0.0.1:9222` (Chromium refuse HTTP/1.0, mesuré le 2026-09-10).

### Vérifier

```bash
docker compose ps browser                       # (healthy)
# ce que fait le healthcheck, à la main : la chaîne socat → Chromium
docker compose exec browser bash -c 'exec 3<>/dev/tcp/127.0.0.1/9222; printf "GET /json/version HTTP/1.1\r\nHost: 127.0.0.1:9222\r\nConnection: close\r\n\r\n" >&3; cat <&3'
# depuis la place d'`api` — son image n'a ni curl ni wget, mais elle a node.
# Host en IP, sinon 500 (voir ci-dessus) :
docker compose exec api node -e "http.get({host:'browser',port:9222,path:'/json/version',headers:{Host:'127.0.0.1'}},r=>r.pipe(process.stdout))"
# et côté serveur : browser_js vrai, et `browser` absent de mock_adapters
curl -sS http://127.0.0.1:8080/readyz
```

### Couper

Retirer `BROWSER_CDP_URL` de `/opt/siglair/.env` et recréer `api` :

```bash
docker compose up -d api
```

Le conteneur `browser` peut rester : sans la variable, `api` retombe sur
`BROWSER_FETCH=http` ou sur le faux, et ne lui parle plus. `api` dépend de lui
(`condition: service_healthy`), donc l'arrêter tout à fait demande de retirer
aussi la dépendance dans `compose.yml` — et `scripts/check-compose.sh` le
refusera, exprès.

### Mettre à jour

Relever le digest de l'index (`docker manifest inspect
chromedp/headless-shell:stable`, ou `docker buildx imagetools inspect`), le
remplacer dans `compose.yml`, copier le fichier dans `/opt/siglair`, puis
`docker compose up -d` : une référence par digest qui n'est pas en local est
tirée. `scripts/deploy.sh` ne fait `pull` que d'`api` et `web`, et n'a pas besoin
de plus.
