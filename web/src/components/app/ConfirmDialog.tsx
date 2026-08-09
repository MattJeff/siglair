import { useEffect, useState } from 'react';
import type { ReactNode } from 'react';
import { Button } from '../Button';
import { Field } from '../Field';
import { Input } from '../Input';
import { Modal } from '../Modal';
import s from './shell.module.css';

export interface ConfirmDialogProps {
  open: boolean;
  title: string;
  confirmLabel: string;
  danger?: boolean;
  loading?: boolean;
  /** Texte exact à saisir pour débloquer la confirmation (suppression de compte). */
  requireText?: string;
  requireLabel?: string;
  onConfirm: () => void;
  onClose: () => void;
  /** Récapitulatif de ce qui va se passer. Une action de masse mérite mieux qu'un « OK ». */
  children: ReactNode;
}

/** Confirmation partagée : suppression, retrait d'un membre, déploiement d'équipe. */
export function ConfirmDialog({
  open,
  title,
  confirmLabel,
  danger = false,
  loading = false,
  requireText,
  requireLabel,
  onConfirm,
  onClose,
  children,
}: ConfirmDialogProps) {
  const [typed, setTyped] = useState('');

  // La saisie ne doit pas survivre à une fermeture : sinon la seconde ouverture est déjà validée.
  useEffect(() => {
    if (!open) setTyped('');
  }, [open]);

  const blocked = requireText !== undefined && typed.trim() !== requireText;

  return (
    <Modal open={open} onClose={onClose} title={title}>
      <div className={s.confirmBody}>
        {children}
        {requireText !== undefined && (
          <Field label={requireLabel ?? `Saisissez « ${requireText} » pour confirmer`}>
            <Input
              value={typed}
              autoComplete="off"
              onChange={(e) => setTyped(e.currentTarget.value)}
            />
          </Field>
        )}
      </div>
      <div className={s.confirmActions}>
        <Button variant="ghost" onClick={onClose}>
          Annuler
        </Button>
        <Button
          variant={danger ? 'danger' : 'primary'}
          loading={loading}
          disabled={blocked}
          onClick={onConfirm}
        >
          {confirmLabel}
        </Button>
      </div>
    </Modal>
  );
}
