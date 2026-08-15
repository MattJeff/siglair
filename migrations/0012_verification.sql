-- Page de vérification publique — l'hypothèse à tester.
--
-- Constat qui la motive : aucune source indépendante des éditeurs de logiciels de signature ne
-- démontre qu'une bannière de signature produit des clics. Toutes les statistiques du secteur
-- sont publiées par des vendeurs de signatures. En revanche, un fait est documenté par
-- cybermalveillance.gouv.fr : la fraude au faux RIB repose sur l'imitation de la signature, du
-- logo et du ton d'un correspondant légitime. Le destinataire n'a aujourd'hui AUCUN moyen de
-- vérifier qu'un e-mail vient bien de qui il prétend.
--
-- D'où une ligne de TEXTE en signature — jamais une image, elle doit survivre à Outlook, aux
-- images bloquées, au transfert et à l'impression — qui mène à une page publique disant qui est
-- cette personne et ce que sa structure ne demandera jamais par e-mail.
--
-- Ce qui reste à prouver et que cette migration ne prouve pas : qu'un destinataire clique. La
-- meilleure donnée disponible pointe dans l'autre sens (étude d'oculométrie anti-hameçonnage,
-- n=78 : les utilisateurs ordinaires n'inspectent jamais l'identité de l'expéditeur). D'où le
-- réglage par organisation, désactivé par défaut : on l'allume sur une seule signature, on
-- regarde /c/, et on décide ensuite.

ALTER TABLE orgs
    -- Désactivé par défaut : tant que le clic n'est pas observé, on n'impose une ligne
    -- supplémentaire dans la signature de personne.
    ADD COLUMN verify_link boolean NOT NULL DEFAULT false,
    -- La phrase que la structure veut opposer à la fraude, telle qu'elle l'écrit.
    -- Exemple attendu : « Nous ne demandons jamais de changement de coordonnées bancaires
    -- par e-mail. » NULL = la page n'affiche pas d'encadré d'avertissement.
    ADD COLUMN verify_notice text
        CONSTRAINT orgs_verify_notice_len CHECK (verify_notice IS NULL OR length(verify_notice) <= 300);
