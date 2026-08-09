/**
 * Les 4 modèles. Appliquer remplace le document — l'action reste annulable (Ctrl+Z).
 * Fichier nommé TemplateGallery et non Templates : `templates.ts` existe déjà à côté et
 * les systèmes de fichiers insensibles à la casse (macOS, Windows) confondent les deux.
 */
import type { Dispatch } from 'react';
import { TEMPLATES, docFromTemplate } from './templates';
import type { Action } from './state';
import s from './editor.module.css';

export function TemplateGallery({ dispatch }: { dispatch: Dispatch<Action> }) {
  return (
    <div className={s.templateGrid}>
      {TEMPLATES.map((template) => (
        <button
          key={template.id}
          type="button"
          className={s.template}
          onClick={() => dispatch({ type: 'replaceDoc', doc: docFromTemplate(template) })}
        >
          <span className={s.templateThumb} style={{ background: template.thumb }} aria-hidden="true">
            <span className={s.thumbRing} />
            <span className={s.thumbLine1} />
            <span className={s.thumbLine2} />
            <span className={s.thumbCta} />
          </span>
          <span className={s.templateMeta}>
            <b>{template.name}</b>
            <small>{template.description}</small>
          </span>
        </button>
      ))}
    </div>
  );
}
