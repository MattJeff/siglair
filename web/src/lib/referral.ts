/**
 * Code de parrainage (contrat §11.3) : `/r/{slug}` → `/?ref={code}` → `/login?ref={code}`
 * → départ OAuth / demande de lien magique → `users.referred_by` à la création du compte.
 *
 * **Aucun cookie.** Le destinataire de l'e-mail n'est pas notre utilisateur, il n'a rien
 * accepté : un cookie d'attribution relève d'ePrivacy et impose un bandeau de consentement,
 * lequel ferait chuter la conversion qu'on cherche précisément à mesurer. Le paramètre
 * d'URL donne la même information, sans bandeau.
 *
 * Pourquoi un relais en `sessionStorage` malgré tout : depuis la landing, « Commencer »
 * pointe vers un `/login?next=...` fixe (`marketing/Chrome.tsx`, hors de ce lot), et le
 * paramètre serait perdu au premier clic. Ce stockage-là est celui du **visiteur qui
 * s'inscrit** — il devient utilisateur, la donnée est fonctionnelle et disparaît à la
 * fermeture de l'onglet. Jamais `localStorage` : persistant, donc du suivi.
 *
 * Rien de tout ceci ne s'affiche : c'est de la mesure interne, pas un message à l'utilisateur.
 */

const KEY = 'siglair:ref';

/**
 * Forme d'un slug public (`util::gen_slug` : 12 caractères alphanumériques ; la borne large
 * encaisse un futur format). On valide la **forme**, ce qui suffit : un code refusé n'est
 * jamais mémorisé, et un code accepté ne contient que `[a-z0-9]`.
 */
const SHAPE = /^[a-z0-9]{6,32}$/i;

/** `public_slug` est `citext` côté base : la casse n'a pas à voyager. */
function valid(raw: string | null): string | null {
  return raw && SHAPE.test(raw) ? raw.toLowerCase() : null;
}

function stored(): string | null {
  try {
    return valid(sessionStorage.getItem(KEY));
  } catch {
    return null;
  }
}

/**
 * Mémorise le code lu dans l'URL et renvoie celui qui vaut pour cette navigation
 * (le paramètre s'il est valide, sinon celui déjà retenu dans l'onglet).
 */
export function captureRef(raw: string | null): string | null {
  const code = valid(raw);
  if (code) {
    try {
      sessionStorage.setItem(KEY, code);
    } catch {
      // Navigation privée verrouillée : c'est l'attribution qui saute, jamais la page.
    }
  }
  return code ?? stored();
}

/** Accroche le code à une URL de départ d'authentification (OAuth : navigation pleine page). */
export function withRef(url: string, code: string | null): string {
  return code ? `${url}${url.includes('?') ? '&' : '?'}ref=${encodeURIComponent(code)}` : url;
}
