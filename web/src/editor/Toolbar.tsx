/** Barre supérieure : identité, historique, vue, état d'enregistrement, actions de sortie. */
import { Link } from 'react-router-dom';
import { Button } from '../components/Button';
import { Spinner } from '../components/Spinner';
import type { Autosave } from './useAutosave';
import s from './editor.module.css';

interface ToolbarProps {
  name: string;
  onNameChange: (name: string) => void;
  save: Autosave;
  canUndo: boolean;
  canRedo: boolean;
  onUndo: () => void;
  onRedo: () => void;
  view: 'desktop' | 'mobile';
  onViewChange: (view: 'desktop' | 'mobile') => void;
  grid: boolean;
  onGridChange: (grid: boolean) => void;
  analyticsHref: string | null;
  onPreview: () => void;
  onExport: () => void;
  onPublish: () => void;
}

export function Toolbar({
  name,
  onNameChange,
  save,
  canUndo,
  canRedo,
  onUndo,
  onRedo,
  view,
  onViewChange,
  grid,
  onGridChange,
  analyticsHref,
  onPreview,
  onExport,
  onPublish,
}: ToolbarProps) {
  return (
    <header className={s.topbar}>
      <div className={s.topGroup}>
        <Link to="/app" className={s.iconBtn} aria-label="Revenir à mes signatures" title="Mes signatures">
          ←
        </Link>
        <label className={s.srOnly} htmlFor="signature-name">
          Nom de la signature
        </label>
        <input
          id="signature-name"
          className={s.nameInput}
          value={name}
          onChange={(event) => onNameChange(event.target.value)}
        />
        <span className={s.saveState} aria-live="polite">
          {save.status === 'saving' && (
            <>
              <Spinner size={13} /> Enregistrement…
            </>
          )}
          {save.status === 'saved' && 'Enregistré'}
          {save.status === 'error' && (
            <>
              <span className={s.saveError}>{save.error ?? 'Enregistrement impossible.'}</span>
              <button type="button" className={s.tinyBtn} onClick={save.retry}>
                Réessayer
              </button>
            </>
          )}
        </span>
      </div>

      <div className={s.topGroup}>
        <button
          type="button"
          className={s.iconBtn}
          onClick={onUndo}
          disabled={!canUndo}
          aria-label="Annuler"
          title="Annuler (Ctrl+Z)"
        >
          ↶
        </button>
        <button
          type="button"
          className={s.iconBtn}
          onClick={onRedo}
          disabled={!canRedo}
          aria-label="Rétablir"
          title="Rétablir (Ctrl+Maj+Z)"
        >
          ↷
        </button>

        <div className={s.segmented} role="group" aria-label="Aperçu de la largeur">
          <button type="button" aria-pressed={view === 'desktop'} onClick={() => onViewChange('desktop')}>
            Ordinateur
          </button>
          <button type="button" aria-pressed={view === 'mobile'} onClick={() => onViewChange('mobile')}>
            Mobile
          </button>
        </div>

        <button
          type="button"
          className={s.tinyBtn}
          aria-pressed={grid}
          onClick={() => onGridChange(!grid)}
        >
          Grille
        </button>

        {analyticsHref && (
          <Link to={analyticsHref} className={s.tinyBtn}>
            Statistiques
          </Link>
        )}
      </div>

      <div className={s.topGroup}>
        <Button variant="ghost" onClick={onPreview}>
          Aperçu
        </Button>
        <Button variant="ghost" onClick={onExport}>
          Exporter
        </Button>
        <Button onClick={onPublish}>Publier</Button>
      </div>
    </header>
  );
}
