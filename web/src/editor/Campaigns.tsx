/**
 * Campagnes datées : une bannière programmée au niveau de l'organisation.
 *
 * Tout est en base (`/api/campaigns`, src/routes/campaigns.rs) : ce panneau ne conserve
 * que le brouillon du formulaire. L'état d'une campagne (à venir / active / terminée) est
 * recalculé depuis ses dates à chaque rendu — aucun statut n'est stocké, aucun ne ment.
 *
 * La portée vient de `limits.campaigns` : `none` montre la fonction sans la donner,
 * `own` la donne, `team` y ajoute le déploiement sur toutes les signatures.
 */
import { useMemo, useState } from 'react';
import type { Dispatch, FormEvent } from 'react';
import { Link } from 'react-router-dom';
import { Button } from '../components/Button';
import { Field } from '../components/Field';
import { Input } from '../components/Input';
import { Spinner } from '../components/Spinner';
import { ConfirmDialog } from '../components/app/ConfirmDialog';
import { apiMessage, formatPrice } from '../components/app/helpers';
import { useToast } from '../components/Toast';
import { useSession } from '../lib/session';
import { defaultCampaigns, isCampaignActive, toLocalInput } from './state';
import type { Action, Campaign as LocalCampaign } from './state';
import {
  PHASE_LABELS,
  bannerSource,
  campaignPhase,
  inputToIso,
  isoToInput,
  useCampaigns,
  windowError,
} from './useCampaigns';
import type { Campaign, CampaignInput } from './useCampaigns';
import s from './editor.module.css';

interface Draft {
  name: string;
  message: string;
  href: string;
  color: string;
  start: string;
  end: string;
}

const inDays = (offset: number): string => toLocalInput(new Date(Date.now() + offset * 86_400_000));

const emptyDraft = (): Draft => ({
  name: '',
  message: '',
  href: '',
  color: '#2563eb',
  start: toLocalInput(new Date()),
  end: inDays(30),
});

const draftOf = (campaign: Campaign): Draft => ({
  name: campaign.name,
  message: campaign.message,
  href: campaign.href,
  color: campaign.color,
  start: isoToInput(campaign.starts_at),
  end: isoToInput(campaign.ends_at),
});

const toInput = (draft: Draft): CampaignInput => ({
  name: draft.name,
  message: draft.message,
  href: draft.href,
  color: draft.color,
  starts_at: inputToIso(draft.start),
  ends_at: inputToIso(draft.end),
});

/** « du 3 mars à 09:00 au 18 mars à 18:00 », en heure locale du lecteur. */
const humanDates = (campaign: Campaign): string => {
  const fmt = (iso: string): string =>
    new Date(iso).toLocaleString('fr-FR', { dateStyle: 'medium', timeStyle: 'short' });
  return `du ${fmt(campaign.starts_at)} au ${fmt(campaign.ends_at)}`;
};

export interface CampaignsProps {
  simulatedDate: string;
  onSimulatedDateChange: (value: string) => void;
  dispatch: Dispatch<Action>;
  /** @deprecated Les campagnes viennent de l'API. Props conservées le temps que Editor.tsx les retire. */
  campaigns?: LocalCampaign[];
  onCampaignsChange?: (campaigns: LocalCampaign[]) => void;
  enabled?: boolean;
  onEnabledChange?: (value: boolean) => void;
}

export function Campaigns({ simulatedDate, onSimulatedDateChange, dispatch }: CampaignsProps) {
  const toast = useToast();
  const { scope, canWrite, items, loading, error, now, teamSignatures, create, update, remove, pushToTeam } =
    useCampaigns();

  const [draft, setDraft] = useState<Draft | null>(null);
  const [editing, setEditing] = useState<Campaign | null>(null);
  const [saving, setSaving] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);
  const [confirm, setConfirm] = useState<{ kind: 'delete' | 'push'; campaign: Campaign } | null>(null);
  const [busy, setBusy] = useState(false);

  const set = <K extends keyof Draft>(key: K, value: Draft[K]) =>
    setDraft((d) => (d ? { ...d, [key]: value } : d));

  const dateError = draft ? windowError(draft.start, draft.end) : null;

  /** Ce que la date simulée montrerait : un repère de travail, pas l'état réel. */
  const simulated = new Date(simulatedDate);
  const simulatedAt = Number.isNaN(simulated.getTime()) ? null : simulated.getTime();
  const simulatedActiveId = simulatedAt === null ? null : items.find((c) => campaignPhase(c, simulatedAt) === 'active')?.id ?? null;

  function openCreate() {
    setEditing(null);
    setFormError(null);
    setDraft(emptyDraft());
  }

  function openEdit(campaign: Campaign) {
    setEditing(campaign);
    setFormError(null);
    setDraft(draftOf(campaign));
  }

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!draft || dateError) return;
    setSaving(true);
    setFormError(null);
    try {
      if (editing) {
        await update(editing.id, toInput(draft));
        toast('Campagne enregistrée.', 'success');
      } else {
        await create(toInput(draft));
        toast('Campagne programmée.', 'success');
      }
      setDraft(null);
      setEditing(null);
    } catch (e) {
      // 402, 403, 422 : le serveur écrit déjà en français, on n'invente rien par-dessus.
      setFormError(apiMessage(e));
    } finally {
      setSaving(false);
    }
  }

  function apply(campaign: Campaign) {
    dispatch({ type: 'applyCampaign', campaign: bannerSource(campaign) });
    toast('Bannière appliquée à cette signature.', 'success');
  }

  async function runConfirm() {
    if (!confirm) return;
    setBusy(true);
    try {
      if (confirm.kind === 'delete') {
        await remove(confirm.campaign.id);
        toast('Campagne supprimée.', 'success');
      } else {
        const { updated, failed } = await pushToTeam(confirm.campaign);
        toast(
          failed === 0
            ? `Bannière posée sur ${updated} signature${updated > 1 ? 's' : ''}. Republiez-les pour que l'e-mail change.`
            : `${updated} signature${updated > 1 ? 's' : ''} mise${updated > 1 ? 's' : ''} à jour, ${failed} en échec. Réessayez.`,
          failed === 0 ? 'success' : 'error',
        );
      }
      setConfirm(null);
    } catch (e) {
      toast(apiMessage(e), 'error');
    } finally {
      setBusy(false);
    }
  }

  if (scope === 'none') return <LockedPanel />;

  return (
    <>
      <div className={s.sectionHead}>
        <h3>Campagne</h3>
        <span>bannière datée</span>
      </div>

      <p className={s.note}>
        Une bannière programmée s’affiche et disparaît toute seule, à la date près, dans les
        signatures de {scope === 'team' ? 'toute votre organisation' : 'votre organisation'}.
      </p>

      <div className={`${s.sectionHead} ${s.gap}`}>
        <h3>Programmées</h3>
        <span>{loading ? '…' : items.length}</span>
      </div>

      {loading && <Spinner size={18} label="Chargement des campagnes" />}

      {error && (
        <p className={`${s.note} ${s.noteWarn}`} role="alert">
          {error}
        </p>
      )}

      {!loading && !error && items.length === 0 && (
        <p className={s.note}>
          Aucune campagne pour l’instant. Programmez-en une : elle apparaîtra d’elle-même le jour
          venu.
        </p>
      )}

      {items.map((campaign) => {
        const phase = campaignPhase(campaign, now);
        return (
          <div
            key={campaign.id}
            className={[s.campaign, campaign.id === simulatedActiveId ? s.campaignActive : null]
              .filter(Boolean)
              .join(' ')}
          >
            <div className={s.campaignHead}>
              <strong>{campaign.name}</strong>
              <span
                className={[s.badgeState, phase === 'active' ? null : s.badgeIdle].filter(Boolean).join(' ')}
              >
                {PHASE_LABELS[phase]}
              </span>
            </div>
            <p>{campaign.message}</p>
            <p className={s.campaignDates}>{humanDates(campaign)}</p>
            <div className={s.row2}>
              <button type="button" className={s.tinyBtn} onClick={() => apply(campaign)}>
                Appliquer à ma signature
              </button>
              {canWrite && (
                <button type="button" className={s.tinyBtn} onClick={() => openEdit(campaign)}>
                  Modifier
                </button>
              )}
            </div>
            {canWrite && (
              <div className={s.row2}>
                {scope === 'team' && (
                  <button
                    type="button"
                    className={s.tinyBtn}
                    onClick={() => setConfirm({ kind: 'push', campaign })}
                  >
                    Pousser à toute l’équipe
                    {teamSignatures !== null && ` (${teamSignatures})`}
                  </button>
                )}
                <button
                  type="button"
                  className={s.tinyBtn}
                  onClick={() => setConfirm({ kind: 'delete', campaign })}
                >
                  Supprimer
                </button>
              </div>
            )}
          </div>
        );
      })}

      {canWrite &&
        (draft ? (
          <form className={s.stack} onSubmit={submit}>
            <Field label="Nom" hint="Interne : sert à retrouver la campagne dans cette liste.">
              <Input
                value={draft.name}
                maxLength={120}
                required
                onChange={(e) => set('name', e.currentTarget.value)}
              />
            </Field>
            <Field label="Message affiché">
              <Input
                value={draft.message}
                maxLength={300}
                required
                onChange={(e) => set('message', e.currentTarget.value)}
              />
            </Field>
            <Field label="Lien" hint="Vide = bannière non cliquable. http(s), mailto: ou tel:">
              <Input
                type="url"
                value={draft.href}
                placeholder="https://exemple.fr/offre"
                onChange={(e) => set('href', e.currentTarget.value)}
              />
            </Field>
            <Field label="Couleur de la bannière">
              <Input type="color" value={draft.color} onChange={(e) => set('color', e.currentTarget.value)} />
            </Field>
            <div className={s.row2}>
              <Field label="Début">
                <Input
                  type="datetime-local"
                  value={draft.start}
                  required
                  onChange={(e) => set('start', e.currentTarget.value)}
                />
              </Field>
              <Field label="Fin" error={dateError ?? undefined}>
                <Input
                  type="datetime-local"
                  value={draft.end}
                  required
                  min={draft.start}
                  onChange={(e) => set('end', e.currentTarget.value)}
                />
              </Field>
            </div>
            {formError && (
              <p className={`${s.note} ${s.noteWarn}`} role="alert">
                {formError}
              </p>
            )}
            <div className={s.row2}>
              <Button type="submit" loading={saving} disabled={dateError !== null}>
                {editing ? 'Enregistrer' : 'Programmer'}
              </Button>
              <Button variant="ghost" onClick={() => setDraft(null)}>
                Annuler
              </Button>
            </div>
          </form>
        ) : (
          <Button variant="ghost" onClick={openCreate}>
            + Nouvelle campagne
          </Button>
        ))}

      {!canWrite && (
        <p className={`${s.note} ${s.gap}`}>
          Seuls les administrateurs de l’organisation créent et modifient les campagnes. Vous
          pouvez appliquer une bannière existante à votre signature.
        </p>
      )}

      <div className={`${s.stack} ${s.gap}`}>
        <Field label="Date simulée" hint="Pour vérifier quelle campagne serait active ce jour-là.">
          <Input
            type="datetime-local"
            value={simulatedDate}
            onChange={(event) => onSimulatedDateChange(event.target.value)}
          />
        </Field>
      </div>

      <p className={`${s.note} ${s.gap}`}>
        La bannière est servie par l’URL hébergée : vous changez la campagne, la signature déjà
        collée dans les emails se met à jour après republication.
      </p>

      <ConfirmDialog
        open={confirm?.kind === 'delete'}
        title="Supprimer la campagne"
        confirmLabel="Supprimer"
        danger
        loading={busy}
        onConfirm={() => void runConfirm()}
        onClose={() => setConfirm(null)}
      >
        <p>
          « {confirm?.campaign.name} » sera définitivement supprimée. Les bannières déjà posées
          dans les documents restent en place : retirez-les depuis le panneau Calques.
        </p>
      </ConfirmDialog>

      <ConfirmDialog
        open={confirm?.kind === 'push'}
        title="Pousser la campagne à toute l’équipe"
        confirmLabel={
          teamSignatures === null
            ? 'Pousser la bannière'
            : `Pousser sur ${teamSignatures} signature${teamSignatures > 1 ? 's' : ''}`
        }
        loading={busy}
        onConfirm={() => void runConfirm()}
        onClose={() => setConfirm(null)}
      >
        <p>
          La bannière « {confirm?.campaign.name} » sera ajoutée — ou mise à jour si elle existe
          déjà — dans{' '}
          <strong>
            {teamSignatures === null
              ? 'toutes les signatures'
              : `${teamSignatures} signature${teamSignatures > 1 ? 's' : ''}`}{' '}
            de l’organisation
          </strong>
          , y compris celles des autres membres.
        </p>
        <p>{confirm ? humanDates(confirm.campaign) : null}</p>
        <p>
          Les documents sont modifiés immédiatement. Chaque signature doit ensuite être republiée
          pour que le changement parte dans les emails déjà envoyés.
        </p>
      </ConfirmDialog>
    </>
  );
}

/**
 * Plan Free : on montre la fonction en marche plutôt qu'un cadenas. Les deux exemples sont
 * ceux de la démonstration, datés par rapport à aujourd'hui, avec leur état réel.
 * Le prix vient du catalogue servi par l'API — jamais d'un montant écrit ici.
 */
function LockedPanel() {
  const { plans } = useSession();
  const demo = useMemo(defaultCampaigns, []);
  const now = new Date();

  const cheapest = plans
    .filter((p) => p.limits.campaigns !== 'none')
    .sort((a, b) => a.price_eur_month - b.price_eur_month)[0];

  return (
    <>
      <div className={s.sectionHead}>
        <h3>Campagne</h3>
        <span>bannière datée</span>
      </div>

      <p className={s.note}>
        Programmez une bannière une fois : elle apparaît le jour dit et disparaît toute seule, dans
        les signatures déjà collées dans les emails de votre équipe. Aucun HTML à recoller.
      </p>

      <div className={`${s.sectionHead} ${s.gap}`}>
        <h3>Exemple</h3>
        <span>aperçu</span>
      </div>

      {demo.map((campaign) => (
        <div
          key={campaign.id}
          className={[s.campaign, isCampaignActive(campaign, now) ? s.campaignActive : null]
            .filter(Boolean)
            .join(' ')}
        >
          <div className={s.campaignHead}>
            <strong>{campaign.name}</strong>
            <span
              className={[s.badgeState, isCampaignActive(campaign, now) ? null : s.badgeIdle]
                .filter(Boolean)
                .join(' ')}
            >
              {isCampaignActive(campaign, now) ? 'active' : 'à venir'}
            </span>
          </div>
          <p>{campaign.message}</p>
          <p className={s.campaignDates}>
            du {campaign.start.replace('T', ' à ')} au {campaign.end.replace('T', ' à ')}
          </p>
        </div>
      ))}

      <p className={`${s.note} ${s.noteGood} ${s.gap}`}>
        {cheapest
          ? `Les campagnes datées sont incluses à partir du plan ${cheapest.name} (${formatPrice(cheapest.price_eur_month)} par mois).`
          : 'Les campagnes datées sont incluses dans les plans payants.'}{' '}
        Elles arrivent avec le GIF hébergé sur une URL stable : c’est lui qui permet à une bannière
        de changer sans que personne ne recolle sa signature.
      </p>

      {/* Un lien, pas un bouton dans un lien : le clic milieu et « ouvrir dans un onglet » marchent. */}
      <Link className={s.tinyBtn} to="/app/billing">
        Voir les plans
      </Link>
    </>
  );
}
