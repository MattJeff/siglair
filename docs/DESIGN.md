# Siglair — contrat visuel

Deux maquettes existent et font foi. On les reprend, on ne les réinvente pas :

| Fichier | Ce qu'il définit |
|---|---|
| `docs/reference/landing-viral.html` | **La marque** : voix, structure de la vitrine, palette |
| `docs/reference/editor-original.html` | **L'outil** : densité, contrôles, palette de travail |

## Jetons — `web/src/styles/tokens.css`, une seule fois

Les deux maquettes ont des palettes proches mais pas identiques. C'est volontaire et on le
garde : la vitrine respire et brille, l'outil est dense et se fait oublier. Un même bleu
saturé qui fonctionne sur un héros de 88 px fatigue sur une barre d'outils de 10 px.

```css
:root {
  /* Marque — issus de landing-viral.html. Vitrine, auth, facturation, emails. */
  --bg:#060911;       --bg2:#0b1020;
  --panel:#0f1628;    --panel2:#111a31;
  --text:#f8fbff;     --muted:#9ca9bd;
  --line:rgba(255,255,255,.10);
  --blue:#4c74ff;     --blue2:#1f5bff;   --cyan:#4de1ff;
  --purple:#8a5cff;   --green:#37d995;   --yellow:#ffbd58;  --red:#fb7185;
  --shadow:0 30px 90px rgba(0,0,0,.45);
  --r-sm:8px; --r:13px; --r-md:15px; --r-lg:22px;
}

/* Outil — issus de editor-original.html. Portée à .app-surface uniquement. */
.app-surface {
  --bg:#070a10;       --panel:#0c1119;   --panel2:#101723;  --panel3:#151e2d;
  --line:#1e2938;     --line2:#2b3a50;
  --text:#f7f9fc;     --muted:#8390a4;
  --blue:#3b6cff;     --blue2:#5b8cff;   --cyan:#38c8ff;
  --shadow:0 28px 90px rgba(0,0,0,.42);
  --r-sm:7px; --r:9px; --r-md:12px; --r-lg:18px;
}
```

Deux surfaces, un seul système : les composants partagés lisent les mêmes noms de variables
et s'adaptent sans savoir où ils sont. Aucune couleur en dur ailleurs que dans ce fichier —
un `#4c74ff` écrit dans un composant est un bug de maintenance : le jour où la marque change,
il faut le retrouver.

Éléments de marque à reprendre tels quels depuis la landing : l'anneau du logo
(`.brand-ring`, gradient masqué), `.gradient-text`, le fond en `radial-gradient` du body,
le bouton primaire `linear-gradient(135deg,#315cff,#527cff 55%,#3bb8ff)`.

## Règles

- **Sombre par défaut**, c'est l'identité du produit. Pas de thème clair en v1 — personne ne
  l'a demandé, et deux thèmes doublent la surface de test CSS.
- Police : la pile système — `Inter, ui-sans-serif, -apple-system, BlinkMacSystemFont,
  "Segoe UI", Arial`. **Aucun fichier de police n'est embarqué et aucune n'est chargée
  depuis un tiers.** Sur macOS on obtient SF Pro, sur Windows Segoe UI : deux polices
  excellentes, indiscernables d'Inter pour un visiteur, à zéro octet et zéro latence.
  Charger Inter depuis Google Fonts serait un problème RGPD gratuit ; l'embarquer coûterait
  ~60 Ko sur le chemin critique de la landing pour un gain que personne ne verra.
  Si la marque exige un jour Inter au pixel près : déposer les `.woff2` dans
  `web/public/fonts/` et ajouter les `@font-face`. Un fichier, aucune autre modification.
- Densité : l'éditeur est dense (10–13 px), la landing est aérée (16–18 px de corps).
  Ce sont deux registres différents et c'est volontaire.
- Rayons : 9 px pour les contrôles, 12–18 px pour les cartes et panneaux.
- Bouton primaire : `linear-gradient(135deg,#2e59f2,#5e8cff)`, sans bordure, `font-weight:700`.
- Transitions ≤ 160 ms. Au-delà, l'interface paraît lente, pas fluide.

## Accessibilité — non négociable

- Contraste AA minimum sur tout texte. `--muted` (#8390a4) sur `--bg` passe à 14 px et plus ;
  ne pas l'utiliser en dessous.
- Tout élément interactif est un `<button>` ou un `<a>` réel, focalisable, avec un état
  `:focus-visible` visible (anneau `--blue2`, 2 px). Une `<div onClick>` est inaccessible
  au clavier et invisible pour un lecteur d'écran.
- Zone tactile ≥ 40 px sur mobile, y compris dans la barre d'outils de l'éditeur.
- `prefers-reduced-motion: reduce` coupe les animations décoratives de l'interface.
  Les animations *de la signature* en cours d'édition restent : c'est le contenu, pas la
  décoration, et l'utilisateur a besoin de les voir pour travailler.
- Chaque champ a un `<label>` associé. Un placeholder n'est pas un label.

## Structure du site

```
/                    landing — promesse, démo animée, preuve, tarifs, FAQ, CTA
/pricing             tarifs détaillés + comparatif
/login               magic link + Google + Apple (boutons masqués si non configurés)
/invite/:token       acceptation d'invitation
/app                 tableau de bord — liste des signatures, état de publication
/app/editor/:id      l'éditeur
/app/analytics/:id   ouvertures, clics, top éléments
/app/team            membres, invitations, rollout (plan Team)
/app/billing         abonnement, changement de plan, portail Stripe
/app/settings        profil, jetons {{...}}, suppression de compte
/legal/*             CGU, confidentialité, mentions légales
```

## Landing — on porte `docs/reference/landing-viral.html`, on ne réécrit pas

**La landing existe et elle est bonne.** Le travail est un PORTAGE en React, pas une création.
Sa structure, ses titres, sa voix et ses animations sont validés : on les garde au mot près
sauf sur les points listés ci-dessous.

Structure à conserver telle quelle : nav collante → héros (copy + maquette d'éditeur animée
avec curseur et pastille « Attends… ta signature bouge ? ») → Le problème (avant/après) →
Fonctions (6 cartes) → Le moment wow (4 étapes) → Motion engine (presets + compatibilité) →
Pour qui (4 publics) → Tarifs → FAQ → CTA final → pied de page.

Réutiliser aussi : `@keyframes spin/float/float2/cursorMove`, `.reveal` à l'IntersectionObserver,
la maquette `.browser`/`.demo-body`, la comparaison `.mail-sig` avant/après.

### Ce qui DOIT changer dans le portage

1. **Tous les liens `./signature-studio-motion-lab.html`** → `/login?next=/app`. Il y en a
   six. C'est un prototype qui pointait vers un fichier local.
2. **Prix** → grille définitive du CONTRACT.md §6 (7,90 € / 5,90 € par membre), lue depuis
   l'API, jamais écrite en dur. Supprimer « Tarifs indicatifs pour visualiser le
   positionnement marketing de la landing ».
3. **Pied de page** « Prototype marketing » → un vrai pied de page : produit, tarifs, légal
   (CGU, confidentialité, mentions), contact.
4. **Retirer la mention « Brand Kit »** de la liste Pro : la fonctionnalité n'existe pas.
   Ne jamais afficher une case non implémentée.
5. **Formulations de prototype** dans la FAQ et le JSON-LD (« Le positionnement proposé pour
   Siglair inclut… », « Siglair est pensé autour d'une logique de… ») → affirmations directes
   au présent. On décrit un produit qui existe, pas une intention.
6. **JSON-LD FAQPage** : ses trois questions ne correspondent pas à celles affichées. Google
   sanctionne le balisage qui ne reflète pas la page. Générer le JSON-LD **depuis** le tableau
   de FAQ affiché, pour qu'ils ne puissent plus diverger.
7. **Accessibilité de la FAQ** : `.faq-q` est une `<div>` avec un `onClick`, donc inaccessible
   au clavier et invisible pour un lecteur d'écran. Passer en `<details>/<summary>` natif —
   au passage, plus de JavaScript à écrire.
8. **`<meta name="keywords">`** : ignoré par les moteurs depuis quinze ans. Supprimer.
9. **`canonical` / `og:url`** : lire le domaine depuis une variable d'environnement Vite.
   Le domaine n'est pas encore acheté ; le coder en dur garantit de l'oublier.
10. **`og:image`** est déclaré côté Twitter mais aucune image n'est fournie. Soit produire
    une vraie image d'aperçu dans `web/public/`, soit retirer la balise. Une balise cassée
    donne un lien laid sur LinkedIn et Slack — précisément là où ce produit se partage.
11. **`prefers-reduced-motion`** : le curseur animé, les `float` et le `spin` doivent
    s'arrêter. Rien dans la maquette ne le gère aujourd'hui.
12. **La signature de démonstration** utilise les vraies coordonnées de Mathis Higuinen
    (email, téléphone). Sur une page publique indexée, c'est une invitation au spam.
    Remplacer par un personnage fictif cohérent, et garder les vraies coordonnées uniquement
    dans les données de démonstration de l'application connectée.

### Le héros change — c'est la modification la plus importante du portage

L'accroche n'est plus « créez une signature animée », c'est la promesse du contrat §6bis :

> **Transformez votre site en signature email.**
> `https://votreentreprise.com                       ✨ Générer`
> *L'IA la dessine. Vous gardez la main sur tout.*

Une grande barre d'URL, un bouton, rien d'autre au-dessus de la ligne de flottaison.
C'est la promesse la plus courte et la plus démontrable du produit : remplir un formulaire
de quinze champs ne se filme pas, coller une URL si.

Le champ est **fonctionnel depuis la page d'accueil, sans compte** : il appelle
`POST /api/onboarding/analyze` et affiche en direct ce qui a été reconnu — logo, couleurs,
nom de marque — avec des étapes honnêtes (« Analyse de votre marque… » → « Logo récupéré »
→ « Couleurs détectées »). Voir sa propre marque apparaître avant même de s'inscrire, c'est
le moment qui convertit. La génération des trois propositions, elle, demande un compte.

Prévoir et soigner les cas d'échec, ils seront fréquents : site injoignable, pas de logo
trouvé, URL invalide, domaine privé refusé par le garde SSRF. Chacun a un message clair et
une porte de sortie (« Continuez sans logo, vous l'ajouterez dans l'éditeur »). Un héros
qui plante sur une URL exotique perd le visiteur au premier geste qu'on lui a demandé.

Garder l'ancienne accroche (« Vos emails finissent. Votre marque, non. ») juste en dessous,
en sous-titre : elle est bonne, elle n'est simplement plus le premier mot.

À droite, la maquette d'éditeur `.browser` de la référence, inchangée.

### Ce qu'il faut AJOUTER — une section, entre « Le moment wow » et « Motion engine »

La landing vend le coup de cœur ; il manque la raison de payer **tous les mois**. Une section
sur l'URL hébergée, dans la même voix :

- On colle **une fois**. Ensuite la signature se met à jour toute seule, dans les emails
  qu'on n'a pas encore envoyés.
- Toute l'équipe change de signature en un clic, sans qu'un seul collaborateur ne touche à
  son client mail. C'est ce que vend le plan Team.
- On voit ce que ça rapporte : ouvertures et clics, par élément.

Montrer visuellement le `<img src="siglair.app/s/…gif">` : c'est concret, et c'est la preuve
qu'il n'y a rien à réinstaller.

### Honnêteté — non négociable

La maquette est déjà honnête sur Outlook Legacy (« Fallback statique », pastille orange) :
**garder cette nuance**. Dire la limite augmente la crédibilité ; la cacher garantit un
remboursement et un avis négatif.

Pas de faux logos clients, pas de « +10 000 entreprises nous font confiance », pas de faux
témoignages, pas de compteur inventé. Un chiffre mensonger sur une landing est une pratique
commerciale trompeuse — et c'est le genre de détail qui se remarque. Tant qu'il n'y a pas de
vrais clients, la section preuve sociale n'existe pas.
