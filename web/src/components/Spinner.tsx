import s from './ui.module.css';

interface SpinnerProps {
  size?: number;
  /** Renseigné = annoncé aux lecteurs d'écran. Omis = purement décoratif. */
  label?: string;
}

export function Spinner({ size = 16, label }: SpinnerProps) {
  return (
    <span
      className={s.spinner}
      style={{ width: size, height: size }}
      data-motion="keep"
      role={label ? 'status' : undefined}
      aria-label={label}
      aria-hidden={label ? undefined : true}
    />
  );
}
