-- Analytics produit first-party. Cette table est distincte de `events` (mesure des
-- signatures dans les clients mail) et de `growth_events` (signaux métier émis côté
-- serveur) : elle reçoit les interactions de la landing et de l'application.

CREATE TABLE product_events (
    -- Fourni par le client et globalement unique : rejouer un lot après une coupure réseau
    -- est sans effet. Aucun identifiant séquentiel n'est exposé.
    event_id             uuid PRIMARY KEY,
    name                 text NOT NULL CHECK (name IN (
        'page_viewed', 'landing_viewed', 'pricing_viewed', 'hero_cta_clicked',
        'navigation_clicked', 'faq_opened', 'url_input_focused', 'url_entered',
        'url_submitted', 'generation_started', 'generation_completed',
        'generation_failed', 'preview_viewed', 'claim_clicked', 'signup_started',
        'signup_completed', 'login_started', 'login_completed', 'editor_opened',
        'editor_session_ended', 'block_added', 'block_deleted', 'block_moved',
        'asset_uploaded', 'logo_changed', 'profile_picture_changed', 'font_changed',
        'colors_changed', 'layout_changed', 'template_selected', 'animation_selected',
        'cta_added', 'social_icon_added', 'field_added', 'field_removed',
        'undo_clicked', 'redo_clicked', 'preview_opened', 'mobile_preview_opened',
        'desktop_preview_opened', 'signature_saved', 'publish_started',
        'publish_completed', 'publish_failed', 'export_started', 'export_completed',
        'install_started', 'install_completed', 'upgrade_clicked', 'checkout_started',
        'campaign_created', 'campaign_scheduled', 'campaign_published',
        'analytics_viewed', 'settings_updated', 'billing_viewed',
        'ai_prompt_submitted', 'ai_edit_completed', 'ai_edit_failed',
        'referral_arrived', 'outbound_link_clicked', 'generation_regenerated',
        'signup_failed', 'generated_signature_claimed', 'editor_action',
        'editor_preview_opened', 'signature_published',
        'install_instructions_viewed', 'install_step_completed', 'install_failed',
        'install_abandoned', 'test_email_started', 'test_email_completed',
        'signature_verified', 'upgrade_prompt_viewed', 'checkout_returned',
        'team_invite_sent'
    )),

    -- UUID aléatoires créés dans le navigateur. Ce ne sont ni une IP, ni une empreinte.
    visitor_id           uuid NOT NULL,
    analytics_session_id uuid NOT NULL,

    -- Toujours dérivés du cookie HttpOnly par le serveur, jamais acceptés dans le JSON.
    -- SET NULL rend la suppression d'un compte effective sans détruire les agrégats.
    user_id              uuid REFERENCES users(id) ON DELETE SET NULL,
    org_id               uuid REFERENCES orgs(id) ON DELETE SET NULL,
    plan                 text CHECK (plan IS NULL OR plan IN ('free', 'pro', 'team')),

    -- Objet plat, liste de clés fermée et taille vérifiée une deuxième fois par PostgreSQL.
    properties           jsonb NOT NULL DEFAULT '{}'::jsonb
                         CHECK (jsonb_typeof(properties) = 'object')
                         CHECK (octet_length(properties::text) <= 4096),

    -- Acquisition sans URL complète : aucun query string, jeton ou e-mail ne peut entrer.
    page_path            text CHECK (page_path IS NULL OR length(page_path) <= 256),
    referrer_host        text CHECK (referrer_host IS NULL OR length(referrer_host) <= 253),
    utm_source           text CHECK (utm_source IS NULL OR length(utm_source) <= 64),
    utm_medium           text CHECK (utm_medium IS NULL OR length(utm_medium) <= 64),
    utm_campaign         text CHECK (utm_campaign IS NULL OR length(utm_campaign) <= 128),
    utm_content          text CHECK (utm_content IS NULL OR length(utm_content) <= 128),
    utm_term             text CHECK (utm_term IS NULL OR length(utm_term) <= 128),

    -- Classification serveur uniquement ; le User-Agent brut n'est jamais conservé.
    browser_family       text CHECK (browser_family IS NULL OR browser_family IN
                         ('chrome', 'edge', 'firefox', 'safari', 'other', 'bot')),
    device_type          text CHECK (device_type IS NULL OR device_type IN
                         ('desktop', 'mobile', 'tablet', 'other', 'bot')),
    country_code         text CHECK (country_code IS NULL OR country_code ~ '^[A-Z]{2}$'),
    language             text CHECK (language IS NULL OR length(language) <= 16),

    occurred_at          timestamptz NOT NULL,
    received_at          timestamptz NOT NULL DEFAULT now()
);

-- Funnel global et taux de conversion par événement.
CREATE INDEX product_events_name_time_idx
    ON product_events (name, occurred_at DESC);
-- Parcours anonyme, time-to-value et cohortes de première visite.
CREATE INDEX product_events_visitor_time_idx
    ON product_events (visitor_id, occurred_at);
CREATE INDEX product_events_session_time_idx
    ON product_events (analytics_session_id, occurred_at);
-- Activation, rétention et conversion après authentification.
CREATE INDEX product_events_user_time_idx
    ON product_events (user_id, occurred_at DESC) WHERE user_id IS NOT NULL;
CREATE INDEX product_events_org_name_time_idx
    ON product_events (org_id, name, occurred_at DESC) WHERE org_id IS NOT NULL;
-- Attribution et comparaison de la qualité des canaux.
CREATE INDEX product_events_acquisition_idx
    ON product_events (utm_source, utm_medium, occurred_at DESC)
    WHERE utm_source IS NOT NULL;
-- Les purges et scans temporels restent peu coûteux lorsque le volume grandit.
CREATE INDEX product_events_received_brin_idx
    ON product_events USING brin (received_at);

-- À appeler quotidiennement depuis la maintenance Siglair. La limite évite une transaction
-- géante au premier déploiement ; répéter jusqu'à 0 supprime tout ce qui dépasse 13 mois.
CREATE OR REPLACE FUNCTION purge_product_events(
    p_before timestamptz DEFAULT now() - interval '13 months',
    p_limit integer DEFAULT 10000
) RETURNS bigint
LANGUAGE plpgsql
AS $$
DECLARE
    deleted_count bigint;
BEGIN
    WITH doomed AS (
        SELECT event_id
        FROM product_events
        WHERE received_at < p_before
        ORDER BY received_at
        LIMIT LEAST(GREATEST(p_limit, 1), 50000)
    )
    DELETE FROM product_events e
    USING doomed d
    WHERE e.event_id = d.event_id;

    GET DIAGNOSTICS deleted_count = ROW_COUNT;
    RETURN deleted_count;
END;
$$;
