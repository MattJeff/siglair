-- Cycle de vie d'un abonnement impayé — src/billing/lifecycle.rs.
--
--     active → past_due → (grâce 7 jours, accès COMPLET) → unpaid → free
--
-- Une carte réémise par la banque est le cas le plus banal du monde. Couper le service
-- d'un client qui paie depuis six mois parce que son numéro a changé est le meilleur
-- moyen de le perdre : pendant la grâce, l'accès reste entier et on relance par e-mail.

ALTER TABLE orgs
    -- Fin de la période de grâce. NULL = aucun impayé en cours. C'est cette date, et non
    -- le statut Stripe, qui décide de l'accès : Stripe passe `unpaid` selon SA propre
    -- politique de relance, qui n'est pas notre promesse commerciale.
    ADD COLUMN grace_until         timestamptz,
    -- Nombre de relances déjà parties (0..3, cf. DUNNING_DAYS). Sert de verrou : la
    -- relance n'est envoyée que si l'UPDATE conditionnel gagne, donc un webhook rejoué
    -- ou deux balayages concurrents n'envoient pas deux fois le même e-mail.
    ADD COLUMN dunning_emails_sent integer NOT NULL DEFAULT 0,
    -- 3-D Secure : `invoice.payment_action_required`. Le paiement n'est ni accepté ni
    -- refusé, il attend l'authentification bancaire du client. Sans ce drapeau, l'écran
    -- de facturation affiche « actif » et l'utilisateur ne voit rien à faire.
    ADD COLUMN requires_action     boolean NOT NULL DEFAULT false;

-- Le balayage ne lit que les organisations en grâce : index partiel, pas de scan complet.
CREATE INDEX orgs_grace_idx ON orgs (grace_until) WHERE grace_until IS NOT NULL;
