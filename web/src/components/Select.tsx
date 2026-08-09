import type { SelectHTMLAttributes } from 'react';
import s from './ui.module.css';

export type SelectProps = SelectHTMLAttributes<HTMLSelectElement>;

export function Select({ className, children, ...rest }: SelectProps) {
  return (
    <select className={[s.control, className].filter(Boolean).join(' ')} {...rest}>
      {children}
    </select>
  );
}
