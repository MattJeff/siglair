/** Palette d'ajout : clic pour poser au centre, ou glisser-déposer sur le canvas. */
import type { Dispatch } from 'react';
import type { ElementType } from '../lib/types';
import type { Action } from './state';
import s from './editor.module.css';

const CARDS: { kind: ElementType; icon: string; title: string; hint: string }[] = [
  { kind: 'text', icon: 'T', title: 'Texte', hint: 'Nom, poste, jeton {{…}}' },
  { kind: 'button', icon: '↗', title: 'Bouton', hint: 'Lien cliquable et suivi' },
  { kind: 'image', icon: '▧', title: 'Image / GIF', hint: 'Logo, photo, GIF animé' },
  { kind: 'video', icon: '▶', title: 'Vidéo', hint: 'Aperçu, image au rendu' },
  { kind: 'badge', icon: '●', title: 'Badge', hint: 'LIVE, nouveau, réseau' },
  { kind: 'shape', icon: '▰', title: 'Bloc', hint: 'Fond décoratif' },
  { kind: 'divider', icon: '━', title: 'Ligne', hint: 'Séparateur' },
  { kind: 'banner', icon: '⚡', title: 'Bannière', hint: 'Campagne datée' },
];

export function Palette({ dispatch }: { dispatch: Dispatch<Action> }) {
  return (
    <div className={s.elementGrid}>
      {CARDS.map((card) => (
        <button
          key={card.kind}
          type="button"
          className={s.elementCard}
          draggable
          onDragStart={(event) => event.dataTransfer.setData('text/plain', card.kind)}
          onClick={() => dispatch({ type: 'add', kind: card.kind })}
        >
          <span className={s.elIcon} aria-hidden="true">
            {card.icon}
          </span>
          <b>{card.title}</b>
          <small>{card.hint}</small>
        </button>
      ))}
    </div>
  );
}
