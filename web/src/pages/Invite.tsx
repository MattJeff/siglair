import { useState } from 'react';
import { Link, useNavigate, useParams } from 'react-router-dom';
import { Button } from '../components/Button';
import { Spinner } from '../components/Spinner';
import { apiMessage, errorCode } from '../components/app/helpers';
import { acceptInvite, logout } from '../lib/api';
import { useSession } from '../lib/session';
import type { Org } from '../lib/types';
import s from './app.module.css';

type Failure = 'expired' | 'used' | 'wrong_account' | 'other';

/** Les trois échecs fréquents méritent chacun leur explication, pas un « erreur 400 ». */
function classify(e: unknown): Failure {
  switch (errorCode(e)) {
    case 'not_found':
    case 'validation':
      return 'expired';
    case 'conflict':
      return 'used';
    case 'forbidden':
      return 'wrong_account';
    default:
      return 'other';
  }
}

export default function Invite() {
  const { token = '' } = useParams();
  const { status, user, refresh } = useSession();
  const navigate = useNavigate();

  const [joined, setJoined] = useState<Org | null>(null);
  const [failure, setFailure] = useState<Failure | null>(null);
  const [message, setMessage] = useState('');
  const [busy, setBusy] = useState(false);

  const accept = async () => {
    setBusy(true);
    setFailure(null);
    try {
      const org = await acceptInvite(token);
      setJoined(org);
      await refresh();
    } catch (e) {
      setFailure(classify(e));
      setMessage(apiMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const switchAccount = async () => {
    setBusy(true);
    try {
      await logout();
    } finally {
      location.assign(`/login?next=${encodeURIComponent(`/invite/${token}`)}`);
    }
  };

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

        {status === 'loading' && <Spinner size={28} label="Chargement de l’invitation" />}

        {status !== 'loading' && joined && (
          <>
            <h1 className={s.authTitle}>Bienvenue dans {joined.name}</h1>
            <p className={s.muted}>
              Vous faites maintenant partie de cette équipe. Votre profil ({'{{name}}'},{' '}
              {'{{role}}'}…) alimentera les signatures générées pour vous.
            </p>
            <Button onClick={() => navigate('/app')}>Aller à mes signatures</Button>
            <Link className={s.linkBtn} to="/app/settings">
              Compléter mon profil
            </Link>
          </>
        )}

        {status !== 'loading' && !joined && failure === null && (
          <>
            <h1 className={s.authTitle}>Vous êtes invité à rejoindre une équipe</h1>
            {status === 'anonymous' ? (
              <>
                <p className={s.muted}>
                  Connectez-vous pour voir l’équipe qui vous invite et accepter l’invitation.
                </p>
                <Button
                  onClick={() =>
                    navigate(`/login?next=${encodeURIComponent(`/invite/${token}`)}`)
                  }
                >
                  Se connecter pour accepter
                </Button>
              </>
            ) : (
              <>
                <p className={s.muted}>
                  Vous êtes connecté avec <strong>{user?.email}</strong>. L’invitation doit avoir
                  été envoyée à cette adresse.
                </p>
                <Button loading={busy} onClick={() => void accept()}>
                  Accepter l’invitation
                </Button>
                <button type="button" className={s.linkBtn} onClick={() => void switchAccount()}>
                  Ce n’est pas mon compte
                </button>
              </>
            )}
          </>
        )}

        {failure === 'expired' && (
          <>
            <h1 className={s.authTitle}>Invitation expirée ou introuvable</h1>
            <p className={s.muted}>
              Ce lien n’est plus valable. Les invitations ont une durée de vie limitée. Demandez à
              la personne qui vous a invité d’en envoyer une nouvelle.
            </p>
            <Link className={s.linkBtn} to="/">
              Revenir à l’accueil
            </Link>
          </>
        )}

        {failure === 'used' && (
          <>
            <h1 className={s.authTitle}>Invitation déjà acceptée</h1>
            <p className={s.muted}>
              Ce lien a déjà servi. Si vous êtes membre de l’équipe, vos signatures vous attendent.
            </p>
            <Link className={s.linkBtn} to="/app">
              Aller à mes signatures
            </Link>
          </>
        )}

        {failure === 'wrong_account' && (
          <>
            <h1 className={s.authTitle}>Cette invitation ne vous est pas destinée</h1>
            <p className={s.muted}>
              Elle a été envoyée à une autre adresse que <strong>{user?.email}</strong>.
              Reconnectez-vous avec l’adresse qui a reçu l’email d’invitation.
            </p>
            <Button loading={busy} onClick={() => void switchAccount()}>
              Changer de compte
            </Button>
          </>
        )}

        {failure === 'other' && (
          <>
            <h1 className={s.authTitle}>Impossible d’accepter l’invitation</h1>
            <p className={s.muted} role="alert">
              {message}
            </p>
            <Button variant="ghost" loading={busy} onClick={() => void accept()}>
              Réessayer
            </Button>
          </>
        )}
      </div>
    </div>
  );
}
