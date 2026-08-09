import { useCallback, useEffect, useState } from 'react';
import type { FormEvent } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { Button } from '../components/Button';
import { Field } from '../components/Field';
import { Input } from '../components/Input';
import { Select } from '../components/Select';
import { Spinner } from '../components/Spinner';
import { useToast } from '../components/Toast';
import { AppShell } from '../components/app/AppShell';
import { ConfirmDialog } from '../components/app/ConfirmDialog';
import { apiMessage, formatDate } from '../components/app/helpers';
import {
  createSignature,
  inviteMember,
  listMembers,
  listSignatures,
  removeMember,
  rollout,
} from '../lib/api';
import { useSession } from '../lib/session';
import type { Member, Role, Signature } from '../lib/types';
import s from './app.module.css';

const ROLE_LABEL: Record<Role, string> = {
  owner: 'Propriétaire',
  admin: 'Administrateur',
  member: 'Membre',
};

export default function Team() {
  const { user, currentOrg, limits } = useSession();
  const navigate = useNavigate();
  const toast = useToast();

  const orgId = currentOrg?.id ?? '';
  const canManage = currentOrg?.role === 'owner' || currentOrg?.role === 'admin';
  const teamPlan = limits?.org_templates === true;

  const [members, setMembers] = useState<Member[] | null>(null);
  const [templates, setTemplates] = useState<Signature[]>([]);
  const [loadError, setLoadError] = useState('');

  const [email, setEmail] = useState('');
  const [role, setRole] = useState<Role>('member');
  const [inviteError, setInviteError] = useState('');
  const [inviting, setInviting] = useState(false);

  const [toRemove, setToRemove] = useState<Member | null>(null);
  const [removing, setRemoving] = useState(false);

  const [templateId, setTemplateId] = useState('');
  const [rolloutOpen, setRolloutOpen] = useState(false);
  const [rollingOut, setRollingOut] = useState(false);
  const [creatingTemplate, setCreatingTemplate] = useState(false);

  const load = useCallback(async () => {
    if (!orgId) return;
    try {
      const [list, signatures] = await Promise.all([listMembers(orgId), listSignatures()]);
      setMembers(list);
      setTemplates(signatures.filter((sig) => sig.kind === 'org_template'));
      setLoadError('');
    } catch (e) {
      setMembers([]);
      setLoadError(apiMessage(e));
    }
  }, [orgId]);

  useEffect(() => {
    void load();
  }, [load]);

  const owners = (members ?? []).filter((m) => m.role === 'owner');

  /** Refusé côté interface AVANT l'appel : une org sans propriétaire n'a plus d'administrateur. */
  const lastOwner = (m: Member) => m.role === 'owner' && owners.length <= 1;

  const invite = async (e: FormEvent) => {
    e.preventDefault();
    setInviteError('');
    setInviting(true);
    try {
      await inviteMember(orgId, email.trim(), role);
      setEmail('');
      toast('Invitation envoyée.', 'success');
      await load();
    } catch (err) {
      setInviteError(apiMessage(err));
    } finally {
      setInviting(false);
    }
  };

  const confirmRemove = async () => {
    if (!toRemove) return;
    setRemoving(true);
    try {
      await removeMember(orgId, toRemove.user_id);
      setToRemove(null);
      await load();
      toast('Membre retiré.', 'success');
    } catch (e) {
      toast(apiMessage(e), 'error');
    } finally {
      setRemoving(false);
    }
  };

  const newTemplate = async () => {
    setCreatingTemplate(true);
    try {
      const sig = await createSignature({ name: 'Modèle d’équipe', kind: 'org_template' });
      navigate(`/app/editor/${sig.id}`);
    } catch (e) {
      toast(apiMessage(e), 'error');
    } finally {
      setCreatingTemplate(false);
    }
  };

  const confirmRollout = async () => {
    setRollingOut(true);
    try {
      const { count } = await rollout(orgId, templateId);
      setRolloutOpen(false);
      toast(
        `${count} signature${count > 1 ? 's' : ''} générée${count > 1 ? 's' : ''} et remise${
          count > 1 ? 's' : ''
        } en publication.`,
        'success',
      );
    } catch (e) {
      toast(apiMessage(e), 'error');
    } finally {
      setRollingOut(false);
    }
  };

  const template = templates.find((t) => t.id === templateId) ?? null;

  return (
    <AppShell
      title="Équipe"
      subtitle={currentOrg ? currentOrg.name : undefined}
    >
      <div className={s.stack}>
        {loadError && (
          <p className={`${s.notice} ${s.alert}`} role="alert">
            {loadError}
          </p>
        )}

        {!teamPlan && (
          <div className={`${s.notice} ${s.upsell}`}>
            <span>
              Les membres, les rôles et le déploiement en masse font partie du plan Équipe. Une
              signature changée une fois, appliquée à tout le monde, sans que personne ne touche à
              son client mail.
            </span>
            <Link className={s.linkBtn} to="/app/billing">
              Voir les plans
            </Link>
          </div>
        )}

        <section className={s.card}>
          <div className={s.cardHead}>
            <h2 className={s.cardTitle}>Membres</h2>
            <p className={s.muted}>
              {members === null ? '' : `${members.length} personne${members.length > 1 ? 's' : ''}`}
            </p>
          </div>

          {members === null ? (
            <Spinner size={22} label="Chargement des membres" />
          ) : (
            <ul className={`${s.rows} ${s.plainList}`}>
              {members.map((m) => {
                const self = m.user_id === user?.id;
                const blocked = lastOwner(m);
                return (
                  <li className={s.row} key={m.user_id}>
                    <div className={s.rowMain}>
                      <p className={s.rowName}>
                        {m.name ?? m.email}
                        {self && ' (vous)'}
                      </p>
                      <p className={s.muted}>
                        {m.email} · {ROLE_LABEL[m.role]} · depuis le {formatDate(m.created_at)}
                      </p>
                      {blocked && (
                        <p className={s.muted}>
                          Dernier propriétaire : il ne peut être ni retiré ni rétrogradé. Nommez
                          d’abord un autre propriétaire.
                        </p>
                      )}
                    </div>
                    {canManage && !blocked && (
                      <Button variant="danger" onClick={() => setToRemove(m)}>
                        {self ? 'Quitter l’équipe' : 'Retirer'}
                      </Button>
                    )}
                  </li>
                );
              })}
            </ul>
          )}
        </section>

        {canManage && teamPlan && (
          <section className={s.card}>
            <div className={s.cardHead}>
              <h2 className={s.cardTitle}>Inviter quelqu’un</h2>
            </div>
            <form className={s.stack} onSubmit={(e) => void invite(e)} noValidate>
              <div className={s.fields}>
                <Field
                  label="Adresse email"
                  hint="La personne recevra un lien d’invitation valable quelques jours."
                  error={inviteError}
                >
                  <Input
                    type="email"
                    value={email}
                    required
                    autoComplete="off"
                    placeholder="collegue@entreprise.com"
                    onChange={(e) => setEmail(e.currentTarget.value)}
                  />
                </Field>
                <Field label="Rôle">
                  <Select
                    value={role}
                    onChange={(e) => setRole(e.currentTarget.value as Role)}
                  >
                    <option value="member">{ROLE_LABEL.member}</option>
                    <option value="admin">{ROLE_LABEL.admin}</option>
                  </Select>
                </Field>
              </div>
              <div className={s.actions}>
                <Button type="submit" loading={inviting}>
                  Envoyer l’invitation
                </Button>
              </div>
            </form>
          </section>
        )}

        {teamPlan && (
          <section className={s.card}>
            <div className={s.cardHead}>
              <h2 className={s.cardTitle}>Modèle d’équipe</h2>
            </div>
            <p className={s.muted}>
              Un modèle unique, le profil de chaque membre ({'{{name}}'}, {'{{role}}'}…), et une
              signature cohérente générée pour tout le monde.
            </p>

            {templates.length === 0 ? (
              <div className={s.actions}>
                <Button loading={creatingTemplate} onClick={() => void newTemplate()}>
                  Créer un modèle d’équipe
                </Button>
              </div>
            ) : (
              <div className={s.stack}>
                <div className={s.fields}>
                  <Field
                    label="Modèle à déployer"
                    hint="Seules les signatures marquées « modèle d’équipe » apparaissent ici."
                  >
                    <Select
                      value={templateId}
                      onChange={(e) => setTemplateId(e.currentTarget.value)}
                    >
                      <option value="">Choisir un modèle…</option>
                      {templates.map((t) => (
                        <option key={t.id} value={t.id}>
                          {t.name}
                        </option>
                      ))}
                    </Select>
                  </Field>
                </div>
                <div className={s.actions}>
                  <Button
                    disabled={templateId === '' || !canManage}
                    onClick={() => setRolloutOpen(true)}
                  >
                    Déployer à l’équipe
                  </Button>
                  {templateId !== '' && (
                    <Link className={s.linkBtn} to={`/app/editor/${templateId}`}>
                      Modifier le modèle
                    </Link>
                  )}
                </div>
                {!canManage && (
                  <p className={s.muted}>
                    Seuls les propriétaires et administrateurs peuvent déployer un modèle.
                  </p>
                )}
              </div>
            )}
          </section>
        )}
      </div>

      <ConfirmDialog
        open={toRemove !== null}
        title={toRemove?.user_id === user?.id ? 'Quitter l’équipe ?' : 'Retirer ce membre ?'}
        confirmLabel={toRemove?.user_id === user?.id ? 'Quitter' : 'Retirer'}
        danger
        loading={removing}
        onClose={() => setToRemove(null)}
        onConfirm={() => void confirmRemove()}
      >
        <p>
          {toRemove?.name ?? toRemove?.email} n’aura plus accès aux signatures de{' '}
          {currentOrg?.name}. Les signatures déjà publiées continuent de s’afficher tant qu’elles
          ne sont pas supprimées.
        </p>
      </ConfirmDialog>

      {/* Action de masse : récapitulatif chiffré avant de lancer, pas un simple bouton. */}
      <ConfirmDialog
        open={rolloutOpen}
        title="Déployer le modèle à toute l’équipe"
        confirmLabel={`Déployer sur ${members?.length ?? 0} membre${
          (members?.length ?? 0) > 1 ? 's' : ''
        }`}
        loading={rollingOut}
        onClose={() => setRolloutOpen(false)}
        onConfirm={() => void confirmRollout()}
      >
        <p>
          Modèle : <strong>{template?.name}</strong>
        </p>
        <p>
          <strong>{members?.length ?? 0}</strong> signature
          {(members?.length ?? 0) > 1 ? 's seront régénérées' : ' sera régénérée'} à partir de ce
          modèle et du profil de chaque membre, puis republiée
          {(members?.length ?? 0) > 1 ? 's' : ''}.
        </p>
        <p>
          Les signatures d’équipe existantes seront remplacées. Il n’y a pas de retour en arrière
          automatique : pour revenir en arrière, il faudra déployer un autre modèle.
        </p>
        <p>
          Les membres n’ont rien à faire : leur URL hébergée reste la même, le contenu se met à
          jour dans les emails déjà envoyés.
        </p>
      </ConfirmDialog>
    </AppShell>
  );
}
