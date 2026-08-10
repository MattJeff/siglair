import { Link, NavLink } from 'react-router-dom';
import type { ReactNode } from 'react';
import {
  ChevronDown,
  CreditCard,
  LayoutDashboard,
  LogOut,
  Settings2,
  Sparkles,
  Users,
} from 'lucide-react';
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
  { to: '/app', label: 'Signatures', end: true, icon: LayoutDashboard },
  { to: '/onboarding', label: 'Créer', end: false, icon: Sparkles },
  { to: '/app/team', label: 'Équipe', end: false, icon: Users },
  { to: '/app/settings', label: 'Réglages', end: false, icon: Settings2 },
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
          {NAV.map((item) => {
            const Icon = item.icon;
            return (
              <NavLink key={item.to} to={item.to} end={item.end} className={s.navLink}>
                <Icon size={16} aria-hidden="true" />
                {item.label}
              </NavLink>
            );
          })}
          {features?.billing && (
            <NavLink to="/app/billing" className={s.navLink}>
              <CreditCard size={16} aria-hidden="true" />
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
              <span className={s.accountLabel}>Mon compte</span>
              <ChevronDown size={14} aria-hidden="true" />
            </summary>
            <div className={s.menuPanel}>
              <p className={s.menuEmail}>{user?.email}</p>
              <Link className={s.menuItem} to="/app/settings">
                <Settings2 size={16} aria-hidden="true" />
                Profil et réglages
              </Link>
              {features?.billing && (
                <Link className={s.menuItem} to="/app/billing">
                  <CreditCard size={16} aria-hidden="true" />
                  Abonnement et factures
                </Link>
              )}
              <button type="button" className={s.menuItem} onClick={() => void onLogout()}>
                <LogOut size={16} aria-hidden="true" />
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
