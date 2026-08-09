/**
 * Variables d'environnement Vite (DESIGN.md §9).
 * Typées ici plutôt que lues via l'index signature de `vite/client`, qui renvoie `any`.
 */
interface ImportMetaEnv {
  /** Domaine public servant les GIF, ex. `siglair.app`. Absent = valeur de repli. */
  readonly VITE_PUBLIC_HOST?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
