-- Campagnes datées — contrat §6 (ligne « Campagnes datées », Pro et Team).
-- Vendues sur la vitrine et présentes dans l'éditeur, elles ne vivaient jusqu'ici que dans
-- l'état React de l'écran : un F5 les perdait. Cette table est le manque.
--
-- Au niveau de l'ORGANISATION, pas de la signature : une bannière s'applique à plusieurs
-- signatures d'un coup, c'est tout son intérêt pour une équipe.

CREATE TABLE campaigns (
    id         uuid PRIMARY KEY,
    org_id     uuid NOT NULL REFERENCES orgs(id) ON DELETE CASCADE,
    name       text NOT NULL,
    message    text NOT NULL,               -- texte de la bannière
    cta        text NOT NULL DEFAULT '',    -- libellé du bouton ; '' = pas de bouton
    href       text NOT NULL DEFAULT '',    -- cible ; '' = bannière non cliquable
    color      text NOT NULL DEFAULT '#2563eb',
    starts_at  timestamptz NOT NULL,
    ends_at    timestamptz NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    -- une campagne qui finit avant de commencer n'est jamais active : autant la refuser
    CONSTRAINT campaigns_window_check CHECK (ends_at > starts_at),
    -- filet : cette couleur finit dans du HTML envoyé par email. L'API valide déjà (422),
    -- la base garantit qu'aucun autre écrivain ne pourra glisser autre chose.
    CONSTRAINT campaigns_color_check CHECK (color ~ '^#[0-9A-Fa-f]{6}$')
);

-- la seule lecture chaude : « quelle campagne de cette org est active maintenant ».
-- Postgres parcourt l'index à l'envers pour un ORDER BY starts_at DESC : pas besoin d'un
-- second index dans l'autre sens.
CREATE INDEX campaigns_org_start_idx ON campaigns (org_id, starts_at);
