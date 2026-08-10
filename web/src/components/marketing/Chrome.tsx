/**
 * Habillage commun aux pages publiques : marque, en-tête collant, pied de page.
 * Rien d'authentifié ici — ces pages doivent s'afficher même si l'API est morte.
 */
import { useEffect, useState } from 'react';
import { Link, useLocation } from 'react-router-dom';
import s from './chrome.module.css';

/** Destination unique du CTA « commencer » : un seul endroit à changer. */
export const START_HREF = `/login?next=${encodeURIComponent('/app')}`;

export function Brand() {
  return (
    <Link to="/" className={s.brand}>
      <img
        className={s.brandMark}
        src="/brand/siglair-mark.png"
        alt=""
        width={36}
        height={36}
        aria-hidden="true"
      />
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
            <Link to="/campagnes-signature-email" className={s.navWide}>
              Campagnes
            </Link>
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
            Des signatures email animées qui deviennent un canal marketing : campagnes, CTA et
            mesure sur une URL stable.
          </p>
        </div>

        <div className={s.footerCol}>
          <h2>Produit</h2>
          <ul>
            <li>
              <Link to="/campagnes-signature-email">Campagnes</Link>
            </li>
            <li>
              <Link to="/signature-email-animee">Signature animée</Link>
            </li>
            <li>
              <Link to="/signature-email-outlook">Signature Outlook</Link>
            </li>
            <li>
              <Link to="/signature-email-gmail">Signature Gmail</Link>
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
        Siglair — signatures email marketing animées et hébergées. Les prix affichés sont ceux
        servis par l’API ; le régime de TVA applicable est précisé au moment du paiement.
      </p>
    </footer>
  );
}

/**
 * Titre et description de l'onglet, par page.
 * index.html porte les balises statiques lues par les robots sociaux ;
 * ici on ne corrige que ce qui dépend de la route.
 */
function upsertMeta(selector: string, attributes: Record<string, string>): HTMLMetaElement {
  const existing = document.querySelector<HTMLMetaElement>(selector);
  const tag = existing ?? document.head.appendChild(document.createElement('meta'));
  Object.entries(attributes).forEach(([name, value]) => tag.setAttribute(name, value));
  return tag;
}

/**
 * Maintient les métadonnées synchronisées lors d'une navigation côté client.
 * Le build pré-rend les mêmes informations pour les robots qui n'exécutent pas JavaScript.
 */
export function usePageMeta(title: string, description: string): void {
  const { pathname } = useLocation();

  useEffect(() => {
    document.title = title;
    const canonicalUrl = new URL(pathname, 'https://siglair.com').toString();
    const robots = pathname.startsWith('/legal/')
      ? 'noindex, nofollow'
      : 'index, follow, max-image-preview:large, max-snippet:-1, max-video-preview:-1';

    upsertMeta('meta[name="description"]', { name: 'description', content: description });
    upsertMeta('meta[name="robots"]', {
      name: 'robots',
      content: robots,
    });
    upsertMeta('meta[property="og:title"]', { property: 'og:title', content: title });
    upsertMeta('meta[property="og:description"]', {
      property: 'og:description',
      content: description,
    });
    upsertMeta('meta[property="og:url"]', { property: 'og:url', content: canonicalUrl });
    upsertMeta('meta[name="twitter:title"]', { name: 'twitter:title', content: title });
    upsertMeta('meta[name="twitter:description"]', {
      name: 'twitter:description',
      content: description,
    });

    const canonical =
      document.querySelector<HTMLLinkElement>('link[rel="canonical"]') ??
      document.head.appendChild(Object.assign(document.createElement('link'), { rel: 'canonical' }));
    canonical.href = canonicalUrl;
  }, [title, description, pathname]);
}
