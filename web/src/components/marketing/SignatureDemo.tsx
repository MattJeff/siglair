/**
 * Signature animée de la page d'accueil.
 *
 * Personnage entièrement fictif : une page publique indexée ne doit pas porter
 * l'email et le téléphone d'une vraie personne (docs/DESIGN.md §12). Le domaine
 * `example.com` et l'indicatif 01 99 00 sont réservés à la fiction — aucun
 * risque d'envoyer du spam à quelqu'un.
 *
 * Illustration non interactive : les boutons sont des <span>, pas de faux liens.
 */
import s from './signature.module.css';

export function SignatureDemo() {
  return (
    <figure className={s.figure}>
      <div className={s.browser} aria-label="Aperçu de l’éditeur Siglair">
        <div className={s.browserBar} aria-hidden="true">
          <span className={s.bubble} />
          <span className={s.bubble} />
          <span className={s.bubble} />
          <span className={s.url}>siglair.com/app/editor/atelier-nord</span>
          <span className={s.live}>Live preview</span>
        </div>

        <div className={s.demoBody}>
          <aside className={s.side} aria-hidden="true">
            <strong>Calques</strong>
            <span className={s.tool}>Logo animé</span>
            <span className={`${s.tool} ${s.toolActive}`}>Bloc identité</span>
            <span className={s.tool}>Boutons CTA</span>
            <span className={s.tool}>Bannière</span>
          </aside>

          <div className={s.editor}>
            <span className={s.result}>Signature générée — prête à modifier</span>
            <span className={s.pill}>Attends… ta signature bouge&nbsp;?</span>

            <div className={s.sig}>
              <div className={s.logoWrap}>
                <img
                  className={s.logo}
                  src="/brand/siglair-mark.png"
                  alt=""
                  width={256}
                  height={256}
                  decoding="async"
                />
              </div>
              <div className={s.rule} aria-hidden="true" />
              <div className={s.body}>
                <p className={s.name}>Camille Roussel</p>
                <p className={s.role}>Directrice associée — Atelier Nord</p>
                <p className={s.tagline}>Studio de design d’interface · Paris</p>
                <p className={s.contact}>camille@example.com · +33 1 99 00 12 34</p>
                <div className={s.buttons}>
                  <span className={`${s.btn} ${s.btnShimmer}`}>Site</span>
                  <span className={`${s.btn} ${s.btnLinked}`}>LinkedIn</span>
                  <span className={`${s.btn} ${s.btnPulse}`}>WhatsApp</span>
                </div>
                <div className={s.accent} />
              </div>
            </div>

            <span className={s.cursor} aria-hidden="true" />
          </div>

          <aside className={s.inspector} aria-hidden="true">
            <strong>Motion</strong>
            <span className={s.motionTag}>Glow</span>
            <span className={s.motionTag}>Draw</span>
            <div className={s.slider} />
            <strong>Compatibilité</strong>
            <span className={s.status}>Gmail · Apple Mail · Outlook web</span>
            <span className={s.statusWarn}>Outlook Windows : image fixe</span>
          </aside>
        </div>

        <div className={s.browserFoot} aria-hidden="true">
          <span>URL hébergée stable</span>
          <span>GIF serveur + première frame</span>
          <span>Analytics par bouton</span>
        </div>
      </div>
      <figcaption className={s.caption}>
        Exemple fictif. La même signature est rendue en GIF par nos serveurs et servie depuis une
        URL stable.
      </figcaption>
    </figure>
  );
}
