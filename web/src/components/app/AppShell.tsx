import { Link, NavLink } from 'react-router-dom';
import type { ReactNode } from 'react';
import { logout } from '../../lib/api';
import { useSession } from '../../lib/session';
import s from './shell.module.css';

export interface AppShellProps {
  title: string;
  subtitle?: string;
  /** Boutons alignés à droite du titre. */
  actions?: ReactNode;
  children: ReactNode;
}

const NAV = [
  { to: '/app', label: 'Signatures', end: true },
  { to: '/app/team', label: 'Équipe', end: false },
  { to: '/app/settings', label: 'Réglages', end: false },
];

/**
 * Barre haute commune à tous les écrans connectés.
 * Le menu utilisateur est un <details> natif : ouverture au clavier, Échap, aucun JS à écrire.
 */
export function AppShell({ title, subtitle, actions, children }: AppShellProps) {
  const { user, currentOrg, plan, features } = useSession();
  const initial = (user?.name ?? user?.email ?? '?').trim().charAt(0);

  const onLogout = async () => {
    try {
      await logout();
    } finally {
      // Rechargement complet : plus aucun état de session en mémoire.
      location.assign('/login');
    }
  };

  return (
    <>
      <a className={s.skip} href="#contenu">
        Aller au contenu
      </a>

      <header className={s.bar}>
        <Link className={s.brand} to="/app">
          <img
            className={s.brandMark}
            src="/brand/siglair-mark.png"
            alt=""
            width={26}
            height={26}
            aria-hidden="true"
          />
          Siglair
        </Link>

        <nav className={s.nav} aria-label="Navigation principale">
          {NAV.map((item) => (
            <NavLink key={item.to} to={item.to} end={item.end} className={s.navLink}>
              {item.label}
            </NavLink>
          ))}
          {features?.billing && (
            <NavLink to="/app/billing" className={s.navLink}>
              Abonnement
            </NavLink>
          )}
        </nav>

        <div className={s.right}>
          {currentOrg && (
            <p className={s.org}>
              <span className={s.orgName}>{currentOrg.name}</span>
              {plan && <span className={s.plan}>{plan}</span>}
            </p>
          )}

          <details className={s.menu}>
            <summary className={s.menuButton}>
              <span className={s.avatar} aria-hidden="true">
                {initial}
              </span>
              Mon compte
            </summary>
            <div className={s.menuPanel}>
              <p className={s.menuEmail}>{user?.email}</p>
              <Link className={s.menuItem} to="/app/settings">
                Profil et réglages
              </Link>
              {features?.billing && (
                <Link className={s.menuItem} to="/app/billing">
                  Abonnement et factures
                </Link>
              )}
              <button type="button" className={s.menuItem} onClick={() => void onLogout()}>
                Se déconnecter
              </button>
            </div>
          </details>
        </div>
      </header>

      <main className={s.main} id="contenu">
        <div className={s.head}>
          <div>
            <h1 className={s.title}>{title}</h1>
            {subtitle && <p className={s.subtitle}>{subtitle}</p>}
          </div>
          {actions && <div className={s.headActions}>{actions}</div>}
        </div>
        {children}
      </main>
    </>
  );
}
