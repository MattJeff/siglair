-- Vérification des migrations sur une base NEUVE.
--
-- Deux choses sont contrôlées ici, et aucune ne l'était avant :
--
-- 1. que toutes les migrations s'appliquent d'affilée sur une base vide. Une migration
--    cassée ne se voyait qu'au démarrage en production, `sqlx::migrate!` s'exécutant au
--    boot de l'API : le conteneur redémarrait en boucle et le site tombait ;
-- 2. que `purge_engagement_events` respecte la rétention DU PLAN de chaque organisation.
--    C'est le seul point subtil de la fonction : un seuil global unique passe tous les
--    tests naïfs et efface pourtant l'historique des clients Team, ceux qui paient le plus
--    cher précisément pour ces douze mois d'historique.
--
-- Lancé par la CI (voir .github/workflows/deploy.yml, job `migrations`).
\set ON_ERROR_STOP on

TRUNCATE users, orgs CASCADE;

INSERT INTO users (id, email) VALUES
  ('11111111-1111-1111-1111-111111111111', 'a@test.fr'),
  ('22222222-2222-2222-2222-222222222222', 'b@test.fr');

INSERT INTO orgs (id, name, slug, plan) VALUES
  ('aaaaaaaa-0000-0000-0000-000000000001', 'Free', 'free-org', 'free'),
  ('bbbbbbbb-0000-0000-0000-000000000002', 'Team', 'team-org', 'team');

INSERT INTO signatures (id, org_id, owner_user_id, name, doc, profile) VALUES
  ('cccccccc-0000-0000-0000-000000000001', 'aaaaaaaa-0000-0000-0000-000000000001',
   '11111111-1111-1111-1111-111111111111', 's', '{}', '{}'),
  ('dddddddd-0000-0000-0000-000000000002', 'bbbbbbbb-0000-0000-0000-000000000002',
   '22222222-2222-2222-2222-222222222222', 's', '{}', '{}');

INSERT INTO events (signature_id, kind, occurred_at) VALUES
  -- org Free, fenêtre 0 + 35 j
  ('cccccccc-0000-0000-0000-000000000001', 'open',  now() - interval '40 days'),   -- part
  ('cccccccc-0000-0000-0000-000000000001', 'open',  now() - interval '10 days'),   -- reste
  -- org Team, fenêtre 365 + 35 j : 100 jours DOIT survivre
  ('dddddddd-0000-0000-0000-000000000002', 'click', now() - interval '100 days'),  -- reste
  ('dddddddd-0000-0000-0000-000000000002', 'click', now() - interval '500 days');  -- part

INSERT INTO growth_events (org_id, kind, occurred_at) VALUES
  ('aaaaaaaa-0000-0000-0000-000000000001', 'powered_by_click', now() - interval '40 days'),
  ('bbbbbbbb-0000-0000-0000-000000000002', 'powered_by_click', now() - interval '100 days');

-- Les mêmes valeurs que celles calculées par l'API depuis plans.rs (analytics_days + 35).
SELECT purge_engagement_events(0 + 35, 30 + 35, 365 + 35);

DO $$
DECLARE
    free_ev   bigint;
    team_ev   bigint;
    free_gr   bigint;
    team_gr   bigint;
BEGIN
    SELECT count(*) INTO free_ev FROM events e
      JOIN signatures s ON s.id = e.signature_id
      WHERE s.org_id = 'aaaaaaaa-0000-0000-0000-000000000001';
    SELECT count(*) INTO team_ev FROM events e
      JOIN signatures s ON s.id = e.signature_id
      WHERE s.org_id = 'bbbbbbbb-0000-0000-0000-000000000002';
    SELECT count(*) INTO free_gr FROM growth_events
      WHERE org_id = 'aaaaaaaa-0000-0000-0000-000000000001';
    SELECT count(*) INTO team_gr FROM growth_events
      WHERE org_id = 'bbbbbbbb-0000-0000-0000-000000000002';

    IF free_ev <> 1 THEN
        RAISE EXCEPTION 'org Free : % événements restants au lieu de 1 (celui de 10 jours)', free_ev;
    END IF;
    IF team_ev <> 1 THEN
        RAISE EXCEPTION 'org Team : % événements restants au lieu de 1 — celui de 100 jours doit survivre, la rétention Team est de 12 mois', team_ev;
    END IF;
    IF free_gr <> 0 THEN
        RAISE EXCEPTION 'org Free : % growth_events restants au lieu de 0', free_gr;
    END IF;
    IF team_gr <> 1 THEN
        RAISE EXCEPTION 'org Team : % growth_events restants au lieu de 1', team_gr;
    END IF;

    RAISE NOTICE 'purge_engagement_events respecte la rétention de chaque plan';
END;
$$;

TRUNCATE users, orgs CASCADE;
