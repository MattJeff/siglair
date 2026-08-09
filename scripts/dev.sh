#!/usr/bin/env bash
#
# Siglair — développement local : Postgres dans Docker, api + renderer en natif.
# Le front se lance à part :  cd web && npm run dev
#
set -euo pipefail

cd "$(dirname "$0")/.."

if [[ ! -f .env ]]; then
    cp .env.example .env
    echo "→ .env créé depuis .env.example."
    echo "  Renseignez au minimum SIGLAIR_SECRET_KEY :"
    echo "      openssl rand -base64 32"
    exit 1
fi

set -a
# shellcheck disable=SC1091
. ./.env
set +a

[[ -n "${SIGLAIR_SECRET_KEY:-}" ]] || {
    echo "SIGLAIR_SECRET_KEY est vide dans .env. Générez-la : openssl rand -base64 32" >&2
    exit 1
}

echo "→ Postgres"
docker compose up -d db
until docker compose exec -T db pg_isready -U siglair -d siglair >/dev/null 2>&1; do sleep 1; done

mkdir -p "${STORAGE_DIR:-./data/storage}"

# cargo watch si disponible, sinon un simple run.
if command -v cargo-watch >/dev/null; then
    run() { cargo watch -q -x "run --bin $1"; }
else
    echo "→ cargo-watch absent (cargo install cargo-watch) : pas de rechargement à chaud."
    run() { cargo run --bin "$1"; }
fi

# Le renderer en tâche de fond, l'API au premier plan : Ctrl-C arrête les deux.
run renderer &
RENDERER=$!
trap 'kill "$RENDERER" 2>/dev/null || true' EXIT

echo "→ API sur http://localhost:${PORT:-8080} — front : cd web && npm run dev"
run api
