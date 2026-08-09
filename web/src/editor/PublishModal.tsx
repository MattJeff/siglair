/**
 * Publication : c'est le produit (contrat §0). On publie, on sonde /status jusqu'au rendu,
 * puis on donne l'URL hébergée et le code à coller — une fois, définitivement.
 */
import { useEffect, useRef, useState } from 'react';
import { Link } from 'react-router-dom';
import { Button } from '../components/Button';
import { Spinner } from '../components/Spinner';
import { useToast } from '../components/Toast';
import { ApiError, exportSignature, getSignatureStatus, publishSignature } from '../lib/api';
import { Modal } from '../components/Modal';
import s from './editor.module.css';

const POLL_INTERVAL = 1500;
const POLL_LIMIT = 80; // ≈ 2 minutes, au-delà on rend la main plutôt que de tourner en boucle.

const sleep = (ms: number): Promise<void> => new Promise((resolve) => window.setTimeout(resolve, ms));

interface PublishModalProps {
  open: boolean;
  onClose: () => void;
  signatureId: string;
  publicSlug: string | null;
  /** limits.hosted_gif */
  hostedAllowed: boolean;
  onPublished: (slug: string) => void;
}

export function PublishModal({
  open,
  onClose,
  signatureId,
  publicSlug,
  hostedAllowed,
  onPublished,
}: PublishModalProps) {
  const toast = useToast();
  const [phase, setPhase] = useState<'idle' | 'working' | 'done' | 'error'>(publicSlug ? 'done' : 'idle');
  const [slug, setSlug] = useState(publicSlug);
  const [embed, setEmbed] = useState('');
  const [error, setError] = useState<string | null>(null);
  const closed = useRef(false);

  const gifUrl = slug ? `${window.location.origin}/s/${slug}.gif` : '';

  const loadEmbed = async () => {
    try {
      const result = await exportSignature(signatureId, 'hosted');
      setEmbed(result.html);
    } catch {
      // Le code à coller est un confort : l'URL seule suffit à utiliser la signature.
      setEmbed('');
    }
  };

  const embedRef = useRef(loadEmbed);
  embedRef.current = loadEmbed;

  useEffect(() => {
    closed.current = !open;
    // Signature déjà publiée : on récupère le code à coller à l'ouverture.
    if (open && hostedAllowed && publicSlug) void embedRef.current();
  }, [open, hostedAllowed, publicSlug]);

  async function publish() {
    setPhase('working');
    setError(null);
    try {
      const started = await publishSignature(signatureId);
      setSlug(started.slug);

      for (let attempt = 0; attempt < POLL_LIMIT; attempt += 1) {
        if (closed.current) return;
        const status = await getSignatureStatus(signatureId);

        if (status.job?.status === 'failed') {
          setPhase('error');
          setError(status.job.error ?? 'Le rendu a échoué. Réessayez dans un instant.');
          return;
        }
        if (status.job?.status === 'done' || (status.job === null && status.render !== null)) {
          setPhase('done');
          onPublished(started.slug);
          await loadEmbed();
          toast('Signature publiée', 'success');
          return;
        }
        await sleep(POLL_INTERVAL);
      }

      setPhase('error');
      setError('Le rendu prend plus de temps que d’habitude. Rouvrez cette fenêtre dans une minute.');
    } catch (cause) {
      setPhase('error');
      setError(cause instanceof ApiError ? cause.message : 'La publication a échoué.');
    }
  }

  async function copy(value: string, label: string) {
    try {
      await navigator.clipboard.writeText(value);
      toast(`${label} copié`, 'success');
    } catch {
      toast('Copie impossible : sélectionnez le texte et copiez-le à la main.', 'error');
    }
  }

  return (
    <Modal open={open} onClose={onClose} title="Publier la signature" size="lg">
      {!hostedAllowed ? (
        <div className={s.stack}>
          <div className={`${s.note} ${s.noteWarn}`}>
            La signature hébergée fait partie du plan Pro. C’est l’adresse que vous collez une
            seule fois dans votre client mail : ensuite, vous modifiez votre signature ici et tous
            les emails que vous n’avez pas encore envoyés affichent la nouvelle version.
          </div>
          <p className={s.note}>
            En attendant, l’export libre ou compatible reste disponible et fonctionne partout.
          </p>
          <Link to="/app/billing">
            <Button>Voir les plans</Button>
          </Link>
        </div>
      ) : (
        <div className={s.stack}>
          {phase === 'working' && (
            <div className={s.note}>
              <Spinner size={16} label="Rendu en cours" /> Rendu du GIF en cours. Quelques secondes
              suffisent en général.
            </div>
          )}

          {phase === 'error' && error && (
            <div className={`${s.note} ${s.noteWarn}`}>{error}</div>
          )}

          {phase === 'done' && slug && (
            <>
              <div className={`${s.note} ${s.noteGood}`}>
                Signature en ligne. Collez cette adresse une fois : les prochaines publications la
                mettent à jour toute seule.
              </div>
              <div className={s.hostedUrl}>
                <code>{gifUrl}</code>
                <Button variant="ghost" onClick={() => void copy(gifUrl, 'Lien')}>
                  Copier
                </Button>
              </div>
              {embed && (
                <>
                  <label className={s.srOnly} htmlFor="embed-code">
                    Code à coller dans votre client mail
                  </label>
                  <textarea id="embed-code" className={s.exportCode} readOnly value={embed} style={{ height: 160 }} />
                  <Button variant="ghost" onClick={() => void copy(embed, 'Code')}>
                    Copier le code
                  </Button>
                </>
              )}
            </>
          )}

          <Button loading={phase === 'working'} onClick={() => void publish()}>
            {phase === 'done' ? 'Republier' : 'Publier'}
          </Button>

          <p className={s.note}>
            Publier enregistre la version actuelle et refabrique le GIF. Tant que vous ne publiez
            pas, vos modifications restent visibles ici seulement.
          </p>
        </div>
      )}
    </Modal>
  );
}
