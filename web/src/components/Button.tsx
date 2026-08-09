import type { ButtonHTMLAttributes } from 'react';
import { Spinner } from './Spinner';
import s from './ui.module.css';

type Variant = 'primary' | 'ghost' | 'danger';

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  loading?: boolean;
}

export function Button({
  variant = 'primary',
  loading = false,
  // Défaut explicite : un <button> dans un <form> soumet sinon par accident.
  type = 'button',
  disabled,
  className,
  children,
  ...rest
}: ButtonProps) {
  return (
    <button
      type={type}
      className={[s.btn, s[variant], className].filter(Boolean).join(' ')}
      disabled={disabled === true || loading}
      aria-busy={loading || undefined}
      {...rest}
    >
      {loading && <Spinner size={14} />}
      {children}
    </button>
  );
}
