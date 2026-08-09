import type { InputHTMLAttributes } from 'react';
import s from './ui.module.css';

export type InputProps = InputHTMLAttributes<HTMLInputElement>;

/** Sans <label> associé (voir Field), un champ est inutilisable au lecteur d'écran. */
export function Input({ className, ...rest }: InputProps) {
  return <input className={[s.control, className].filter(Boolean).join(' ')} {...rest} />;
}
