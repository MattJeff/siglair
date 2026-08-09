# Siglair — image unique pour les deux binaires (`api` et `renderer`).
# Ils partagent 99 % de leurs dépendances et la même stack de rendu : construire
# deux images de 1,5 Go pour une différence de commande serait deux fois le temps
# de build et deux fois le pull sur la VM.

# ---------------------------------------------------------------- builder
FROM rust:1.90-slim-bookworm AS builder
WORKDIR /app

# Étape 1 : les dépendances seules, avec un src factice. Tant que Cargo.toml et
# Cargo.lock ne bougent pas, ce layer est en cache et les ~250 crates ne sont
# pas recompilées à chaque modification de src/.
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src/bin \
 && touch src/lib.rs \
 && echo 'fn main() {}' > src/bin/api.rs \
 && echo 'fn main() {}' > src/bin/renderer.rs \
 && cargo build --release --locked \
 && rm -rf src

# Étape 2 : le vrai code. `migrations/` est embarqué par sqlx::migrate! à la
# compilation, il doit donc être présent avant le build.
COPY migrations ./migrations
COPY src ./src
# COPY conserve les mtimes du contexte : sans ce touch, cargo peut juger les
# sources plus vieilles que le build factice et livrer les binaires vides.
RUN find src migrations -type f -exec touch {} + \
 && cargo build --release --locked --bins

# ---------------------------------------------------------------- runtime
FROM debian:bookworm-slim

# fonts-noto-color-emoji n'est pas optionnel : les signatures sont pleines
# d'emoji et sans cette police Chromium les rasterise en carrés vides.
#
# chromium-sandbox non plus : sans lui, Chromium refuse de démarrer dans un conteneur
# (« No usable sandbox! ») et la seule issue serait --no-sandbox. Or on rasterise du
# HTML fourni par l'utilisateur et des images distantes : une faille Chromium sans bac
# à sable donnerait un accès direct à DATABASE_URL, que ce conteneur porte. Le paquet
# fournit le bac à sable SUID, qui fonctionne pour un utilisateur non-root.
RUN apt-get update && apt-get install -y --no-install-recommends \
        chromium \
        chromium-sandbox \
        ffmpeg \
        ca-certificates \
        curl \
        procps \
        fonts-dejavu-core \
        fonts-liberation \
        fonts-noto-color-emoji \
 && rm -rf /var/lib/apt/lists/*

RUN useradd --create-home --home-dir /home/app --uid 10001 --user-group app \
 && mkdir -p /data/storage \
 && chown -R app:app /data /home/app

COPY --from=builder /app/target/release/api      /usr/local/bin/api
COPY --from=builder /app/target/release/renderer /usr/local/bin/renderer

ENV HOME=/home/app \
    STORAGE_DIR=/data/storage \
    PORT=8080 \
    CHROME_PATH=/usr/bin/chromium \
    FFMPEG_PATH=/usr/bin/ffmpeg \
    RUST_LOG=info

USER app
WORKDIR /home/app
EXPOSE 8080

HEALTHCHECK --interval=15s --timeout=5s --start-period=20s --retries=5 \
    CMD curl -fsS "http://127.0.0.1:${PORT}/ready" || exit 1

CMD ["api"]
