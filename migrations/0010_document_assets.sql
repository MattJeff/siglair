-- Les chercheurs d'emploi peuvent joindre un CV PDF à leur signature. Le document est
-- servi en téléchargement, jamais interprété comme du HTML ni inséré dans le canvas.

ALTER TABLE assets DROP CONSTRAINT assets_kind_check;
ALTER TABLE assets
    ADD CONSTRAINT assets_kind_check CHECK (kind IN ('image', 'video', 'document'));
