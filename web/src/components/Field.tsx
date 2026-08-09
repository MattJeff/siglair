import { cloneElement, useId } from 'react';
import type { ReactElement } from 'react';
import s from './ui.module.css';

/** Les props que Field pose sur son enfant : un Input, un Select, ou tout contrôle natif. */
interface Labelable {
  id?: string;
  'aria-invalid'?: boolean;
  'aria-describedby'?: string;
}

export interface FieldProps {
  label: string;
  hint?: string;
  error?: string;
  /** Un seul contrôle. Field lui pose l'id, l'état d'erreur et la description. */
  children: ReactElement<Labelable>;
}

export function Field({ label, hint, error, children }: FieldProps) {
  const id = useId();
  const hintId = `${id}-hint`;
  const errorId = `${id}-error`;
  const describedBy = [hint ? hintId : null, error ? errorId : null].filter(Boolean).join(' ');

  return (
    <div className={s.field}>
      <label className={s.label} htmlFor={id}>
        {label}
      </label>
      {hint && (
        <p className={s.hint} id={hintId}>
          {hint}
        </p>
      )}
      {cloneElement(children, {
        id,
        'aria-invalid': error ? true : undefined,
        'aria-describedby': describedBy || undefined,
      })}
      {error && (
        <p className={s.errorText} id={errorId} role="alert">
          {error}
        </p>
      )}
    </div>
  );
}
