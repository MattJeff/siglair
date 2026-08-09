# Mise en ligne — une machine, une heure

Tout tient sur un seul serveur : Caddy termine le TLS, sert le front et relaie vers l'API.
Même origine pour le navigateur, donc **aucun CORS, aucun cookie cross-site, aucun stockage
objet à configurer**. C'est la raison de ce choix, pas une simplification par paresse.

Testé de bout en bout en local : GIF animé réellement produit par Chromium, ouvertures et
clics comptés. Cf. `scripts/smoke.sh`, qui est le juge du projet.

---

## Avant de commencer

| | |
|---|---|
| Serveur | Hetzner CX22 — 2 vCPU, 4 Go, 40 Go, image **Docker CE** (Ubuntu 24.04) |
| Domaine | un nom que vous possédez, DNS géré chez Cloudflare ou ailleurs |
| Clé SSH | ajoutée à la création du serveur |

**4 Go n'est pas du confort.** Chromium prend ~1 Go pendant une capture, plus 1 Go de
`/dev/shm` réservé dans `compose.yml`. Sur 2 Go, le premier GIF un peu grand déclenche un
OOM kill et l'utilisateur voit « publication échouée » sans comprendre pourquoi.

**Pas d'Arm64 pour un premier déploiement.** Même prix, mêmes caractéristiques, mais
Chromium headless y est un chemin moins battu. Ce n'est pas le soir de la mise en ligne
qu'on veut découvrir ça.

---

## 1. DNS

Un enregistrement `A` vers l'IPv4 du serveur, et `AAAA` vers l'IPv6 si vous en avez une.

**Chez Cloudflare : nuage GRIS (DNS only), pas orange.** Avec le proxy activé, Caddy ne peut
pas obtenir son certificat Let's Encrypt par HTTP-01 et le déploiement s'arrête là. Le proxy
orange se rallume plus tard, une fois le certificat en place et Cloudflare réglé sur
« Full (strict) ».

Vérifiez la propagation avant d'aller plus loin — sinon vous déboguerez Caddy alors que le
problème est dans le DNS :

```bash
dig +short A votredomaine.fr
```

## 2. Préparer la machine

```bash
ssh root@IP_DU_SERVEUR

# Du swap AVANT le premier build : l'édition de liens de Rust en release dépasse
# facilement 4 Go et se fait tuer par l'OOM, sans message clair.
fallocate -l 4G /swapfile && chmod 600 /swapfile && mkswap /swapfile && swapon /swapfile
echo '/swapfile none swap sw 0 0' >> /etc/fstab

docker compose version   # doit répondre v2.x
apt-get install -y jq    # requis par scripts/smoke.sh
```

Le pare-feu : n'installez rien. `compose.yml` lie déjà Postgres, l'API et le front à
`127.0.0.1` ; seul Caddy écoute sur 80/443. **`ufw` ne protégerait de toute façon pas un
port publié par Docker** — les règles DNAT de Docker sont insérées avant celles d'ufw, et
c'est ainsi que des bases finissent chiffrées par des rançongiciels en quelques heures.

Si vous voulez tout de même un pare-feu, utilisez celui de Hetzner (Cloud Firewall), qui
agit en amont de la machine : autorisez 22, 80, 443, refusez le reste.

## 3. Récupérer le code

```bash
git clone https://github.com/MattJeff/siglair.git /opt/siglair
cd /opt/siglair
```

## 4. Configurer

```bash
cp .env.example .env
openssl rand -base64 32   # → SIGLAIR_SECRET_KEY
openssl rand -hex 16      # → SIGLAIR_IP_SALT
nano .env
```

Le minimum pour démarrer :

```ini
SIGLAIR_SECRET_KEY=<la valeur générée>
SIGLAIR_IP_SALT=<la valeur générée>

SITE_ADDRESS=votredomaine.fr          # sans https:// — Caddy s'occupe du certificat
APP_URL=https://votredomaine.fr
PUBLIC_URL=https://votredomaine.fr    # écrit dans le <img src> collé dans Gmail
```

`PUBLIC_URL` doit être joignable depuis Internet : c'est l'URL que les clients mail
appelleront pour afficher le GIF. Une erreur ici ne se voit pas au déploiement, elle se voit
dans la boîte de réception d'un prospect.

Laissez tout le reste vide pour l'instant. Chaque clé absente désactive proprement sa
fonctionnalité — rien ne plante, le bouton correspondant disparaît simplement.

### Base de données

Par défaut, Postgres tourne dans un conteneur avec un volume : rien à faire, mais les
sauvegardes sont à votre charge.

Pour utiliser Supabase à la place — sauvegardes gérées, et les données survivent à la perte
de la machine — remplissez `SIGLAIR_DATABASE_URL`. **Prenez le pooler en mode session
(port 5432), pas le pooler transactionnel (6543)** : SQLx utilise des requêtes préparées,
que le mode transactionnel ne supporte pas. La panne serait intermittente et sous charge,
donc particulièrement pénible à diagnostiquer.

## 5. Démarrer

```bash
docker compose --profile proxy up -d --build
```

Comptez ~15 minutes au premier lancement : compilation Rust en release et image Chromium.
Ensuite le cache fait son travail.

```bash
docker compose ps                     # tous les services en healthy
docker compose logs -f caddy          # le certificat doit être obtenu sans erreur
```

## 6. Vérifier — la seule étape qui compte

```bash
API=https://votredomaine.fr bash scripts/smoke.sh
```

Le script déroule un parcours réel : lien magique, création de signature, publication,
**téléchargement du GIF avec vérification des octets `GIF89a` et d'une taille plausible**,
clic redirigé, et contrôle que les événements respectent la vie privée. Il sort en code non
nul au premier écart.

Tant qu'il n'est pas vert, ce n'est pas en ligne. « Les conteneurs tournent » ne veut rien
dire pour un pipeline de rendu vidéo.

## 7. Brancher les services tiers

Dans l'ordre où ils rapportent quelque chose :

1. **Resend** — sans lui, personne ne peut se connecter par e-mail. À faire en premier.
2. **Stripe** — en mode Test d'abord. Le webhook doit pointer sur
   `https://votredomaine.fr/api/stripe/webhook`.
3. **Google OAuth** — le bouton n'apparaît qu'une fois configuré.
4. **xAI** — `AI_MODEL` + `AI_API_KEY`. Sans, la génération par IA bascule sur le repli
   déterministe : trois propositions construites depuis les couleurs extraites du site.
5. **Apple** — en dernier. Exige un compte développeur payant et refuse `localhost`.

`.env.example` documente chacun avec les URL exactes et les réglages à saisir, écran par
écran. Après chaque ajout :

```bash
docker compose up -d api renderer      # relit .env, pas de rebuild nécessaire
curl -s https://votredomaine.fr/api/config | jq
```

`/api/config` dit lesquels sont réellement actifs. C'est la source de vérité, pas le
contenu de `.env` : une clé mal collée s'y voit immédiatement.

---

## Exploitation

**Mettre à jour**

```bash
cd /opt/siglair && git pull && docker compose --profile proxy up -d --build
```

Les migrations s'appliquent au démarrage de l'API. Elles ne sont jamais rejouées deux fois.

**Sauvegarder** (inutile si vous êtes sur Supabase)

```bash
docker compose exec -T db pg_dump -U siglair siglair | gzip > ~/siglair-$(date +%F).sql.gz
```

À mettre dans un `cron` quotidien, avec une copie **hors de la machine**. Une sauvegarde qui
vit sur le serveur qu'elle protège n'est pas une sauvegarde.

Le volume `storage` contient les GIF et les logos. Il est reconstructible — republier
régénère tout — mais une republication de masse coûte du temps de rendu.

**Quand ça casse**

| Symptôme | Où regarder |
|---|---|
| Publication bloquée sur « en cours » | `docker compose logs renderer` |
| « service de rendu indisponible » | `docker compose exec db psql -U siglair -d siglair -c "SELECT error FROM render_jobs WHERE status='failed' ORDER BY created_at DESC LIMIT 3"` |
| Rendu tué en silence | RAM. `docker stats`, puis vérifier le swap |
| Certificat absent | `docker compose logs caddy` — presque toujours le DNS, ou le proxy Cloudflare laissé en orange |
| Connexion impossible | `RESEND_API_KEY` absente, ou `APP_URL` qui ne correspond pas au domaine réel |

---

## Ce qui reste à faire avant d'encaisser un paiement

Ce n'est pas de la paperasse optionnelle : ce sont des obligations, et deux d'entre elles
supposent des informations que je n'ai pas pu inventer.

- **Mentions légales et politique de confidentialité** : les pages existent, les champs
  raison sociale, SIRET, adresse et hébergeur sont à compléter. Ils sont marqués comme tels
  à l'écran. Faire relire par un juriste avant l'ouverture.
- **Le fournisseur d'IA doit être nommé comme sous-traitant** dans la politique de
  confidentialité, avec sa localisation.
- **Le suivi d'ouverture d'e-mail est un traitement de données personnelles.** Le champ
  `orgs.analytics_enabled` permet de le couper, et la suppression de compte purge les
  événements. À documenter côté client.
- **Stripe en mode Live** et un premier paiement de test réel.
- **Sauvegardes vérifiées** : une restauration jamais testée n'est pas une sauvegarde.
