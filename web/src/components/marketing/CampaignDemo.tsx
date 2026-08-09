import {
  BarChart3,
  CalendarClock,
  Check,
  Mail,
  MousePointerClick,
  RefreshCw,
  Send,
  Users,
} from 'lucide-react';
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
            <span className={s.topLabel}>Campagne active</span>
            <strong>Guide signature 2026</strong>
          </div>
          <div className={s.deliveryState}>
            <span className={s.avatarStack} aria-hidden="true">
              <i>CR</i>
              <i>AM</i>
              <i>JL</i>
            </span>
            <span className={s.live}>20 signatures à jour</span>
          </div>
        </div>

        <div className={s.workspace}>
          <aside className={s.schedule}>
            <div className={s.panelTitle}>
              <CalendarClock size={15} aria-hidden="true" />
              <span>Planning</span>
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
            <div className={s.reach}>
              <Users size={16} aria-hidden="true" />
              <div>
                <span>Équipe commerciale</span>
                <strong>20 collaborateurs</strong>
              </div>
              <Check size={14} aria-hidden="true" />
            </div>
          </aside>

          <div className={s.preview}>
            <div className={s.previewHead}>
              <span><Mail size={13} aria-hidden="true" /> Aperçu destinataire</span>
              <span className={s.previewMode}>Gmail · Outlook · Apple Mail</span>
            </div>
            <div className={s.mail}>
              <div className={s.mailHeader}>
                <div className={s.senderAvatar}>CR</div>
                <div>
                  <strong>Camille Roussel</strong>
                  <span>à Marc Durand</span>
                </div>
                <small>10:42</small>
              </div>
              <div className={s.mailCopy}>
                <strong>Re: Déploiement de votre nouvelle identité</strong>
                <p>Bonjour Marc,</p>
                <p>Tout est prêt de notre côté. Je vous laisse regarder la synthèse ci-jointe.</p>
              </div>
              <div className={s.signature}>
                <div className={s.identity}>
                  <img src="/demo-logo.png" alt="" width={256} height={256} decoding="async" />
                  <span className={s.rule} aria-hidden="true" />
                  <div>
                    <strong>Camille Roussel</strong>
                    <span>Directrice associée · Atelier Nord</span>
                    <small>camille@example.com · +33 1 99 00 12 34</small>
                    <span className={s.signatureLinks}>Prendre rendez-vous · LinkedIn</span>
                  </div>
                </div>
                <div className={s.banner}>
                  <div>
                    <span>Le guide 2026 vient de sortir</span>
                    <strong>7 idées pour transformer chaque email en opportunité</strong>
                  </div>
                  <span className={s.bannerCta}>Voir le guide <Send size={12} aria-hidden="true" /></span>
                </div>
              </div>
            </div>
            <div className={s.updateBar}>
              <RefreshCw size={14} aria-hidden="true" />
              <span>La même URL vient d’être actualisée</span>
              <strong><Check size={12} aria-hidden="true" /> 20/20 publiées</strong>
            </div>
          </div>

          <aside className={s.metrics}>
            <div className={s.panelTitle}>
              <BarChart3 size={15} aria-hidden="true" />
              <span>Résultats</span>
            </div>
            <div className={s.metricLead}>
              <MousePointerClick size={17} aria-hidden="true" />
              <div>
                <strong>312 clics</strong>
                <span>30 derniers jours · démo</span>
              </div>
            </div>
            <div className={s.metricRows} aria-hidden="true">
              <div>
                <span><b>Bannière</b><em>184</em></span>
                <i><b style={{ width: '84%' }} /></i>
              </div>
              <div>
                <span><b>Rendez-vous</b><em>86</em></span>
                <i><b style={{ width: '61%' }} /></i>
              </div>
              <div>
                <span><b>LinkedIn</b><em>42</em></span>
                <i><b style={{ width: '37%' }} /></i>
              </div>
            </div>
            <div className={s.bestCta}>
              <span>CTA le plus performant</span>
              <strong>Voir le guide</strong>
              <small>59 % des clics</small>
            </div>
          </aside>
        </div>

        <div className={s.flow}>
          <span><CalendarClock size={14} aria-hidden="true" /> Planifier une fois</span>
          <i aria-hidden="true">→</i>
          <span><Users size={14} aria-hidden="true" /> Diffuser à l’équipe</span>
          <i aria-hidden="true">→</i>
          <span><BarChart3 size={14} aria-hidden="true" /> Mesurer le message</span>
        </div>
      </div>
      <figcaption>
        Exemple fictif. Une campagne, une signature hébergée par collaborateur et des clics
        distingués pour chaque CTA.
      </figcaption>
    </figure>
  );
}
