-- Croissance : la boucle virale et sa mesure — contrat §11.
--
-- Numéro 0007 et pas 0006 : 0006_campaign_automation.sql existe déjà et est appliqué en
-- production. sqlx dérive la version du préfixe du nom de fichier et `_sqlx_migrations`
-- a `version` en clé primaire — deux fichiers `0006_` font échouer le démarrage de l'API
-- sur un conflit de version (ou pire, sur un checksum différent pour une version déjà
-- appliquée). Ce fichier n'est plus modifié après commit : toute évolution = 0008.

-- ---------------------------------------------------------------- événements

-- Six kinds, fermés par un CHECK (contrat §11.1). Le CHECK n'est pas décoratif : c'est
-- lui qui empêche « tracker tout ce qui est possible » d'arriver par accident, un
-- INSERT à la fois. Ajouter un septième événement demande une migration, donc une
-- question précise à laquelle il répond.
CREATE TABLE growth_events (
    id           bigserial PRIMARY KEY,
    org_id       uuid NOT NULL REFERENCES orgs(id) ON DELETE CASCADE,
    -- NULL quand l'événement vient d'un destinataire d'e-mail : ce n'est pas un
    -- utilisateur, il n'a pas de ligne dans `users` et n'en aura jamais (§11.3).
    user_id      uuid REFERENCES users(id) ON DELETE CASCADE,
    signature_id uuid REFERENCES signatures(id) ON DELETE CASCADE,
    kind         text NOT NULL CHECK (kind IN (
                     'signature_generated', 'signature_installed', 'powered_by_click',
                     'upgrade_started', 'upgrade_completed', 'team_invite')),
    props        jsonb NOT NULL DEFAULT '{}'::jsonb,
    ip_hash      bytea,        -- sha256(ip || salt)[..16] — jamais l'IP en clair (§4.1)
    ua_family    text,         -- gmail | outlook | apple-mail | other — jamais l'UA brut
    occurred_at  timestamptz NOT NULL DEFAULT now()
);

-- Le tableau de bord d'une org lit une fenêtre récente de SES événements.
CREATE INDEX growth_events_org_idx  ON growth_events (org_id, occurred_at DESC);
-- Nos propres questions se posent par événement, toutes orgs confondues.
CREATE INDEX growth_events_kind_idx ON growth_events (kind, occurred_at DESC);

-- Une installation par signature, garantie par la base et pas par une lecture-puis-
-- écriture côté application : le proxy d'images de Gmail rappelle l'URL plusieurs fois
-- en parallèle, et deux tâches détachées qui vérifient « ça n'existe pas encore » en
-- même temps insèrent deux lignes. Ici la seconde tombe sur ON CONFLICT DO NOTHING.
CREATE UNIQUE INDEX growth_events_install_once_idx
    ON growth_events (signature_id) WHERE kind = 'signature_installed';

-- ---------------------------------------------------------------- attribution

-- §11.3 : l'attribution passe par l'URL, jamais par un cookie posé sur le destinataire.
-- Ces deux colonnes sont écrites une seule fois, à la création du compte.
ALTER TABLE users
    -- ON DELETE SET NULL, surtout pas CASCADE : supprimer la signature qui a amené
    -- quelqu'un ne doit pas supprimer cette personne ni son compte.
    ADD COLUMN referred_by_signature_id uuid REFERENCES signatures(id) ON DELETE SET NULL,
    -- Le code brut reçu dans l'URL. Il survit à la suppression de la signature, alors
    -- que la clé étrangère ci-dessus passe à NULL : sans lui on perdrait la trace de
    -- l'origine du compte au premier ménage.
    ADD COLUMN referral_code text;

-- Sert l'entonnoir (§11.4, comptes créés depuis les signatures d'une org) et le
-- ON DELETE SET NULL, qui sans index balaye `users` à chaque suppression de signature.
CREATE INDEX users_referred_idx ON users (referred_by_signature_id)
    WHERE referred_by_signature_id IS NOT NULL;

-- Le code court qui identifie la signature dans le badge (§11.2).
-- citext UNIQUE : il voyage dans une URL, il sera recopié à la main et en majuscules.
-- Aucun remplissage rétroactif ici volontairement : l'alphabet sans caractères ambigus
-- (ni 0/O, ni 1/l) vit dans `util::gen_slug`, une seule fois. Le recopier en SQL
-- créerait une deuxième source de vérité qui divergerait. Les signatures existantes
-- reçoivent leur code à la prochaine écriture, avec `util::gen_slug()`.
ALTER TABLE signatures ADD COLUMN referral_code citext UNIQUE;
