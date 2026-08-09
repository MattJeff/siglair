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

log() { printf '\033[1m▸ %s\033[0m\n' "$1"; }

log "Images"
docker compose pull --quiet api renderer web

# L'image précédente est notée AVANT de basculer : sans elle, un retour arrière
# demanderait de retrouver le tag à la main pendant que le site est cassé.
PREV=$(docker inspect --format '{{index .RepoDigests 0}}' \
       "$(docker compose config --images api | head -1)" 2>/dev/null || echo '')

log "Bascule"
docker compose --profile proxy up -d --remove-orphans

# Les migrations s'appliquent au démarrage de l'API ; on attend qu'elle soit saine
# avant de déclarer quoi que ce soit.
log "Santé"
for i in $(seq 1 60); do
    if curl -fsS --max-time 3 http://127.0.0.1:8080/health >/dev/null 2>&1; then
        echo "  API saine après ${i}s"
        break
    fi
    if [ "$i" -eq 60 ]; then
        echo "  ÉCHEC : /health muet après 60s" >&2
        docker compose logs api --tail 40 >&2
        [ -n "$PREV" ] && echo "  retour arrière : docker compose down && docker run $PREV" >&2
        exit 1
    fi
    sleep 1
done

# Le juge. « Les conteneurs tournent » ne veut rien dire pour un pipeline de rendu :
# ce script vérifie qu'un GIF sort réellement de Chromium et qu'un clic est compté.
log "Parcours complet"
if API=https://siglair.com bash scripts/smoke.sh; then
    log "Déploiement validé"
else
    echo "  ÉCHEC du parcours complet — le site tourne mais quelque chose est cassé." >&2
    echo "  Image précédente : ${PREV:-inconnue}" >&2
    exit 1
fi

# Les images orphelines s'accumulent vite : ~1,5 Go chacune sur un disque de 40 Go.
docker image prune -f --filter 'until=72h' >/dev/null 2>&1 || true
