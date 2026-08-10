/**
 * Publication : lance ou reprend un rendu, puis garde le même écran jusqu'à l'URL en ligne.
 * Fermer la modale n'arrête jamais le worker ; la rouvrir se resynchronise sur le dernier job.
 */
import { useEffect, useRef, useState } from 'react';
import type { CSSProperties } from 'react';
import { Link } from 'react-router-dom';
import {
  Check,
  CheckCircle2,
  Clock3,
  Code2,
  Copy,
  ExternalLink,
  RefreshCw,
  Send,
  Sparkles,
  TriangleAlert,
} from 'lucide-react';
import { Button } from '../components/Button';
import { Modal } from '../components/Modal';
import { Spinner } from '../components/Spinner';
import { useToast } from '../components/Toast';
import { ApiError, exportSignature, getSignatureStatus, publishSignature } from '../lib/api';
import { getAnalytics } from '../lib/analytics';
import { publicationPhase } from './publication';
import type { PublicationPhase } from './publication';
import s from './editor.module.css';

const POLL_INTERVAL = 1600;

const CONFETTI = Array.from({ length: 26 }, (_, index) => ({
  color: ['#315cff', '#39d4ff', '#43e6a6', '#fbbf24', '#f472b6'][index % 5],
  delay: (index % 7) * 0.035,
  rotate: 180 + ((index * 47) % 360),
  tx: -210 + ((index * 83) % 420),
  ty: 120 + ((index * 37) % 220),
}));

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

const activePhase = (phase: PublicationPhase): boolean =>
  phase === 'checking' || phase === 'queued' || phase === 'rendering';

export function PublishModal({
  open,
  onClose,
  signatureId,
  publicSlug,
  hostedAllowed,
  onPublished,
}: PublishModalProps) {
  const toast = useToast();
  const [phase, setPhase] = useState<PublicationPhase>('idle');
  const [slug, setSlug] = useState(publicSlug);
  const [embed, setEmbed] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [trackingIssue, setTrackingIssue] = useState<string | null>(null);
  const [attempts, setAttempts] = useState(0);
  const [startedAt, setStartedAt] = useState<number | null>(null);
  const [elapsed, setElapsed] = useState(0);
  const [celebrating, setCelebrating] = useState(false);
  const runId = useRef(0);
  const publicSlugRef = useRef(publicSlug);
  const monitorRef = useRef<(knownSlug: string | null, token: number, celebrate: boolean) => Promise<void>>(
    async () => undefined,
  );
  publicSlugRef.current = publicSlug;

  const gifUrl = slug ? `${window.location.origin}/s/${slug}.gif` : '';

  async function loadEmbed() {
    try {
      const result = await exportSignature(signatureId, 'hosted');
      setEmbed(result.html);
    } catch {
      setEmbed('');
    }
  }

  function celebrate() {
    setCelebrating(true);
    window.setTimeout(() => setCelebrating(false), 2300);
  }

  async function complete(nextSlug: string | null, withCelebration: boolean) {
    if (!nextSlug) {
      setPhase('error');
      setError('Le rendu est prêt, mais son adresse publique est introuvable. Réessayez.');
      return;
    }
    setSlug(nextSlug);
    setPhase('done');
    setError(null);
    setTrackingIssue(null);
    onPublished(nextSlug);
    void loadEmbed();
    if (withCelebration) {
      getAnalytics().track('publish_completed', {
        signature_id: signatureId,
        ...(startedAt ? { duration_ms: Math.max(0, Date.now() - startedAt) } : {}),
      });
      getAnalytics().track('signature_published', {
        signature_id: signatureId,
        zero_edit: false,
      });
      celebrate();
      toast('Signature publiée', 'success');
    }
  }

  async function monitor(knownSlug: string | null, token: number, withCelebration: boolean) {
    let celebrateWhenDone = withCelebration;
    while (runId.current === token) {
      try {
        const status = await getSignatureStatus(signatureId);
        if (runId.current !== token) return;
        setTrackingIssue(null);

        const nextSlug = status.slug ?? knownSlug;
        if (nextSlug) {
          knownSlug = nextSlug;
          setSlug(nextSlug);
        }

        const nextPhase = publicationPhase(status);
        if (nextPhase === 'error') {
          getAnalytics().track('publish_failed', {
            signature_id: signatureId,
            failure_code: 'render_failed',
          });
          setPhase('error');
          setAttempts(status.job?.attempts ?? 0);
          setError(status.job?.error ?? 'Le rendu a échoué. Réessayez dans un instant.');
          return;
        }
        if (nextPhase === 'done') {
          await complete(knownSlug, celebrateWhenDone);
          return;
        }
        if (nextPhase === 'queued' || nextPhase === 'rendering') {
          celebrateWhenDone = true;
          setPhase(nextPhase);
          setAttempts(status.job?.attempts ?? 0);
          const created = Date.parse(status.job?.created_at ?? '');
          if (Number.isFinite(created)) setStartedAt(created);
          await sleep(POLL_INTERVAL);
          continue;
        }

        setPhase('idle');
        return;
      } catch {
        if (runId.current !== token) return;
        setTrackingIssue('Connexion au suivi interrompue. Nouvelle tentative automatique…');
        await sleep(2500);
      }
    }
  }
  monitorRef.current = monitor;

  useEffect(() => {
    if (!open || !hostedAllowed) {
      runId.current += 1;
      return;
    }

    const token = ++runId.current;
    setPhase('checking');
    setSlug(publicSlugRef.current);
    setEmbed('');
    setError(null);
    setTrackingIssue(null);
    setCelebrating(false);
    void monitorRef.current(publicSlugRef.current, token, false);

    return () => {
      runId.current += 1;
    };
  }, [open, hostedAllowed, signatureId]);

  useEffect(() => {
    if (!activePhase(phase) || startedAt === null) {
      if (!activePhase(phase)) setElapsed(0);
      return;
    }
    const tick = () => setElapsed(Math.max(0, Math.round((Date.now() - startedAt) / 1000)));
    tick();
    const timer = window.setInterval(tick, 1000);
    return () => window.clearInterval(timer);
  }, [phase, startedAt]);

  async function publish() {
    const token = ++runId.current;
    setPhase('queued');
    setStartedAt(Date.now());
    setElapsed(0);
    setAttempts(0);
    setError(null);
    setTrackingIssue(null);
    setEmbed('');
    getAnalytics().track('publish_started', { signature_id: signatureId });
    try {
      const started = await publishSignature(signatureId);
      if (runId.current !== token) return;
      setSlug(started.slug);
      if (started.render_id) {
        await complete(started.slug, true);
        return;
      }
      await monitorRef.current(started.slug, token, true);
    } catch (cause) {
      if (runId.current !== token) return;
      setPhase('error');
      getAnalytics().track('publish_failed', {
        signature_id: signatureId,
        failure_code: cause instanceof ApiError ? cause.code : 'unknown',
      });
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

  const title = phase === 'done' ? 'Signature publiée' : activePhase(phase) ? 'Publication en cours' : 'Publier la signature';
  const progress = phase === 'checking' ? 8 : phase === 'queued' ? 32 : phase === 'rendering' ? 68 : phase === 'done' ? 100 : 0;

  return (
    <Modal open={open} onClose={onClose} title={title}>
      {!hostedAllowed ? (
        <div className={s.publishStack}>
          <div className={`${s.publishHero} ${s.publishHeroWarn}`}>
            <span className={s.publishIcon} aria-hidden="true">
              <TriangleAlert size={25} />
            </span>
            <div>
              <h3>Publication disponible avec Pro</h3>
              <p>L’URL hébergée reste la même à chaque mise à jour.</p>
            </div>
          </div>
          <Link to="/app/billing">
            <Button>Voir les plans</Button>
          </Link>
        </div>
      ) : (
        <div className={s.publishStack}>
          {celebrating && (
            <div className={s.confetti} aria-hidden="true">
              {CONFETTI.map((piece, index) => (
                <i
                  key={index}
                  className={s.confettiPiece}
                  style={
                    {
                      '--confetti-delay': `${piece.delay}s`,
                      '--confetti-rotate': `${piece.rotate}deg`,
                      '--confetti-x': `${piece.tx}px`,
                      '--confetti-y': `${piece.ty}px`,
                      backgroundColor: piece.color,
                    } as CSSProperties
                  }
                />
              ))}
            </div>
          )}

          {phase === 'idle' && (
            <>
              <div className={s.publishHero}>
                <span className={s.publishIcon} aria-hidden="true">
                  <Send size={25} />
                </span>
                <div>
                  <h3>Mettre cette version en ligne</h3>
                  <p>Le GIF sera optimisé puis remplacera la version publique à la même adresse.</p>
                </div>
              </div>
              <div className={s.publishActions}>
                <Button onClick={() => void publish()}>
                  <Sparkles size={17} /> Publier maintenant
                </Button>
              </div>
            </>
          )}

          {activePhase(phase) && (
            <div className={s.publishWorking} aria-live="polite">
              <div className={s.publishHero}>
                <span className={`${s.publishIcon} ${s.publishIconWorking}`} aria-hidden="true">
                  <Spinner size={23} />
                </span>
                <div>
                  <h3>
                    {phase === 'checking'
                      ? 'Vérification du rendu'
                      : phase === 'queued'
                        ? attempts > 0
                          ? 'Nouvelle tentative automatique'
                          : 'Version enregistrée'
                        : 'Création du GIF animé'}
                  </h3>
                  <p>
                    {phase === 'checking'
                      ? 'On retrouve la publication en cours.'
                      : phase === 'queued'
                        ? 'Le moteur de rendu va prendre le relais.'
                        : 'Capture des animations et optimisation pour les clients mail.'}
                  </p>
                </div>
                {elapsed > 0 && (
                  <span className={s.publishElapsed}>
                    <Clock3 size={14} /> {elapsed} s
                  </span>
                )}
              </div>

              <div
                className={s.publishProgress}
                role="progressbar"
                aria-label="Progression de la publication"
                aria-valuemin={0}
                aria-valuemax={100}
                aria-valuenow={progress}
              >
                <span style={{ width: `${progress}%` }} />
              </div>
              <ol className={s.publishSteps}>
                <li className={phase !== 'checking' ? s.publishStepDone : undefined}>
                  <Check size={14} /> Enregistrée
                </li>
                <li className={phase === 'rendering' ? s.publishStepActive : undefined}>
                  <Spinner size={13} /> GIF optimisé
                </li>
                <li>
                  <CheckCircle2 size={14} /> En ligne
                </li>
              </ol>
              {trackingIssue && <p className={s.publishTracking}>{trackingIssue}</p>}
              <p className={s.publishQuiet}>Vous pouvez fermer cette fenêtre : le rendu continue en arrière-plan.</p>
            </div>
          )}

          {phase === 'error' && error && (
            <>
              <div className={`${s.publishHero} ${s.publishHeroError}`} role="alert">
                <span className={s.publishIcon} aria-hidden="true">
                  <TriangleAlert size={25} />
                </span>
                <div>
                  <h3>Le GIF n’a pas été publié</h3>
                  <p>{error}</p>
                </div>
              </div>
              <div className={s.publishActions}>
                <Button onClick={() => void publish()}>
                  <RefreshCw size={17} /> Réessayer
                </Button>
                <Button variant="ghost" onClick={onClose}>Fermer</Button>
              </div>
            </>
          )}

          {phase === 'done' && slug && (
            <>
              <div className={`${s.publishHero} ${s.publishHeroDone}`} aria-live="polite">
                <span className={`${s.publishIcon} ${s.publishSuccessIcon}`} aria-hidden="true">
                  <CheckCircle2 size={28} />
                </span>
                <div>
                  <h3>C’est en ligne</h3>
                  <p>La nouvelle version est servie à tous les prochains emails.</p>
                </div>
              </div>

              <div className={s.publishUrl}>
                <div>
                  <span>Adresse publique</span>
                  <code>{gifUrl}</code>
                </div>
                <Button variant="ghost" onClick={() => void copy(gifUrl, 'Lien')}>
                  <Copy size={16} /> Copier
                </Button>
              </div>

              <div className={s.publishActions}>
                <Button disabled={!embed} onClick={() => void copy(embed, 'Code')}>
                  <Code2 size={17} /> Copier le code email
                </Button>
                <a className={s.publishOpen} href={gifUrl} target="_blank" rel="noreferrer">
                  <ExternalLink size={16} /> Voir le GIF
                </a>
                <Button variant="ghost" onClick={() => void publish()}>
                  <RefreshCw size={16} /> Republier
                </Button>
              </div>

              {embed && (
                <details className={s.publishCodeDetails}>
                  <summary>Afficher le code d’intégration</summary>
                  <textarea className={s.exportCode} readOnly value={embed} />
                </details>
              )}
            </>
          )}
        </div>
      )}
    </Modal>
  );
}
