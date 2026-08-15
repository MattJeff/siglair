#!/usr/bin/env bash
#
# Déploiement — exécuté par GitHub Actions via une clé SSH à commande forcée.
#
# La clé de déploiement ne peut lancer QUE ce script : pas de shell, pas de tunnel,
# pas de transfert de fichier (cf. authorized_keys). Une fuite du secret GitHub ne
# donne donc pas un accès root au serveur, seulement le droit de redéployer.
#
# Le build ne se fait PAS ici. Les images sont construites par GitHub et simplement
# téléchargées : sur 2 vCPU qui font déjà tourner Chromium, une compilation Rust en
# release met ~15 minutes et peut faire tuer un rendu en cours par l'OOM.
#
set -euo pipefail

cd /opt/siglair

# Une copie de toute la sortie sur le disque du serveur.
#
# Un déploiement lancé par la CI passe par une clé à commande forcée : sa sortie n'existe que
# dans le flux SSH. Le 15 août, ce flux s'est fermé une seconde après l'ouverture de session,
# sans une seule ligne — impossible de savoir où le script était mort, ni même s'il avait
# démarré. Le journal du runner ne montrait que « exit code 255 ».
#
# `tee -a` et non `>` : deux déploiements consécutifs doivent tous les deux rester lisibles.
exec > >(tee -a /var/log/siglair-deploy.log) 2>&1
printf '\n===== %s — déploiement lancé (pid %s) =====\n' "$(date -Is)" "$$"

log() { printf '\033[1m▸ %s\033[0m\n' "$1"; }

# Les images en place sont notées AVANT le `pull`, et par IDENTIFIANT.
#
# L'ancienne version les relevait APRÈS le pull : `docker inspect` sur le nom de l'image
# résolvait alors la NOUVELLE, et le « retour arrière » proposé pointait sur l'image cassée.
# L'identifiant, lui, ne bouge pas quand un tag est réattribué — un digest ou un nom de tag
# ne survivrait pas au pull suivant.
# `docker compose config --images api | head -1` — la version précédente — avait DEUX défauts,
# et le second cachait le premier.
#
# 1. `--images api` liste aussi les images des services dont `api` dépend. La première ligne est
#    `postgres:16-alpine`, pas celle de l'API : le retour arrière aurait retagué Postgres.
# 2. `head -1` ferme le tuyau dès la première ligne. Quand docker n'a pas encore fini d'écrire
#    la seconde, il prend un SIGPIPE et sort en 255 ; `pipefail` propage, `set -e` tue le script.
#    C'est une course : selon que docker a rempli le tampon du tuyau avant ou après, le
#    déploiement passait ou échouait. D'où des échecs « exit code 255 » sans une ligne de sortie,
#    un jour sur deux, sans qu'aucun code n'ait changé.
#
# `jq` lit son entrée jusqu'au bout : pas de SIGPIPE possible. Il est déjà exigé par smoke.sh.
IMG_API=$(docker compose config --format json | jq -r '.services.api.image')
IMG_WEB=$(docker compose config --format json | jq -r '.services.web.image')
PREV_API=$(docker image inspect --format '{{.Id}}' "$IMG_API" 2>/dev/null || echo '')
PREV_WEB=$(docker image inspect --format '{{.Id}}' "$IMG_WEB" 2>/dev/null || echo '')

sain() { curl -fsS --max-time 3 http://127.0.0.1:8080/health >/dev/null 2>&1; }

attendre_sante() {
    for i in $(seq 1 60); do
        if sain; then echo "  API saine après ${i}s"; return 0; fi
        sleep 1
    done
    return 1
}

# Le déploiement REVIENT EN ARRIÈRE tout seul. L'ancienne version se contentait
# d'imprimer une suggestion et sortait en erreur : une migration refusée au démarrage
# laissait l'API redémarrer en boucle, Caddy sans amont, et le site en 502 jusqu'à ce
# qu'un humain regarde. C'est exactement ce qui s'est produit avec le doublon de
# numéro de migration 0009.
retour_arriere() {
    if [ -z "$PREV_API" ]; then
        echo "  AUCUNE image précédente connue — le site reste en panne, intervention requise." >&2
        return 1
    fi
    echo "  RETOUR ARRIÈRE vers ${PREV_API:0:19}" >&2
    docker tag "$PREV_API" "$IMG_API"
    [ -n "$PREV_WEB" ] && docker tag "$PREV_WEB" "$IMG_WEB"
    docker compose --profile proxy up -d --remove-orphans >&2
    if attendre_sante >&2; then
        echo "  service restauré sur la version précédente." >&2
        return 0
    fi
    echo "  RETOUR ARRIÈRE ÉCHOUÉ — intervention manuelle requise." >&2
    return 1
}

log "Images"
docker compose pull --quiet api renderer web

log "Bascule"
docker compose --profile proxy up -d --remove-orphans

# Les migrations s'appliquent au démarrage de l'API ; on attend qu'elle soit saine
# avant de déclarer quoi que ce soit.
log "Santé"
if ! attendre_sante; then
    echo "  ÉCHEC : /health muet après 60s" >&2
    docker compose logs api --tail 40 >&2
    retour_arriere || true
    exit 1
fi

# Le juge. « Les conteneurs tournent » ne veut rien dire pour un pipeline de rendu :
# ce script vérifie qu'un GIF sort réellement de Chromium et qu'un clic est compté.
log "Parcours complet"
if API=https://siglair.com bash scripts/smoke.sh; then
    log "Déploiement validé"
else
    echo "  ÉCHEC du parcours complet — le site répond mais quelque chose est cassé." >&2
    retour_arriere || true
    exit 1
fi

# Les images orphelines s'accumulent vite : ~1,5 Go chacune sur un disque de 40 Go.
docker image prune -f --filter 'until=72h' >/dev/null 2>&1 || true
