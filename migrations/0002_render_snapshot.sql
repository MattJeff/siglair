-- Deux défauts révélés à l'intégration du backend. Voir docs/CONTRACT.md §5.1 et §7.1.

-- 1. /c/{slug}/{element_id} résolvait la cible du clic dans le BROUILLON courant.
--    Le contrat §5.1 exige le document PUBLIÉ : sans ça, modifier un brouillon change
--    la destination de liens déjà partis dans des emails, sans republication et sans
--    trace. C'est aussi la garantie qui empêche notre domaine de devenir un
--    redirecteur ouvert : la cible doit venir d'un état figé et daté.
--
-- 2. render_jobs.doc_hash ne hachait que le doc, pas le profil. Changer sa fonction
--    dans Réglages puis republier réutilisait le GIF en cache : l'ancienne fonction
--    restait affichée indéfiniment. Le hash porte désormais sur (doc, profile) ;
--    le nom de colonne reste doc_hash, il désigne l'empreinte du rendu.

ALTER TABLE renders
    ADD COLUMN doc     jsonb,
    ADD COLUMN profile jsonb;

-- Instantané exact de ce qui a été rasterisé. C'est cette copie que lit /c/ et /s/,
-- jamais signatures.doc, qui continue de vivre pendant que l'utilisateur édite.
COMMENT ON COLUMN renders.doc IS
    'Document figé au moment du rendu. Source de vérité de /c/{slug}/{element_id}.';
COMMENT ON COLUMN renders.profile IS
    'Profil figé au moment du rendu, pour rejouer le rendu à l''identique.';

-- Les rendus antérieurs à cette migration n'ont pas d'instantané : ils restent servis
-- (le GIF existe), mais leurs clics retombent sur le brouillon jusqu'à republication.
-- Rien à rétro-remplir : republier régénère proprement.
