/**
 * Les 3 propositions, animées et à taille réelle (contrat §6bis, étape 4).
 *
 * Le HTML vient de `POST /api/preview` et s'affiche dans une <iframe srcDoc sandbox> : le
 * rendu est écrit une seule fois, en Rust (§2). Le reconstruire ici garantirait de diverger,
 * et l'écart se découvrirait dans l'Outlook d'un client.
 */
import { useEffect, useState } from 'react';
import { Button } from '../components/Button';
import { Spinner } from '../components/Spinner';
import { previewDoc } from '../lib/api';
import type { OnboardingVariant, Profile } from '../lib/types';
import s from './onboarding.module.css';

/** Une proposition affichable, c'est exactement ce que rend `/api/onboarding/generate`. */
export type Proposal = OnboardingVariant;

interface VariantCardsProps {
  proposals: Proposal[];
  profile: Profile;
  /** Index de la proposition en cours de création, sinon null. */
  picking: number | null;
  onPick: (index: number) => void;
}

export function VariantCards({ proposals, profile, picking, onPick }: VariantCardsProps) {
  // '' = pas encore chargé, null = aperçu indisponible (on laisse quand même choisir).
  const [html, setHtml] = useState<Record<number, string | null>>({});

  useEffect(() => {
    let alive = true;
    void Promise.all(
      proposals.map(async (p) => {
        const result = await previewDoc(p.doc, profile).catch(() => null);
        return [p.index, result?.html ?? null] as const;
      }),
    ).then((pairs) => {
      if (alive) setHtml(Object.fromEntries(pairs));
    });
    return () => {
      alive = false;
    };
  }, [proposals, profile]);

  return (
    <ul className={s.variants}>
      {proposals.map((p) => {
        const preview = html[p.index];
        const { width, height } = p.doc.canvas;
        return (
          <li className={s.variant} key={p.index}>
            <h3 className={s.variantName}>{p.name || `Proposition ${p.index + 1}`}</h3>

            <div className={s.frame} style={{ width, height }}>
              {preview === undefined && <Spinner size={20} label="Chargement de l’aperçu" />}
              {preview === null && (
                <p className={s.muted}>
                  L’aperçu n’a pas pu être chargé. La proposition reste utilisable.
                </p>
              )}
              {preview && (
                <iframe
                  className={s.iframe}
                  title={`Aperçu — ${p.name}`}
                  sandbox=""
                  width={width}
                  height={height}
                  // L'animation EST le contenu : elle joue même en mouvement réduit (DESIGN.md).
                  data-motion="keep"
                  srcDoc={preview}
                />
              )}
            </div>

            <p className={s.rationale}>{p.rationale}</p>

            <Button
              onClick={() => onPick(p.index)}
              loading={picking === p.index}
              disabled={picking !== null}
            >
              Choisir
            </Button>
          </li>
        );
      })}
    </ul>
  );
}
