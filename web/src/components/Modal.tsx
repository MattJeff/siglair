import { useEffect, useId, useRef } from 'react';
import type { MouseEvent, ReactNode } from 'react';
import s from './ui.module.css';

export interface ModalProps {
  open: boolean;
  onClose: () => void;
  title: string;
  /** `lg` pour ce qui doit tenir une signature de 620 px : aperçu, export, publication. */
  size?: 'md' | 'lg';
  children: ReactNode;
}

/**
 * <dialog> natif : piège de focus, fermeture Échap et restitution du focus à la fermeture
 * sont fournis par le navigateur. Une réimplémentation en JS serait plus longue et moins juste.
 *
 * Seule modale de l'application : l'éditeur avait sa propre copie (editor/Dialog.tsx),
 * supprimée au profit de `size="lg"`.
 */
export function Modal({ open, onClose, title, size = 'md', children }: ModalProps) {
  const ref = useRef<HTMLDialogElement>(null);
  const titleId = useId();

  useEffect(() => {
    const dialog = ref.current;
    if (!dialog) return;
    if (open && !dialog.open) dialog.showModal();
    else if (!open && dialog.open) dialog.close();
  }, [open]);

  // Clic sur le fond : la cible est le <dialog> lui-même, le contenu étant dans un enfant.
  const onBackdropClick = (e: MouseEvent<HTMLDialogElement>) => {
    if (e.target === ref.current) onClose();
  };

  return (
    <dialog
      ref={ref}
      className={[s.modal, size === 'lg' ? s.modalLg : null].filter(Boolean).join(' ')}
      aria-labelledby={titleId}
      onClose={onClose}
      onClick={onBackdropClick}
    >
      {open && (
        <div className={s.modalBody}>
          <div className={s.modalHead}>
            <h2 className={s.modalTitle} id={titleId}>
              {title}
            </h2>
            <button type="button" className={s.modalClose} onClick={onClose} aria-label="Fermer">
              ✕
            </button>
          </div>
          <div className={s.modalContent}>{children}</div>
        </div>
      )}
    </dialog>
  );
}
