-- Purge des événements d'engagement — contrat §11.5, « pas de conservation au-delà de la
-- rétention du plan ».
--
-- `product_events` avait déjà sa purge (0008). `events` (ouvertures et clics) et
-- `growth_events` (les six signaux du canal viral) n'en avaient aucune : elles grossissaient
-- indéfiniment. Sur une signature installée, le proxy d'images de Gmail rappelle l'URL à
-- chaque ouverture de l'e-mail par le destinataire — c'est la table qui grossit le plus vite
-- de toute la base, et personne ne la lit au-delà de la fenêtre du plan.
--
-- Les durées ne sont PAS écrites ici. Elles arrivent en paramètres depuis `plans.rs`, qui
-- est le seul endroit où les limites d'un plan existent (contrat §6). Une copie des valeurs
-- dans une migration dériverait le jour où un plan change, et personne ne le verrait : la
-- purge continuerait de tourner en silence sur les anciennes durées.
--
-- La lecture applique déjà la rétention du plan (`window_days` borne la fenêtre demandée sur
-- `analytics_days`). Cette fonction ferme l'autre bout : ce qui n'est plus lisible finit par
-- ne plus être stocké.
CREATE OR REPLACE FUNCTION purge_engagement_events(
    p_free_days integer,
    p_pro_days  integer,
    p_team_days integer,
    p_limit     integer DEFAULT 10000
) RETURNS bigint
LANGUAGE plpgsql
AS $$
DECLARE
    total bigint := 0;
    n     bigint;
BEGIN
    -- Le plan est lu dans `orgs.plan`, pas via `effective_plan` : cette machine à états vit
    -- en Rust et dépend du délai de grâce. La colonne bascule sur 'free' AVANT la fin de ce
    -- délai — d'où la marge ajoutée aux durées côté appelant, pour qu'un incident de
    -- paiement de quelques jours n'efface pas l'historique d'un client qui revient.
    WITH bornes AS (
        SELECT o.id AS org_id,
               now() - make_interval(days => CASE o.plan
                   WHEN 'team' THEN GREATEST(p_team_days, 1)
                   WHEN 'pro'  THEN GREATEST(p_pro_days, 1)
                   ELSE GREATEST(p_free_days, 1)
               END) AS avant
        FROM orgs o
    ),
    doomed AS (
        SELECT e.id
        FROM events e
        JOIN signatures s ON s.id = e.signature_id
        JOIN bornes b     ON b.org_id = s.org_id
        WHERE e.occurred_at < b.avant
        ORDER BY e.id
        LIMIT LEAST(GREATEST(p_limit, 1), 50000)
    )
    DELETE FROM events e USING doomed d WHERE e.id = d.id;
    GET DIAGNOSTICS n = ROW_COUNT;
    total := total + n;

    WITH bornes AS (
        SELECT o.id AS org_id,
               now() - make_interval(days => CASE o.plan
                   WHEN 'team' THEN GREATEST(p_team_days, 1)
                   WHEN 'pro'  THEN GREATEST(p_pro_days, 1)
                   ELSE GREATEST(p_free_days, 1)
               END) AS avant
        FROM orgs o
    ),
    doomed AS (
        SELECT g.id
        FROM growth_events g
        JOIN bornes b ON b.org_id = g.org_id
        WHERE g.occurred_at < b.avant
        ORDER BY g.id
        LIMIT LEAST(GREATEST(p_limit, 1), 50000)
    )
    DELETE FROM growth_events g USING doomed d WHERE g.id = d.id;
    GET DIAGNOSTICS n = ROW_COUNT;
    total := total + n;

    RETURN total;
END;
$$;

-- La purge balaie par `occurred_at` sur toute la table ; sans cet index elle fait un
-- parcours complet chaque nuit. `events_signature_idx` ne sert pas : il commence par
-- `signature_id`.
CREATE INDEX IF NOT EXISTS events_occurred_idx        ON events (occurred_at);
CREATE INDEX IF NOT EXISTS growth_events_occurred_idx ON growth_events (occurred_at);
