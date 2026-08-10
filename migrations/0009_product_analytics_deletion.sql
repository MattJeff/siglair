-- Les événements authentifiés suivent le cycle de vie du compte et de l'organisation.
-- Cette migration séparée garde 0008 immuable après son test sur une base locale.

ALTER TABLE product_events
    DROP CONSTRAINT product_events_user_id_fkey,
    ADD CONSTRAINT product_events_user_id_fkey
        FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE;

ALTER TABLE product_events
    DROP CONSTRAINT product_events_org_id_fkey,
    ADD CONSTRAINT product_events_org_id_fkey
        FOREIGN KEY (org_id) REFERENCES orgs(id) ON DELETE CASCADE;
