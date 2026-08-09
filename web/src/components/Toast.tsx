import { createContext, useCallback, useContext, useState } from 'react';
import type { ReactNode } from 'react';
import s from './ui.module.css';

export type ToastKind = 'info' | 'success' | 'error';

interface Item {
  id: number;
  message: string;
  kind: ToastKind;
}

type Push = (message: string, kind?: ToastKind) => void;

const ToastContext = createContext<Push | null>(null);
let seq = 0;

export function ToastProvider({ children }: { children: ReactNode }) {
  const [items, setItems] = useState<Item[]>([]);

  const dismiss = useCallback((id: number) => {
    setItems((list) => list.filter((t) => t.id !== id));
  }, []);

  const push = useCallback<Push>(
    (message, kind = 'info') => {
      const id = ++seq;
      setItems((list) => [...list, { id, message, kind }]);
      window.setTimeout(() => dismiss(id), 5000);
    },
    [dismiss],
  );

  return (
    <ToastContext.Provider value={push}>
      {children}
      {/* ponytail : une seule région polie. Les erreurs bloquantes s'affichent
          dans le formulaire (Field), pas seulement ici. */}
      <div className={s.toasts} role="status" aria-live="polite">
        {items.map((t) => (
          <div
            key={t.id}
            className={[
              s.toast,
              t.kind === 'success' ? s.toastSuccess : null,
              t.kind === 'error' ? s.toastError : null,
            ]
              .filter(Boolean)
              .join(' ')}
          >
            <span className={s.toastText}>{t.message}</span>
            <button
              type="button"
              className={s.toastClose}
              onClick={() => dismiss(t.id)}
              aria-label="Masquer la notification"
            >
              ✕
            </button>
          </div>
        ))}
      </div>
    </ToastContext.Provider>
  );
}

/** `const toast = useToast(); toast('Signature publiée', 'success')` */
export function useToast(): Push {
  const push = useContext(ToastContext);
  if (!push) throw new Error('useToast doit être utilisé dans <ToastProvider>');
  return push;
}
