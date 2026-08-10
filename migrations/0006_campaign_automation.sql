-- Campagnes réellement programmées et rendus déterministes.
--
-- Un job doit conserver le document exact demandé. Relire signatures.doc au moment où le
-- worker démarre faisait disparaître une campagne programmée (ou publier un brouillon plus
-- récent que le clic de l'utilisateur).
ALTER TABLE render_jobs
    ADD COLUMN doc         jsonb,
    ADD COLUMN profile     jsonb,
    ADD COLUMN campaign_id uuid;

-- NULL = le rendu publié est le document normal. Un uuid = le scheduler a publié cette
-- campagne. Pas de FK volontairement : supprimer une campagne doit laisser ce marqueur le
-- temps que le prochain balayage republie le document normal.
ALTER TABLE signatures
    ADD COLUMN published_campaign_id  uuid,
    ADD COLUMN campaign_base_doc      jsonb,
    ADD COLUMN campaign_base_profile  jsonb;

-- Pro applique une campagne aux signatures de son créateur ; Team l'applique à toute
-- l'organisation. Les campagnes historiques appartiennent au propriétaire le plus ancien.
ALTER TABLE campaigns
    ADD COLUMN owner_user_id uuid REFERENCES users(id) ON DELETE SET NULL;

UPDATE campaigns c
SET owner_user_id = (
    SELECT m.user_id
    FROM org_members m
    WHERE m.org_id = c.org_id AND m.role = 'owner'
    ORDER BY m.created_at, m.user_id
    LIMIT 1
)
WHERE c.owner_user_id IS NULL;

CREATE INDEX campaigns_owner_start_idx
    ON campaigns (org_id, owner_user_id, starts_at);
