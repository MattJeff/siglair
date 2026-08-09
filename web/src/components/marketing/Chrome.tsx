/**
 * Habillage commun aux pages publiques : marque, en-tête collant, pied de page.
 * Rien d'authentifié ici — ces pages doivent s'afficher même si l'API est morte.
 */
import { useEffect, useState } from 'react';
import { Link } from 'react-router-dom';
import s from './chrome.module.css';

/** Destination unique du CTA « commencer » : un seul endroit à changer. */
export const START_HREF = `/login?next=${encodeURIComponent('/app')}`;

export function Brand() {
  return (
    <Link to="/" className={s.brand}>
      <span className={s.ring} aria-hidden="true" />
      Siglair
    </Link>
  );
}

export function SiteHeader() {
  const [scrolled, setScrolled] = useState(false);

  useEffect(() => {
    // setState avec la même valeur ne re-rend pas : pas besoin de throttler.
    const onScroll = () => setScrolled(window.scrollY > 4);
    onScroll();
    window.addEventListener('scroll', onScroll, { passive: true });
    return () => window.removeEventListener('scroll', onScroll);
  }, []);

  return (
    <>
      <a className={s.skip} href="#contenu">
        Aller au contenu
      </a>
      <header className={s.header} data-scrolled={scrolled || undefined}>
        <div className={s.inner}>
          <Brand />
          <nav className={s.nav} aria-label="Navigation principale">
            <Link to="/#fonctionnalites" className={s.navWide}>
              Fonctionnalités
            </Link>
            <Link to="/#compatibilite" className={s.navWide}>
              Compatibilité
            </Link>
            <Link to="/pricing" className={s.navSecondary}>
              Tarifs
            </Link>
            <Link to="/login">Connexion</Link>
            <Link to={START_HREF} className={s.cta}>
              <span className={s.ctaLong}>Commencer gratuitement</span>
              <span className={s.ctaShort}>Commencer</span>
            </Link>
          </nav>
        </div>
      </header>
    </>
  );
}

export function SiteFooter() {
  return (
    <footer className={s.footer}>
      <div className={s.footerInner}>
        <div>
          <Brand />
          <p className={s.tagline}>
            Des signatures email animées, hébergées sur une URL stable. Vous collez une fois, vous
            changez quand vous voulez.
          </p>
        </div>

        <div className={s.footerCol}>
          <h2>Produit</h2>
          <ul>
            <li>
              <Link to="/#fonctionnalites">Fonctionnalités</Link>
            </li>
            <li>
              <Link to="/#compatibilite">Compatibilité</Link>
            </li>
            <li>
              <Link to="/pricing">Tarifs</Link>
            </li>
            <li>
              <Link to="/#faq">Questions fréquentes</Link>
            </li>
          </ul>
        </div>

        <div className={s.footerCol}>
          <h2>Compte</h2>
          <ul>
            <li>
              <Link to="/login">Connexion</Link>
            </li>
            <li>
              <Link to={START_HREF}>Créer un compte</Link>
            </li>
          </ul>
        </div>

        <div className={s.footerCol}>
          <h2>Légal</h2>
          <ul>
            <li>
              <Link to="/legal/cgu">Conditions d’utilisation</Link>
            </li>
            <li>
              <Link to="/legal/confidentialite">Confidentialité</Link>
            </li>
            <li>
              <Link to="/legal/mentions">Mentions légales</Link>
            </li>
            <li>
              {/* Pas d'adresse de contact inventée : elle est à renseigner
                  dans les mentions légales, avec les autres informations
                  d'identification de l'éditeur. */}
              <Link to="/legal/mentions#contact">Contact</Link>
            </li>
          </ul>
        </div>
      </div>

      <p className={s.legalLine}>
        Siglair — signatures email animées et hébergées. Les prix affichés sont ceux servis par
        l’API ; le régime de TVA applicable est précisé au moment du paiement.
      </p>
    </footer>
  );
}

/**
 * Titre et description de l'onglet, par page.
 * index.html porte les balises statiques lues par les robots sociaux ;
 * ici on ne corrige que ce qui dépend de la route.
 */
export function usePageMeta(title: string, description: string): void {
  useEffect(() => {
    document.title = title;
    const tag =
      document.querySelector<HTMLMetaElement>('meta[name="description"]') ??
      document.head.appendChild(Object.assign(document.createElement('meta'), { name: 'description' }));
    tag.content = description;
  }, [title, description]);
}
