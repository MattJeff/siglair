/** Panneau de calques : l'ordre du tableau EST le z-index, index 0 = derrière (contrat §3). */
import type { Dispatch } from 'react';
import { LockKeyhole, Trash2 } from 'lucide-react';
import type { Doc, ElementType, Profile } from '../lib/types';
import { FREE_BRANDING_LABEL } from './BrandingUpsell';
import { elementLabel } from './state';
import type { Action } from './state';
import s from './editor.module.css';

const ICONS: Record<ElementType, string> = {
  text: 'T',
  button: '↗',
  image: '▧',
  video: '▶',
  badge: '●',
  shape: '▰',
  divider: '━',
  banner: '⚡',
};

interface LayersProps {
  doc: Doc;
  selectedId: string | null;
  dispatch: Dispatch<Action>;
  profile: Profile;
  branding: boolean;
  onBrandingAttempt: () => void;
}

export function Layers({ doc, selectedId, dispatch, profile, branding, onBrandingAttempt }: LayersProps) {
  // Affichage du premier plan vers l'arrière-plan : c'est l'ordre visuel attendu.
  const rows = [...doc.elements].reverse();

  if (rows.length === 0 && !branding) {
    return <p className={s.note}>Aucun calque. Ajoutez un élément depuis la palette ci-dessus.</p>;
  }

  return (
    <ul className={s.layers}>
      {branding && (
        <li className={`${s.layer} ${s.brandingLayer}`}>
          <button type="button" className={s.layerSelect} onClick={onBrandingAttempt}>
            <span className={s.layerIcon} aria-hidden="true">
              S
            </span>
            <span className={s.layerName}>{FREE_BRANDING_LABEL}</span>
            <LockKeyhole size={13} aria-label="Verrouillé sur Free" />
          </button>
          <button
            type="button"
            className={s.layerBtn}
            aria-label="Retirer la mention Siglair"
            title="Retirer avec Pro"
            onClick={onBrandingAttempt}
          >
            <Trash2 size={14} />
          </button>
        </li>
      )}
      {rows.map((element, reverseIndex) => {
        const index = doc.elements.length - 1 - reverseIndex;
        const name = elementLabel(element, profile);
        return (
          <li
            key={element.id}
            className={[s.layer, element.id === selectedId ? s.layerActive : null].filter(Boolean).join(' ')}
          >
            <button
              type="button"
              className={s.layerSelect}
              aria-pressed={element.id === selectedId}
              onClick={() => dispatch({ type: 'select', id: element.id })}
            >
              <span className={s.layerIcon} aria-hidden="true">
                {ICONS[element.type]}
              </span>
              <span className={s.layerName}>{name}</span>
              {element.anim.preset !== 'none' && (
                <span className={s.layerAnim} title={`Animation : ${element.anim.preset}`}>
                  ✦<span className={s.srOnly}> animé : {element.anim.preset}</span>
                </span>
              )}
            </button>

            <button
              type="button"
              className={s.layerBtn}
              aria-pressed={element.locked}
              aria-label={`${element.locked ? 'Déverrouiller' : 'Verrouiller'} ${name}`}
              onClick={() => dispatch({ type: 'update', id: element.id, patch: { locked: !element.locked } })}
            >
              {element.locked ? '🔒' : '🔓'}
            </button>
            <button
              type="button"
              className={s.layerBtn}
              aria-pressed={element.hidden}
              aria-label={`${element.hidden ? 'Afficher' : 'Masquer'} ${name}`}
              onClick={() => dispatch({ type: 'update', id: element.id, patch: { hidden: !element.hidden } })}
            >
              {element.hidden ? '🙈' : '👁'}
            </button>
            <button
              type="button"
              className={s.layerBtn}
              disabled={index === doc.elements.length - 1}
              aria-label={`Monter ${name}`}
              onClick={() => dispatch({ type: 'reorder', id: element.id, to: 'up' })}
            >
              ↑
            </button>
            <button
              type="button"
              className={s.layerBtn}
              disabled={index === 0}
              aria-label={`Descendre ${name}`}
              onClick={() => dispatch({ type: 'reorder', id: element.id, to: 'down' })}
            >
              ↓
            </button>
          </li>
        );
      })}
    </ul>
  );
}
