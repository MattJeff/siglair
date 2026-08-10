import { useEffect, useState } from 'react';
import type { FormEvent } from 'react';
import { Link, Navigate, useSearchParams } from 'react-router-dom';
import { Button } from '../components/Button';
import { Field } from '../components/Field';
import { Input } from '../components/Input';
import { Spinner } from '../components/Spinner';
import { PENDING_NEXT, apiMessage, safeNext } from '../components/app/helpers';
import { oauthStartUrl, requestMagicLink } from '../lib/api';
import { getAnalytics } from '../lib/analytics';
import { captureRef, withRef } from '../lib/referral';
import { readHandoff } from '../onboarding/api';
import { useSession } from '../lib/session';
import s from './app.module.css';

/** Délai avant de pouvoir redemander un lien. Le serveur limite à 5 demandes/heure (§5.2). */
const RESEND_SECONDS = 60;

export default function Login() {
  const { status, features } = useSession();
  const [params] = useSearchParams();
  const next = safeNext(params.get('next'));

  // Contrat §11.3 : arrivée directe sur /login?ref=… ou relais posé par la landing. Figé au
  // premier rendu pour que les liens OAuth le portent dès leur affichage. Aucun cookie.
  const [refCode] = useState(() => captureRef(params.get('ref')));

  const [email, setEmail] = useState('');
  const [sent, setSent] = useState(false);
  const [sending, setSending] = useState(false);
  const [error, setError] = useState('');
  const [left, setLeft] = useState(0);

  useEffect(() => {
    if (left <= 0) return;
    const t = window.setTimeout(() => setLeft((n) => n - 1), 1000);
    return () => window.clearTimeout(t);
  }, [left]);

  if (status === 'loading') {
    return (
      <div className={s.center}>
        <Spinner size={28} label="Chargement" />
      </div>
    );
  }
  if (status === 'authenticated') return <Navigate to={next} replace />;

  // Le serveur redirige toujours vers /app après consommation du lien : on garde la
  // destination demandée de côté, /app la rejoue une fois.
  const rememberNext = () => {
    if (next !== '/app') sessionStorage.setItem(PENDING_NEXT, next);
  };

  const send = async (e?: FormEvent) => {
    e?.preventDefault();
    if (sending) return;
    setError('');
    setSending(true);
    getAnalytics().track('signup_started', { method: 'magic_link' });
    try {
      rememberNext();
      await requestMagicLink(email.trim(), readHandoff(), refCode);
      setSent(true);
      setLeft(RESEND_SECONDS);
    } catch (err) {
      getAnalytics().track('signup_failed', {
        method: 'magic_link',
        error_code: 'magic_link_request_failed',
      });
      setError(apiMessage(err));
    } finally {
      setSending(false);
    }
  };

  const oauth = features?.google === true || features?.apple === true;
  const nothing = features !== null && !features.magic && !oauth;

  return (
    <div className={s.authWrap}>
      <div className={s.authCard}>
        <Link className={s.authBrand} to="/">
          <img
            className={s.authLogo}
            src="/brand/siglair-mark.png"
            alt=""
            width={28}
            height={28}
            aria-hidden="true"
          />
          Siglair
        </Link>

        {sent ? (
          <>
            <h1 className={s.authTitle}>Vérifiez vos emails</h1>
            <p className={s.muted}>
              Si un compte peut être créé ou retrouvé pour <strong>{email.trim()}</strong>, un lien
              de connexion vient d’y être envoyé. Il est valable 15 minutes et ne fonctionne
              qu’une fois.
            </p>
            <p className={s.muted}>
              Rien reçu ? Regardez dans les indésirables — le message vient de Siglair.
            </p>
            {error && (
              <p className={s.muted} role="alert">
                {error}
              </p>
            )}
            <Button
              variant="ghost"
              loading={sending}
              disabled={left > 0}
              onClick={() => void send()}
            >
              {left > 0 ? `Renvoyer le lien dans ${left} s` : 'Renvoyer le lien'}
            </Button>
            <button
              type="button"
              className={s.linkBtn}
              onClick={() => {
                setSent(false);
                setError('');
              }}
            >
              Utiliser une autre adresse
            </button>
          </>
        ) : (
          <>
            <h1 className={s.authTitle}>Connexion à Siglair</h1>
            <p className={s.muted}>
              Pas de mot de passe : on vous envoie un lien de connexion par email.
            </p>

            {nothing && (
              <p className={`${s.notice} ${s.alert}`} role="alert">
                Aucune méthode de connexion n’est configurée sur cette instance. Contactez
                l’administrateur.
              </p>
            )}

            {features?.magic !== false && (
              <form onSubmit={(e) => void send(e)} className={s.stack} noValidate>
                <Field
                  label="Adresse email"
                  hint="On vous envoie un lien, rien à retenir."
                  error={error}
                >
                  <Input
                    type="email"
                    name="email"
                    value={email}
                    required
                    autoComplete="email"
                    autoFocus
                    placeholder="vous@entreprise.com"
                    onChange={(e) => setEmail(e.currentTarget.value)}
                  />
                </Field>
                <Button type="submit" loading={sending}>
                  Recevoir le lien
                </Button>
              </form>
            )}

            {oauth && features?.magic !== false && <p className={s.sep}>ou</p>}

            {/* Un bouton vers un fournisseur non configuré renverrait une 500 : on le masque. */}
            {features?.google && (
              <a
                className={s.oauth}
                href={withRef(oauthStartUrl('google'), refCode)}
                onClick={() => {
                  rememberNext();
                  getAnalytics().track('signup_started', { method: 'google' });
                }}
              >
                Continuer avec Google
              </a>
            )}
            {features?.apple && (
              <a
                className={s.oauth}
                href={withRef(oauthStartUrl('apple'), refCode)}
                onClick={() => {
                  rememberNext();
                  getAnalytics().track('signup_started', { method: 'apple' });
                }}
              >
                Continuer avec Apple
              </a>
            )}

            <p className={s.muted}>
              En continuant, vous acceptez les <Link to="/legal/cgu">conditions</Link> et la{' '}
              <Link to="/legal/confidentialite">politique de confidentialité</Link>.
            </p>
          </>
        )}
      </div>
    </div>
  );
}
