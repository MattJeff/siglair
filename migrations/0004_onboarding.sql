-- Ce que le parcours d'onboarding (§6bis) et les routes rattrapées supposaient déjà en base.
-- Écrit à l'intégration : chacune de ces colonnes est référencée par du code livré sans elle,
-- donc par une requête qui échouait à l'exécution — pas à la compilation (sqlx runtime, §1).

-- ---------------------------------------------------------------- quota IA (§6bis.6)
-- Free : 1 génération AU TOTAL. Pro/Team : remis à zéro chaque mois. Un seul compteur pour
-- les deux règles ; `ai_quota_reset_at` porte la date du prochain remise à zéro, NULL tant
-- qu'aucune génération n'a eu lieu. Pas de tâche planifiée : le compteur se replie tout seul
-- à la lecture suivante, qui est aussi son écriture (UPDATE atomique).
ALTER TABLE orgs
    ADD COLUMN ai_generations_used integer NOT NULL DEFAULT 0,
    ADD COLUMN ai_quota_reset_at   timestamptz;

-- ---------------------------------------------------------------- organisation active
-- `PATCH /api/me` change l'organisation courante. Sans cette colonne, `current_org` reste
-- sur la plus ancienne appartenance et le sélecteur d'organisation ne fait rien.
-- ON DELETE SET NULL : quitter une org fait retomber sur l'org la plus ancienne.
ALTER TABLE users
    ADD COLUMN current_org_id uuid REFERENCES orgs(id) ON DELETE SET NULL;

-- ---------------------------------------------------------------- logo d'un visiteur
-- `POST /api/onboarding/analyze` est PUBLIQUE : le visiteur n'a pas encore d'organisation à
-- qui rattacher le logo extrait de son site. L'asset naît donc orphelin et daté ; `pick` le
-- rattache, la purge opportuniste enlève les autres.
ALTER TABLE assets
    ALTER COLUMN org_id DROP NOT NULL,
    ADD COLUMN expires_at timestamptz;

-- `UNIQUE (org_id, sha256)` ne contraint rien quand org_id est NULL (NULL n'est jamais égal
-- à NULL) : deux visiteurs peuvent analyser le même site, c'est voulu. Cet index sert la
-- purge, qui est la seule lecture des assets orphelins.
CREATE INDEX assets_expiry_idx ON assets (expires_at) WHERE org_id IS NULL;
