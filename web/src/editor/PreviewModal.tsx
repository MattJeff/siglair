/**
 * Aperçu multi-clients.
 *
 * Le HTML n'est PAS reconstruit ici (contrat §2) : il vient de POST /api/preview et s'affiche
 * dans une <iframe srcDoc sandbox>. Le seul ajout est la simulation « Outlook ancien », une
 * feuille de style qui fige les animations : c'est une limite du client mail qu'on montre,
 * pas une seconde implémentation du rendu.
 */
import { useCallback, useEffect, useRef, useState } from 'react';
import { Button } from '../components/Button';
import { Spinner } from '../components/Spinner';
import { ApiError, previewDoc } from '../lib/api';
import type { Doc, Profile, User } from '../lib/types';
import { Modal } from '../components/Modal';
import s from './editor.module.css';

const CLIENTS = [
  { id: 'gmail', name: 'Gmail', hint: 'GIF animé pris en charge.' },
  { id: 'apple', name: 'Apple Mail', hint: 'GIF animé pris en charge.' },
  { id: 'outlook-web', name: 'Outlook web', hint: 'GIF animé pris en charge.' },
  {
    id: 'outlook-legacy',
    name: 'Outlook Windows (ancien)',
    hint: "Simulation : animations figées. Ce client n'affiche que la première image du GIF.",
  },
] as const;

type ClientId = (typeof CLIENTS)[number]['id'];

/** Injectée dans l'iframe pour simuler le figeage d'Outlook, sans toucher au HTML du serveur. */
const FREEZE = '<style>*{animation:none !important;transition:none !important}</style>';

interface PreviewModalProps {
  open: boolean;
  onClose: () => void;
  doc: Doc;
  profile: Profile;
  user: User | null;
}

export function PreviewModal({ open, onClose, doc, profile, user }: PreviewModalProps) {
  const [client, setClient] = useState<ClientId>('gmail');
  const [html, setHtml] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  // Change à chaque « Rejouer » : remonte l'iframe, donc relance les animations.
  const [reloadKey, setReloadKey] = useState(0);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await previewDoc(doc, profile);
      setHtml(result.html);
    } catch (cause) {
      setError(cause instanceof ApiError ? cause.message : "L'aperçu n'a pas pu être chargé.");
    } finally {
      setLoading(false);
    }
  }, [doc, profile]);

  // Chargé à l'ouverture seulement : rappeler /api/preview à chaque frappe serait
  // un aller-retour réseau par caractère. Le bouton « Rejouer » rafraîchit à la demande.
  const loadRef = useRef(load);
  loadRef.current = load;
  useEffect(() => {
    if (open) void loadRef.current();
  }, [open]);

  const hint = CLIENTS.find((item) => item.id === client)?.hint ?? '';
  const senderName = user?.name ?? user?.email ?? 'Vous';

  return (
    <Modal open={open} onClose={onClose} title="Aperçu dans les clients mail" size="lg">
      <div className={s.toolbarRow}>
        <label className={s.inlineLabel}>
          Client
          <select
            className={s.inlineSelect}
            value={client}
            onChange={(event) => setClient(event.target.value as ClientId)}
          >
            {CLIENTS.map((item) => (
              <option key={item.id} value={item.id}>
                {item.name}
              </option>
            ))}
          </select>
        </label>
        <Button
          variant="ghost"
          onClick={() => {
            setReloadKey((key) => key + 1);
            void load();
          }}
        >
          ↻ Rejouer
        </Button>
        {loading && <Spinner size={16} label="Chargement de l’aperçu" />}
        <span className={s.inlineLabel}>{hint}</span>
      </div>

      {error ? (
        <div className={`${s.note} ${s.noteWarn}`}>
          {error}
          <div className={s.gap}>
            <Button variant="ghost" onClick={() => void load()}>
              Réessayer
            </Button>
          </div>
        </div>
      ) : (
        <div className={s.mailShell}>
          <div className={s.mail}>
            <div className={s.mailHead}>
              <span className={s.avatar} aria-hidden="true">
                {senderName.slice(0, 1).toUpperCase()}
              </span>
              <div>
                <strong>{senderName}</strong>
                <small>à un client · maintenant</small>
              </div>
            </div>
            <p>Bonjour,</p>
            <p>Voici le rendu de votre signature dans le client sélectionné.</p>
            <p>Bien à vous,</p>
            <div className={s.previewScroll}>
              <iframe
                key={`${client}:${reloadKey}`}
                className={s.previewFrame}
                title="Aperçu de la signature"
                sandbox=""
                width={doc.canvas.width}
                height={doc.canvas.height}
                style={{ width: doc.canvas.width, height: doc.canvas.height }}
                srcDoc={client === 'outlook-legacy' ? html + FREEZE : html}
              />
            </div>
          </div>
        </div>
      )}

      <p className={`${s.note} ${s.gap}`}>
        Dans un vrai Outlook ancien, c’est la première image du GIF qui s’affiche : soignez ce qui
        est visible à l’instant zéro.
      </p>
    </Modal>
  );
}
