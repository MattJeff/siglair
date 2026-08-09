-- Siglair — schéma initial. Voir docs/CONTRACT.md §4.
-- Règle : ce fichier n'est plus modifié après commit. Toute évolution = nouvelle migration.

CREATE EXTENSION IF NOT EXISTS citext;
CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- ---------------------------------------------------------------- identités

CREATE TABLE users (
    id            uuid PRIMARY KEY,
    email         citext NOT NULL UNIQUE,
    email_verified boolean NOT NULL DEFAULT false,
    name          text,
    avatar_url    text,
    created_at    timestamptz NOT NULL DEFAULT now(),
    last_seen_at  timestamptz
);

CREATE TABLE identities (
    id           uuid PRIMARY KEY,
    user_id      uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider     text NOT NULL CHECK (provider IN ('google', 'apple')),
    provider_uid text NOT NULL,
    created_at   timestamptz NOT NULL DEFAULT now(),
    UNIQUE (provider, provider_uid)
);
CREATE INDEX identities_user_idx ON identities (user_id);

CREATE TABLE magic_links (
    id           uuid PRIMARY KEY,
    email        citext NOT NULL,
    token_hash   bytea NOT NULL UNIQUE,
    expires_at   timestamptz NOT NULL,
    used_at      timestamptz,
    requested_ip inet,
    created_at   timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX magic_links_email_idx ON magic_links (email, created_at DESC);

CREATE TABLE sessions (
    id           uuid PRIMARY KEY,          -- valeur du cookie sig_session
    user_id      uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at   timestamptz NOT NULL DEFAULT now(),
    expires_at   timestamptz NOT NULL,
    last_seen_at timestamptz NOT NULL DEFAULT now(),
    user_agent   text,
    ip_hash      bytea
);
CREATE INDEX sessions_user_idx ON sessions (user_id);
CREATE INDEX sessions_expiry_idx ON sessions (expires_at);

-- ---------------------------------------------------------------- comptes

CREATE TABLE orgs (
    id                     uuid PRIMARY KEY,
    name                   text NOT NULL,
    slug                   citext NOT NULL UNIQUE,
    plan                   text NOT NULL DEFAULT 'free' CHECK (plan IN ('free', 'pro', 'team')),
    seats                  integer NOT NULL DEFAULT 1 CHECK (seats >= 1),
    analytics_enabled      boolean NOT NULL DEFAULT true,
    stripe_customer_id     text UNIQUE,
    stripe_subscription_id text UNIQUE,
    subscription_status    text,
    current_period_end     timestamptz,
    trial_ends_at          timestamptz,
    created_at             timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE org_members (
    org_id     uuid NOT NULL REFERENCES orgs(id) ON DELETE CASCADE,
    user_id    uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role       text NOT NULL DEFAULT 'member' CHECK (role IN ('owner', 'admin', 'member')),
    -- profil du membre : {{name}}, {{role}}, {{email}}... résolus dans ses signatures
    profile    jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (org_id, user_id)
);
CREATE INDEX org_members_user_idx ON org_members (user_id);

CREATE TABLE invites (
    id          uuid PRIMARY KEY,
    org_id      uuid NOT NULL REFERENCES orgs(id) ON DELETE CASCADE,
    email       citext NOT NULL,
    role        text NOT NULL DEFAULT 'member' CHECK (role IN ('admin', 'member')),
    token_hash  bytea NOT NULL UNIQUE,
    invited_by  uuid REFERENCES users(id) ON DELETE SET NULL,
    expires_at  timestamptz NOT NULL,
    accepted_at timestamptz,
    created_at  timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX invites_org_idx ON invites (org_id) WHERE accepted_at IS NULL;

-- ---------------------------------------------------------------- contenu

CREATE TABLE assets (
    id           uuid PRIMARY KEY,
    org_id       uuid NOT NULL REFERENCES orgs(id) ON DELETE CASCADE,
    kind         text NOT NULL CHECK (kind IN ('image', 'video')),
    filename     text NOT NULL,
    content_type text NOT NULL,
    bytes        bigint NOT NULL,
    sha256       bytea NOT NULL,
    storage_key  text NOT NULL,
    width        integer,
    height       integer,
    created_at   timestamptz NOT NULL DEFAULT now(),
    UNIQUE (org_id, sha256)
);
CREATE INDEX assets_org_idx ON assets (org_id, created_at DESC);

CREATE TABLE signatures (
    id                  uuid PRIMARY KEY,
    org_id              uuid NOT NULL REFERENCES orgs(id) ON DELETE CASCADE,
    owner_user_id       uuid REFERENCES users(id) ON DELETE SET NULL,
    name                text NOT NULL,
    kind                text NOT NULL DEFAULT 'personal' CHECK (kind IN ('personal', 'org_template')),
    doc                 jsonb NOT NULL,
    profile             jsonb NOT NULL DEFAULT '{}'::jsonb,
    public_slug         citext UNIQUE,
    published_render_id uuid,               -- FK ajoutée après la table renders
    created_at          timestamptz NOT NULL DEFAULT now(),
    updated_at          timestamptz NOT NULL DEFAULT now(),
    deleted_at          timestamptz
);
CREATE INDEX signatures_org_idx ON signatures (org_id) WHERE deleted_at IS NULL;
CREATE INDEX signatures_owner_idx ON signatures (owner_user_id) WHERE deleted_at IS NULL;

CREATE TABLE renders (
    id            uuid PRIMARY KEY,
    signature_id  uuid NOT NULL REFERENCES signatures(id) ON DELETE CASCADE,
    gif_key       text NOT NULL,
    png_key       text NOT NULL,
    width         integer NOT NULL,
    height        integer NOT NULL,
    frames        integer NOT NULL,
    fps           integer NOT NULL,
    bytes         bigint NOT NULL,
    doc_hash      bytea NOT NULL,
    created_at    timestamptz NOT NULL DEFAULT now()
);
-- republier un document inchangé doit réutiliser le rendu existant (contrat §7.1)
CREATE INDEX renders_reuse_idx ON renders (signature_id, doc_hash);

ALTER TABLE signatures
    ADD CONSTRAINT signatures_published_render_fkey
    FOREIGN KEY (published_render_id) REFERENCES renders(id) ON DELETE SET NULL;

CREATE TABLE render_jobs (
    id           uuid PRIMARY KEY,
    signature_id uuid NOT NULL REFERENCES signatures(id) ON DELETE CASCADE,
    status       text NOT NULL DEFAULT 'queued' CHECK (status IN ('queued', 'running', 'done', 'failed')),
    attempts     integer NOT NULL DEFAULT 0,
    error        text,
    doc_hash     bytea NOT NULL,
    run_after    timestamptz NOT NULL DEFAULT now(),   -- backoff des réessais
    created_at   timestamptz NOT NULL DEFAULT now(),
    started_at   timestamptz,
    finished_at  timestamptz
);
-- le worker lit exactement ceci : WHERE status='queued' AND run_after <= now() ORDER BY created_at
CREATE INDEX render_jobs_claim_idx ON render_jobs (status, run_after, created_at);
CREATE INDEX render_jobs_signature_idx ON render_jobs (signature_id, created_at DESC);

-- ---------------------------------------------------------------- mesure

CREATE TABLE events (
    id           bigserial PRIMARY KEY,
    signature_id uuid NOT NULL REFERENCES signatures(id) ON DELETE CASCADE,
    kind         text NOT NULL CHECK (kind IN ('open', 'click')),
    element_id   text,
    target_host  text,
    ip_hash      bytea,        -- sha256(ip || salt)[..16] — jamais l'IP en clair (contrat §4.1)
    ua_family    text,         -- gmail | outlook | apple-mail | other — jamais l'UA brut
    occurred_at  timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX events_signature_idx ON events (signature_id, occurred_at DESC);

CREATE TABLE stripe_events (
    id          text PRIMARY KEY,           -- evt_... : rejoue le même webhook sans effet
    received_at timestamptz NOT NULL DEFAULT now()
);
