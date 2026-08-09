/**
 * Campagnes datées : une bannière programmée, appliquée au document.
 *
 * Le contrat n'expose ni table ni route pour les campagnes : elles vivent donc dans l'écran,
 * seul leur effet (l'élément `banner`) est enregistré avec le document. Signalé dans le rapport.
 */
import { useState } from 'react';
import type { Dispatch, FormEvent } from 'react';
import { Button } from '../components/Button';
import { Field } from '../components/Field';
import { Input } from '../components/Input';
import { isCampaignActive, toLocalInput, uid } from './state';
import type { Action, Campaign } from './state';
import s from './editor.module.css';

const inThirtyDays = (): string => toLocalInput(new Date(Date.now() + 30 * 86_400_000));

interface CampaignsProps {
  campaigns: Campaign[];
  onCampaignsChange: (campaigns: Campaign[]) => void;
  simulatedDate: string;
  onSimulatedDateChange: (value: string) => void;
  enabled: boolean;
  onEnabledChange: (value: boolean) => void;
  dispatch: Dispatch<Action>;
}

export function Campaigns({
  campaigns,
  onCampaignsChange,
  simulatedDate,
  onSimulatedDateChange,
  enabled,
  onEnabledChange,
  dispatch,
}: CampaignsProps) {
  const [creating, setCreating] = useState(false);
  const now = new Date(simulatedDate);
  const reference = Number.isNaN(now.getTime()) ? new Date() : now;
  const activeId = enabled ? campaigns.find((c) => isCampaignActive(c, reference))?.id : undefined;

  function create(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const form = new FormData(event.currentTarget);
    const read = (key: string): string => String(form.get(key) ?? '');
    onCampaignsChange([
      ...campaigns,
      {
        id: uid(),
        name: read('name') || 'Nouvelle campagne',
        message: read('message'),
        href: read('href'),
        color: read('color'),
        start: read('start'),
        end: read('end'),
      },
    ]);
    setCreating(false);
  }

  return (
    <>
      <div className={s.sectionHead}>
        <h3>Campagne</h3>
        <span>bannière datée</span>
      </div>

      <div className={s.stack}>
        <Field label="Date simulée" hint="Pour vérifier quelle campagne serait active ce jour-là.">
          <Input
            type="datetime-local"
            value={simulatedDate}
            onChange={(event) => onSimulatedDateChange(event.target.value)}
          />
        </Field>
        <label className={s.switch}>
          <span>Activer les campagnes</span>
          <input type="checkbox" checked={enabled} onChange={(event) => onEnabledChange(event.target.checked)} />
        </label>
      </div>

      <div className={`${s.sectionHead} ${s.gap}`}>
        <h3>Programmées</h3>
        <span>{campaigns.length}</span>
      </div>

      {campaigns.map((campaign) => {
        const active = campaign.id === activeId;
        return (
          <div
            key={campaign.id}
            className={[s.campaign, active ? s.campaignActive : null].filter(Boolean).join(' ')}
          >
            <div className={s.campaignHead}>
              <strong>{campaign.name}</strong>
              <span className={[s.badgeState, active ? null : s.badgeIdle].filter(Boolean).join(' ')}>
                {active ? 'active' : 'programmée'}
              </span>
            </div>
            <p>{campaign.message}</p>
            <p className={s.campaignDates}>
              du {campaign.start.replace('T', ' à ')} au {campaign.end.replace('T', ' à ')}
            </p>
            <div className={s.row2}>
              <button
                type="button"
                className={s.tinyBtn}
                onClick={() => dispatch({ type: 'applyCampaign', campaign })}
              >
                Appliquer
              </button>
              <button
                type="button"
                className={s.tinyBtn}
                onClick={() => onCampaignsChange(campaigns.filter((c) => c.id !== campaign.id))}
              >
                Retirer
              </button>
            </div>
          </div>
        );
      })}

      {creating ? (
        <form className={s.stack} onSubmit={create}>
          <Field label="Nom">
            <Input name="name" defaultValue="Nouvelle campagne" required />
          </Field>
          <Field label="Message affiché">
            <Input name="message" defaultValue="✨ Une nouveauté à annoncer" required />
          </Field>
          <Field label="Lien">
            <Input name="href" type="url" defaultValue="https://" />
          </Field>
          <Field label="Couleur de la bannière">
            <Input name="color" type="color" defaultValue="#2563eb" />
          </Field>
          <div className={s.row2}>
            <Field label="Début">
              <Input name="start" type="datetime-local" defaultValue={simulatedDate} required />
            </Field>
            <Field label="Fin">
              <Input name="end" type="datetime-local" defaultValue={inThirtyDays()} required />
            </Field>
          </div>
          <div className={s.row2}>
            <Button type="submit">Créer</Button>
            <Button variant="ghost" onClick={() => setCreating(false)}>
              Annuler
            </Button>
          </div>
        </form>
      ) : (
        <Button variant="ghost" onClick={() => setCreating(true)}>
          + Nouvelle campagne
        </Button>
      )}

      <p className={`${s.note} ${s.gap}`}>
        La bannière est servie par l’URL hébergée : vous changez la campagne, la signature déjà
        collée dans les emails se met à jour après republication.
      </p>
    </>
  );
}
