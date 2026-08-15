# Prospection — organismes de formation certifiés Qualiopi

## Source

- Jeu de données : **Liste Publique des Organismes de Formation (L.6351-7-1 du Code du Travail)**
  data.gouv.fr, id `582c8978c751df788ec0bb7e`, producteur Ministère du Travail (DGEFP).
  https://www.data.gouv.fr/datasets/liste-publique-des-organismes-de-formation-l-6351-7-1-du-code-du-travail
- Fichier téléchargé (CSV, `;`, UTF-8, 38,2 Mo) :
  https://www.monactiviteformation.emploi.gouv.fr/mon-activite-formation/public/listePubliqueOF?format=csv
- Mise à jour : quotidienne. Version utilisée : ressource datée du **2026-08-14**, téléchargée le 2026-08-15.
- Licence : ouverte (data.gouv.fr). Gratuit, réutilisable.

Reproduire : `curl -sL -o listeOF.csv "<url csv>" && python3 build.py listeOF.csv organismes-qualiopi.csv`
(stdlib uniquement, lecture en streaming ligne à ligne).

## Le chiffre de 42 892 : faux

Le fichier contient **160 934 organismes déclarés**, dont **45 368 certifiés Qualiopi**
(au moins un périmètre de certification actif) au 14/08/2026. Le chiffre de 42 892 ne correspond
à rien dans ce fichier — il est probablement issu d'un export plus ancien. Le bon chiffre à citer
est 45 368, et il bouge tous les jours.

## Filtre appliqué

| Étape | Lignes |
|---|---|
| Lignes en entrée (hors en-tête) | 160 934 |
| Écartées : ligne malformée (guillemet non échappé, « UFORCA NANTES ») | 1 |
| Écartées : aucun périmètre Qualiopi actif | 115 565 |
| **Certifiés Qualiopi** | **45 368** |
| Écartées : `effectifFormateurs` < 3 (0, 1 ou 2) | 21 502 |
| **Lignes en sortie** | **23 866** |

« Certifié » = au moins un `true` parmi les 4 colonnes `certifications.*` du fichier
(actionsDeFormation, bilansDeCompetences, VAE, actionsDeFormationParApprentissage).
Le périmètre exact est conservé dans la colonne `perimetre_certification`.

**Bonne nouvelle sur le critère de taille** : le fichier contient bien
`informationsDeclarees.effectifFormateurs` — le nombre de formateurs déclaré au Bilan Pédagogique
et Financier. Pas besoin d'indicateur de substitution. Il est renseigné (numérique) pour 100 % des
organismes certifiés. `nb_stagiaires` est conservé en colonne secondaire pour qualifier le volume
d'activité.

Répartition en sortie : médiane 10 formateurs, max 11 743, 12 225 organismes à 10 formateurs ou plus.
Top régions : Île-de-France 6 862, Auvergne-Rhône-Alpes 3 102, Occitanie 2 222.

## Colonnes du CSV

`raison_sociale, siren, siret, region, ville, code_postal, specialite_principale,
effectif_formateurs, nb_stagiaires, perimetre_certification, num_declaration_activite,
date_derniere_declaration`

Trié par `effectif_formateurs` décroissant, puis `nb_stagiaires` décroissant.

## Ce que la donnée NE dit PAS

- **Aucun site web, aucun email, aucun téléphone.** Ces champs n'existent pas dans le fichier
  DGEFP. Il faut les enrichir ailleurs (SIRENE / annuaire-entreprises, scraping, ou un outil
  d'enrichissement) à partir du SIREN/SIRET, qui sont eux fiables.
- **`effectifFormateurs` n'est pas l'effectif salarié.** C'est le nombre de formateurs déclarés,
  internes + sous-traitants confondus. Un organisme à 8 formateurs peut être une seule personne
  qui pilote 7 indépendants. C'est un proxy de volume d'activité, pas de taille d'entreprise.
  Pour l'effectif salarié réel, croiser avec la tranche d'effectif SIRENE via le SIREN.
- **Adresse souvent vide** : `ville` et `code_postal` sont vides sur 10 502 lignes (44 %).
  L'adresse est récupérable via le SIRET dans SIRENE.
- **Déclaratif et daté** : les chiffres viennent du dernier BPF transmis par l'organisme
  (exercice N-1 le plus souvent). 42 145 déclarations datent de 2026, 3 112 de 2025, le reste
  est plus ancien.
- **Une ligne = une déclaration d'activité, pas une entreprise.** 45 309 SIREN uniques pour
  45 368 certifiés : quelques groupes déposent plusieurs déclarations (jusqu'à 7).
- **Pas de date d'obtention ni d'expiration du Qualiopi**, pas de nom du certificateur,
  pas de chiffre d'affaires.
- Le fichier ne liste que les organismes **à jour de leur obligation de BPF** : un OF certifié
  mais en retard de déclaration en est absent.
