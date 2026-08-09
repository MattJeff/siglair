import { BarChart3, CalendarClock, MousePointerClick, RefreshCw } from 'lucide-react';
import s from './campaign.module.css';

const CAMPAIGNS = [
  {
    date: 'Maintenant',
    name: 'Guide signature 2026',
    state: 'Publiée',
    active: true,
  },
  {
    date: '18 septembre',
    name: 'Webinaire produit',
    state: 'Planifiée',
    active: false,
  },
  {
    date: '2 octobre',
    name: 'Nouveau cas client',
    state: 'Brouillon',
    active: false,
  },
];

export function CampaignDemo() {
  return (
    <figure className={s.figure}>
      <div className={s.product} aria-label="Démonstration des campagnes de signature Siglair">
        <div className={s.topbar}>
          <div>
            <span className={s.topLabel}>Siglair Campaigns</span>
            <strong>Planning marketing</strong>
          </div>
          <span className={s.live}>Campagne publiée</span>
        </div>

        <div className={s.workspace}>
          <aside className={s.schedule}>
            <div className={s.panelTitle}>
              <CalendarClock size={15} aria-hidden="true" />
              <span>Campagnes</span>
              <small>3</small>
            </div>
            <div className={s.campaignList}>
              {CAMPAIGNS.map((campaign) => (
                <div
                  className={`${s.campaignRow} ${campaign.active ? s.campaignActive : ''}`}
                  key={campaign.name}
                >
                  <span className={s.campaignDate}>{campaign.date}</span>
                  <strong>{campaign.name}</strong>
                  <small>{campaign.state}</small>
                </div>
              ))}
            </div>
          </aside>

          <div className={s.preview}>
            <div className={s.previewHead}>
              <span>Aperçu dans un email</span>
              <span className={s.previewMode}>Gmail · Outlook · Apple Mail</span>
            </div>
            <div className={s.mail}>
              <div className={s.mailCopy} aria-hidden="true">
                <i />
                <i />
                <i />
              </div>
              <div className={s.signature}>
                <div className={s.identity}>
                  <img src="/demo-logo.png" alt="" width={256} height={256} decoding="async" />
                  <span className={s.rule} aria-hidden="true" />
                  <div>
                    <strong>Camille Roussel</strong>
                    <span>Directrice associée · Atelier Nord</span>
                    <small>camille@example.com · +33 1 99 00 12 34</small>
                  </div>
                </div>
                <div className={s.banner}>
                  <div>
                    <span>Nouveau guide</span>
                    <strong>7 idées pour faire travailler chaque email</strong>
                  </div>
                  <span className={s.bannerCta}>Télécharger</span>
                </div>
              </div>
            </div>
            <div className={s.updateBar}>
              <RefreshCw size={14} aria-hidden="true" />
              <span>Même URL hébergée</span>
              <strong>Dernière publication à l’instant</strong>
            </div>
          </div>

          <aside className={s.metrics}>
            <div className={s.panelTitle}>
              <BarChart3 size={15} aria-hidden="true" />
              <span>Performance</span>
            </div>
            <div className={s.metricLead}>
              <MousePointerClick size={17} aria-hidden="true" />
              <div>
                <strong>Clics par CTA</strong>
                <span>Données de démonstration</span>
              </div>
            </div>
            <div className={s.metricRows} aria-hidden="true">
              <div>
                <span>Bannière</span>
                <i><b style={{ width: '84%' }} /></i>
              </div>
              <div>
                <span>Prendre rendez-vous</span>
                <i><b style={{ width: '61%' }} /></i>
              </div>
              <div>
                <span>LinkedIn</span>
                <i><b style={{ width: '37%' }} /></i>
              </div>
            </div>
            <p>Identifiez le message qui intéresse vraiment vos destinataires.</p>
          </aside>
        </div>

        <div className={s.flow}>
          <span><CalendarClock size={14} aria-hidden="true" /> Préparer le temps fort</span>
          <i aria-hidden="true">→</i>
          <span><RefreshCw size={14} aria-hidden="true" /> Republier sans changer l’URL</span>
          <i aria-hidden="true">→</i>
          <span><BarChart3 size={14} aria-hidden="true" /> Mesurer chaque CTA</span>
        </div>
      </div>
      <figcaption>
        Exemple fictif. La campagne est appliquée à la signature hébergée et ses clics sont suivis
        séparément des autres liens.
      </figcaption>
    </figure>
  );
}
