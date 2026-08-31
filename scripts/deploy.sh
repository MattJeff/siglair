#!/usr/bin/env bash
#
# Déploiement — exécuté par GitHub Actions via une clé SSH à commande forcée.
#
# NE PAS RENOMMER, NE PAS DÉPLACER. La clé de déploiement du serveur est câblée
# sur /opt/siglair/scripts/deploy.sh : elle ne peut lancer QUE ce chemin — pas de
# shell, pas de tunnel, pas de transfert de fichier (cf. authorized_keys). Une
# fuite du secret GitHub ne donne donc pas un accès root au serveur, seulement le
# droit de redéployer. Changer le chemin, c'est casser le déploiement sans que
# rien ne le dise avant la prochaine bascule.
#
# Ce qui tourne ici est maintenant `agentos-server` (InternationalAgent). Il n'y
# a plus de renderer ni de front : la console vit dans orizn-web et appelle
# l'API par HTTP.
#
# Le build ne se fait PAS ici. Les images sont construites par GitHub et
# simplement téléchargées : sur 2 vCPU, une compilation Rust en release met un
# quart d'heure et peut faire tuer autre chose par l'OOM.
#
set -euo pipefail

cd /opt/siglair

# Une copie de toute la sortie sur le disque du serveur.
#
# Un déploiement lancé par la CI passe par une clé à commande forcée : sa sortie
# n'existe que dans le flux SSH. Le 15 août, ce flux s'est fermé une seconde
# après l'ouverture de session, sans une seule ligne — impossible de savoir où le
# script était mort, ni même s'il avait démarré. Le journal du runner ne montrait
# que « exit code 255 ».
#
# `tee -a` et non `>` : deux déploiements consécutifs doivent tous les deux
# rester lisibles.
exec > >(tee -a /var/log/siglair-deploy.log) 2>&1
printf '\n===== %s — déploiement lancé (pid %s) =====\n' "$(date -Is)" "$$"

log() { printf '\033[1m▸ %s\033[0m\n' "$1"; }

# L'image en place est notée AVANT le `pull`, et par IDENTIFIANT.
#
# L'ancienne version la relevait APRÈS le pull : `docker inspect` sur le nom de
# l'image résolvait alors la NOUVELLE, et le « retour arrière » proposé pointait
# sur l'image cassée. L'identifiant, lui, ne bouge pas quand un tag est
# réattribué — un digest ou un nom de tag ne survivrait pas au pull suivant.
#
# `docker compose config --images api | head -1` — une version encore plus
# ancienne — avait DEUX défauts, et le second cachait le premier.
#
# 1. `--images api` liste aussi les images des services dont `api` dépend. La
#    première ligne était celle de Postgres, pas celle de l'API : le retour
#    arrière aurait retagué la base.
# 2. `head -1` ferme le tuyau dès la première ligne. Quand docker n'a pas encore
#    fini d'écrire la seconde, il prend un SIGPIPE et sort en 255 ; `pipefail`
#    propage, `set -e` tue le script. C'est une course : selon que docker avait
#    rempli le tampon avant ou après, le déploiement passait ou échouait. D'où
#    des échecs « exit code 255 » sans une ligne de sortie, un jour sur deux,
#    sans qu'aucun code n'ait changé.
#
# `jq` lit son entrée jusqu'au bout : pas de SIGPIPE possible.
IMG_API=$(docker compose config --format json | jq -r '.services.api.image')
PREV_API=$(docker image inspect --format '{{.Id}}' "$IMG_API" 2>/dev/null || echo '')

# /livez et pas /health : agentos-server n'expose pas /health. Et pas /readyz
# non plus — voir plus bas, /readyz peut légitimement répondre 503.
# L'image runtime ne contient ni curl ni wget, donc la sonde vient de l'hôte, par
# le port publié en loopback dans compose.yml.
sain() { curl -fsS --max-time 3 http://127.0.0.1:8080/livez >/dev/null 2>&1; }

attendre_sante() {
    # 120 et pas 60 : au premier démarrage `Db::migrate()` applique 69 fichiers
    # de migration sur une base vide, dont la création d'une extension et d'un
    # index HNSW, AVANT que le listener ne se lie. Un délai calibré sur un
    # démarrage à chaud déclencherait un retour arrière sur un serveur qui
    # travaille normalement.
    for i in $(seq 1 120); do
        if sain; then echo "  API saine après ${i}s"; return 0; fi
        sleep 1
    done
    return 1
}

# Le déploiement REVIENT EN ARRIÈRE tout seul. L'ancienne version se contentait
# d'imprimer une suggestion et sortait en erreur : une migration refusée au
# démarrage laissait l'API redémarrer en boucle, Caddy sans amont, et le site en
# 502 jusqu'à ce qu'un humain regarde. C'est exactement ce qui s'est produit avec
# le doublon de numéro de migration 0009.
migrations_appliquees() {
    # `|| true`, et ce n'est pas de la négligence : sous `set -euo pipefail`,
    # une substitution de commande qui échoue tue le script. Or il existe un cas
    # où cette requête DOIT échouer — le tout premier déploiement de cette
    # version, où le conteneur `db` en place est encore l'ancien Postgres 16 et
    # sa base ne s'appelle pas `agentos`. Sans cette garde, le déploiement qui
    # remplace l'ancienne pile est exactement celui qui ne peut pas partir.
    #
    # Une version illisible sort donc VIDE, et le refus de retour arrière plus
    # haut ne se déclenche que sur deux versions lues et différentes : « je ne
    # sais pas » n'est pas « la base a bougé ».
    docker compose exec -T db psql -U postgres -d agentos -tAc \
        'SELECT coalesce(max(version), 0) FROM _sqlx_migrations' 2>/dev/null | tr -d ' \r' || true
}

retour_arriere() {
    if [ -z "$PREV_API" ]; then
        echo "  AUCUNE image précédente connue — le site reste en panne, intervention requise." >&2
        return 1
    fi

    # UN RETOUR ARRIÈRE NE TRAVERSE PAS UNE MIGRATION.
    #
    # `sqlx::migrate!` embarque les migrations DANS le binaire et refuse de
    # démarrer s'il en trouve une en base qu'il ne connaît pas : « migration N
    # was previously applied but is missing in the resolved migrations ».
    # Remettre l'image précédente après que la nouvelle a migré la base produit
    # donc un binaire qui ne démarre PLUS JAMAIS.
    #
    # C'est arrivé le 15 août : la nouvelle image était saine, la validation a
    # échoué sur un bug applicatif, le retour arrière s'est déclenché — et a
    # transformé « une version imparfaite tourne » en « aucune version ne peut
    # démarrer ». Le remède a fait plus de mal que le mal. Dans ce cas il n'y a
    # qu'une issue sûre : garder la nouvelle image et corriger en avant.
    local apres
    apres=$(migrations_appliquees)
    if [ -n "$AVANT_MIGRATIONS" ] && [ -n "$apres" ] && [ "$apres" != "$AVANT_MIGRATIONS" ]; then
        echo "  RETOUR ARRIÈRE REFUSÉ : la base est passée de la migration ${AVANT_MIGRATIONS} à ${apres}." >&2
        echo "  L'image précédente ne connaît pas la migration ${apres} et refuserait de démarrer." >&2
        echo "  La nouvelle image reste en place. Corriger en avant et redéployer." >&2
        return 1
    fi
    echo "  RETOUR ARRIÈRE vers ${PREV_API:0:19}" >&2
    docker tag "$PREV_API" "$IMG_API"
    docker compose --profile proxy up -d --remove-orphans >&2
    if attendre_sante >&2; then
        echo "  service restauré sur la version précédente." >&2
        return 0
    fi
    echo "  RETOUR ARRIÈRE ÉCHOUÉ — intervention manuelle requise." >&2
    return 1
}

# Relevé AVANT la bascule : c'est le seul moment où la base est encore à la
# version que l'image précédente sait lire. Après, l'information est perdue.
AVANT_MIGRATIONS=$(migrations_appliquees)

log "Images"
docker compose pull --quiet api

log "Bascule"
# `--remove-orphans` : au tout premier déploiement de cette version, il supprime
# les conteneurs `renderer` et `web` de l'ancien siglair, qui n'existent plus
# dans compose.yml.
docker compose --profile proxy up -d --remove-orphans

# Les migrations s'appliquent au démarrage du serveur ; on attend qu'il soit sain
# avant de déclarer quoi que ce soit.
log "Santé"
if ! attendre_sante; then
    echo "  ÉCHEC : /livez muet après 120s" >&2
    docker compose logs api --tail 60 >&2
    retour_arriere || true
    exit 1
fi

# /readyz est LU mais NE DÉCLENCHE PAS de retour arrière, et c'est délibéré.
#
# Il répond 503 tant que le plafond de politique n'est pas installé
# (`no_platform_policy`, `agentos-server policy install`), et il le fait aussi
# quand le retard de l'outbox dépasse son seuil — deux états d'exploitation
# normaux, pas des pannes de déploiement. Revenir en arrière là-dessus, ce serait
# refaire l'erreur du 15 août : un remède pire que le mal, sur un serveur qui
# tourne. Ce que ça publie mérite en revanche d'être dans le journal, et
# notamment `mock_adapters` — la liste des adaptateurs qui ne font rien de réel.
log "Prêt ?"
# Sans `-f`, exprès : `--fail` remplace le corps par rien du tout sur un 503, et
# c'est justement le corps qui nomme la raison.
curl -sS --max-time 5 http://127.0.0.1:8080/readyz || true
echo

log "Déploiement validé"

# Les images orphelines s'accumulent vite. Le disque de ce serveur a déjà tué un
# cluster PostgreSQL une fois.
docker image prune -f --filter 'until=72h' >/dev/null 2>&1 || true
