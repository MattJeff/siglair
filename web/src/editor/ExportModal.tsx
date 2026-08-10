/**
 * Export : les trois modes du contrat §5.3. Le HTML vient de l'API, il n'est pas fabriqué ici.
 * Le rapport de compatibilité, lui, est un conseil calculé sur le document — pas un rendu.
 */
import { useCallback, useEffect, useRef, useState } from 'react';
import { Link } from 'react-router-dom';
import { Button } from '../components/Button';
import { Spinner } from '../components/Spinner';
import { useToast } from '../components/Toast';
import { ApiError, exportSignature } from '../lib/api';
import { getAnalytics } from '../lib/analytics';
import type { Doc, ExportMode } from '../lib/types';
import { Modal } from '../components/Modal';
import { compatibility } from './state';
import s from './editor.module.css';

const MODES: { mode: ExportMode; title: string; hint: string }[] = [
  { mode: 'hosted', title: 'GIF hébergé', hint: 'Une image, mise à jour sans recoller' },
  { mode: 'freeform', title: 'Libre', hint: 'Positionnement exact, HTML complet' },
  { mode: 'safe', title: 'Compatible', hint: 'Tableau simple, aucune animation' },
];

interface ExportModalProps {
  open: boolean;
  onClose: () => void;
  signatureId: string;
  doc: Doc;
  /** limits.hosted_gif — le mode hébergé est le verrou Free → Pro (contrat §6). */
  hostedAllowed: boolean;
  /** limits.branding — la marque Siglair est imposée dans l'export du plan Free. */
  branding: boolean;
}

export function ExportModal({ open, onClose, signatureId, doc, hostedAllowed, branding }: ExportModalProps) {
  const toast = useToast();
  const [mode, setMode] = useState<ExportMode>(hostedAllowed ? 'hosted' : 'freeform');
  const [html, setHtml] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const report = compatibility(doc);

  const load = useCallback(
    async (next: ExportMode) => {
      setLoading(true);
      setError(null);
      getAnalytics().track('export_started', {
        signature_id: signatureId,
        export_format: next,
      });
      try {
        const result = await exportSignature(signatureId, next);
        setHtml(result.html);
        getAnalytics().track('export_completed', {
          signature_id: signatureId,
          export_format: next,
        });
      } catch (cause) {
        setHtml('');
        setError(cause instanceof ApiError ? cause.message : "L'export n'a pas pu être généré.");
      } finally {
        setLoading(false);
      }
    },
    [signatureId],
  );

  const loadRef = useRef(load);
  loadRef.current = load;
  useEffect(() => {
    if (open) {
      getAnalytics().track('install_instructions_viewed', {
        signature_id: signatureId,
        client: 'unknown',
      });
      void loadRef.current(mode);
    }
  }, [open, mode, signatureId]);

  async function copy() {
    getAnalytics().track('install_started', {
      signature_id: signatureId,
      client: 'unknown',
      method: 'copy_html',
    });
    try {
      await navigator.clipboard.writeText(html);
      getAnalytics().track('install_completed', {
        signature_id: signatureId,
        client: 'unknown',
        method: 'copy_html',
      });
      toast('HTML copié dans le presse-papier', 'success');
    } catch {
      toast('Copie impossible : sélectionnez le code et copiez-le à la main.', 'error');
    }
  }

  function download() {
    getAnalytics().track('install_started', {
      signature_id: signatureId,
      client: 'unknown',
      method: 'manual',
    });
    const url = URL.createObjectURL(new Blob([html], { type: 'text/html' }));
    const link = document.createElement('a');
    link.href = url;
    link.download = 'signature.html';
    link.click();
    getAnalytics().track('install_completed', {
      signature_id: signatureId,
      client: 'unknown',
      method: 'manual',
    });
    URL.revokeObjectURL(url);
  }

  return (
    <Modal open={open} onClose={onClose} title="Exporter la signature" size="lg">
      <div className={s.exportGrid}>
        <div>
          <div className={s.modes}>
            {MODES.map((item) => {
              const locked = item.mode === 'hosted' && !hostedAllowed;
              return (
                <button
                  key={item.mode}
                  type="button"
                  className={[s.mode, mode === item.mode ? s.modeActive : null].filter(Boolean).join(' ')}
                  aria-pressed={mode === item.mode}
                  disabled={locked}
                  onClick={() => setMode(item.mode)}
                >
                  <b>{item.title}</b>
                  <small>{locked ? 'Inclus dans le plan Pro' : item.hint}</small>
                </button>
              );
            })}
          </div>

          <label className={s.srOnly} htmlFor="export-code">
            Code HTML de la signature
          </label>
          <textarea id="export-code" className={s.exportCode} readOnly value={html} />

          <div className={`${s.toolbarRow} ${s.gap}`}>
            <Button onClick={() => void copy()} disabled={!html}>
              Copier le HTML
            </Button>
            <Button variant="ghost" onClick={download} disabled={!html}>
              Télécharger .html
            </Button>
            {loading && <Spinner size={16} label="Génération de l’export" />}
          </div>
        </div>

        <div className={s.stack}>
          <div className={s.sectionHead}>
            <h3>Compatibilité</h3>
            <span>{report.score}/100</span>
          </div>

          {error && (
            <div className={`${s.note} ${s.noteWarn}`}>
              {error}
              <div className={s.gap}>
                <Button variant="ghost" onClick={() => void load(mode)}>
                  Réessayer
                </Button>
              </div>
            </div>
          )}

          {!hostedAllowed && (
            <div className={`${s.note} ${s.noteWarn}`}>
              Le GIF hébergé — l’URL que vous collez une fois pour toutes — fait partie du plan Pro.{' '}
              <Link to="/app/billing">Voir les plans</Link>
            </div>
          )}

          {branding && (
            <div className={s.note}>
              L’export du plan gratuit porte la mention Siglair. Elle disparaît avec le plan Pro.
            </div>
          )}

          {report.warnings.map((warning) => (
            <div key={warning} className={`${s.note} ${s.noteWarn}`}>
              {warning}
            </div>
          ))}

          <div className={`${s.note} ${s.noteGood}`}>
            Ce HTML est produit par le serveur, exactement comme celui utilisé pour fabriquer le
            GIF : ce que vous copiez est ce qui sera envoyé.
          </div>
        </div>
      </div>
    </Modal>
  );
}
