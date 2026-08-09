import { useEffect, useState } from 'react';
import type { FormEvent } from 'react';
import { Link } from 'react-router-dom';
import { Button } from '../components/Button';
import { Field } from '../components/Field';
import { Input } from '../components/Input';
import { Spinner } from '../components/Spinner';
import { useToast } from '../components/Toast';
import { AppShell } from '../components/app/AppShell';
import { ConfirmDialog } from '../components/app/ConfirmDialog';
import { apiMessage, formatDate } from '../components/app/helpers';
import {
  ApiError,
  deleteAccount,
  listMembers,
  listSignatures,
  logout,
  updateMemberProfile,
  updateSignature,
} from '../lib/api';
import { useSession } from '../lib/session';
import { PROFILE_KEYS } from '../lib/types';
import type { Profile, ProfileKey } from '../lib/types';
import s from './app.module.css';

type Values = Record<ProfileKey, string>;

const EMPTY: Values = {
  name: '',
  role: '',
  email: '',
  phone: '',
  website: '',
  linkedin: '',
  whatsapp: '',
  tagline: '',
  company: '',
};

const FIELDS: { key: ProfileKey; label: string; type: string; placeholder: string }[] = [
  { key: 'name', label: 'Nom complet', type: 'text', placeholder: 'Camille Ferrand' },
  { key: 'role', label: 'Fonction', type: 'text', placeholder: 'Directrice marketing' },
  { key: 'email', label: 'Email affiché', type: 'email', placeholder: 'camille@exemple.com' },
  { key: 'phone', label: 'Téléphone', type: 'tel', placeholder: '+33 6 12 34 56 78' },
  { key: 'website', label: 'Site web', type: 'url', placeholder: 'https://exemple.com' },
  { key: 'linkedin', label: 'LinkedIn', type: 'url', placeholder: 'https://linkedin.com/in/…' },
  { key: 'whatsapp', label: 'WhatsApp', type: 'tel', placeholder: '+33 6 12 34 56 78' },
  { key: 'tagline', label: 'Accroche', type: 'text', placeholder: 'On répond en moins d’une heure' },
  { key: 'company', label: 'Entreprise', type: 'text', placeholder: 'Exemple SAS' },
];

/**
 * Écriture de `org_members.profile` en meilleur effort : la route n'est pas encore au
 * contrat §5.3. Renvoie false si elle n'est pas servie — les signatures de l'utilisateur
 * ont alors quand même été mises à jour, et c'est elles qui portent le rendu (§3.1).
 */
async function putMemberProfile(orgId: string, userId: string, profile: Profile): Promise<boolean> {
  try {
    await updateMemberProfile(orgId, userId, profile);
    return true;
  } catch (e) {
    if (e instanceof ApiError && (e.status === 404 || e.status === 405)) return false;
    throw e;
  }
}

export default function Settings() {
  const { user, currentOrg, plan } = useSession();
  const toast = useToast();

  const [values, setValues] = useState<Values>(EMPTY);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState('');
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [deleting, setDeleting] = useState(false);

  const orgId = currentOrg?.id ?? '';
  const userId = user?.id ?? '';

  useEffect(() => {
    if (!orgId || !userId) return;
    let alive = true;
    listMembers(orgId)
      .then((members) => {
        if (!alive) return;
        const me = members.find((m) => m.user_id === userId);
        const next = { ...EMPTY };
        for (const key of PROFILE_KEYS) next[key] = me?.profile[key] ?? '';
        // Pré-remplissage raisonnable au premier passage.
        if (!next.name) next.name = user?.name ?? '';
        if (!next.email) next.email = user?.email ?? '';
        setValues(next);
      })
      .catch((e: unknown) => {
        if (alive) setError(apiMessage(e));
      })
      .finally(() => {
        if (alive) setLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [orgId, userId, user?.name, user?.email]);

  const save = async (e: FormEvent) => {
    e.preventDefault();
    setSaving(true);
    setError('');
    try {
      const profile: Profile = {};
      for (const key of PROFILE_KEYS) {
        const v = values[key].trim();
        if (v) profile[key] = v;
      }

      // Les signatures portent leur propre copie du profil (§3.1) : on les met à jour,
      // sinon le formulaire ne changerait rien au rendu.
      const mine = (await listSignatures()).filter((sig) => sig.owner_user_id === userId);
      await Promise.all(mine.map((sig) => updateSignature(sig.id, { profile })));

      const stored = await putMemberProfile(orgId, userId, profile);
      if (!stored && mine.length === 0) {
        throw new ApiError(
          'internal',
          'Créez d’abord une signature : votre profil y sera enregistré.',
          404,
        );
      }

      toast(
        mine.length > 0
          ? `Profil enregistré sur ${mine.length} signature${mine.length > 1 ? 's' : ''}. Republiez-les pour que vos destinataires voient le changement.`
          : 'Profil enregistré.',
        'success',
      );
    } catch (err) {
      setError(apiMessage(err));
    } finally {
      setSaving(false);
    }
  };

  const onDelete = async () => {
    setDeleting(true);
    try {
      await deleteAccount();
      location.assign('/');
    } catch (e) {
      toast(apiMessage(e), 'error');
      setDeleting(false);
    }
  };

  const preview = [values.name, values.role, values.company].filter(Boolean);
  const contact = [values.email, values.phone, values.website].filter(Boolean);

  return (
    <AppShell title="Réglages" subtitle={user?.email}>
      <div className={s.stack}>
        <section className={s.card}>
          <div className={s.cardHead}>
            <h2 className={s.cardTitle}>Mon profil</h2>
          </div>
          <p className={s.muted}>
            Ces valeurs remplacent les jetons {'{{name}}'}, {'{{role}}'}, {'{{email}}'} … de vos
            signatures : vous écrivez le jeton une fois dans l’éditeur, et il affiche toujours la
            bonne information. Un champ laissé vide n’affiche rien du tout — jamais le jeton.
          </p>

          {loading ? (
            <Spinner size={22} label="Chargement du profil" />
          ) : (
            <form className={s.stack} onSubmit={(e) => void save(e)} noValidate>
              <div className={s.fields}>
                {FIELDS.map((f) => (
                  <Field key={f.key} label={f.label} hint={`{{${f.key}}}`}>
                    <Input
                      type={f.type}
                      value={values[f.key]}
                      placeholder={f.placeholder}
                      autoComplete="off"
                      onChange={(e) =>
                        setValues((prev) => ({ ...prev, [f.key]: e.currentTarget.value }))
                      }
                    />
                  </Field>
                ))}
              </div>

              <div className={s.preview}>
                <p className={s.muted}>Aperçu des valeurs</p>
                <p className={s.previewName}>{preview.join(' · ') || 'Votre nom apparaîtra ici'}</p>
                <p className={s.muted}>{contact.join(' · ') || 'Aucun contact renseigné'}</p>
                {values.tagline && <p className={s.muted}>{values.tagline}</p>}
              </div>

              {error && (
                <p className={`${s.notice} ${s.alert}`} role="alert">
                  {error}
                </p>
              )}

              <div className={s.actions}>
                <Button type="submit" loading={saving}>
                  Enregistrer le profil
                </Button>
              </div>
            </form>
          )}
        </section>

        <section className={s.card}>
          <div className={s.cardHead}>
            <h2 className={s.cardTitle}>Compte et sessions</h2>
          </div>
          <p className={s.muted}>
            Connecté avec <strong>{user?.email}</strong>. Compte créé le{' '}
            {formatDate(user?.created_at ?? null)}, dernière activité le{' '}
            {formatDate(user?.last_seen_at ?? null)}.
          </p>
          <p className={s.muted}>
            La déconnexion révoque immédiatement la session de cet appareil ; les autres appareils
            restent connectés jusqu’à l’expiration de leur session.
          </p>
          <div className={s.actions}>
            <Button
              variant="ghost"
              onClick={() => {
                void logout().finally(() => location.assign('/login'));
              }}
            >
              Se déconnecter
            </Button>
          </div>
        </section>

        <section className={s.card}>
          <div className={s.cardHead}>
            <h2 className={s.cardTitle}>Supprimer mon compte</h2>
          </div>
          <p className={s.muted}>
            La suppression est définitive et immédiate. Sont détruits :
          </p>
          <ul className={s.features}>
            <li>toutes vos signatures et leurs documents ;</li>
            <li>
              vos URL publiques : <strong>les emails déjà envoyés cesseront d’afficher votre
              signature</strong>, vos destinataires verront une image manquante ;
            </li>
            <li>vos fichiers importés (logos, images) ;</li>
            <li>vos statistiques d’ouverture et de clic ;</li>
            <li>votre appartenance à {currentOrg?.name ?? 'votre organisation'}.</li>
          </ul>
          {plan !== 'free' && (
            <p className={`${s.notice} ${s.warn}`}>
              Résiliez d’abord votre abonnement depuis{' '}
              <Link to="/app/billing">la page Abonnement</Link> : supprimer le compte n’annule pas
              un prélèvement en cours.
            </p>
          )}
          <div className={s.actions}>
            <Button variant="danger" onClick={() => setConfirmDelete(true)}>
              Supprimer mon compte
            </Button>
          </div>
        </section>
      </div>

      <ConfirmDialog
        open={confirmDelete}
        title="Supprimer définitivement votre compte"
        confirmLabel="Supprimer définitivement"
        danger
        loading={deleting}
        requireText={user?.email ?? ''}
        requireLabel={`Saisissez ${user?.email ?? 'votre adresse'} pour confirmer`}
        onClose={() => setConfirmDelete(false)}
        onConfirm={() => void onDelete()}
      >
        <p>
          Il n’y a pas de corbeille, pas de délai de grâce et pas de restauration possible. Les
          signatures que vos correspondants voient aujourd’hui dans leurs emails disparaîtront.
        </p>
        <p>
          Si vous vouliez seulement arrêter de payer, résiliez l’abonnement : votre compte reste
          utilisable avec les limites du plan gratuit.
        </p>
      </ConfirmDialog>
    </AppShell>
  );
}
