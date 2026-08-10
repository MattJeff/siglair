import { Suspense } from 'react';
import { Link, Navigate, Outlet, Route, Routes, useLocation } from 'react-router-dom';
import { AppErrorBoundary } from './components/AppErrorBoundary';
import { AnalyticsLifecycle } from './components/AnalyticsLifecycle';
import { Spinner } from './components/Spinner';
import { lazyPage } from './lib/lazyPage';
import { useSession } from './lib/session';

/*
 * Chaque page exporte un composant PAR DÉFAUT (contrainte de React.lazy).
 * Le chargement paresseux évite d'embarquer l'éditeur dans le bundle de la landing.
 */
const Landing = lazyPage(() => import('./pages/Landing'));
const Pricing = lazyPage(() => import('./pages/Pricing'));
const SeoPages = lazyPage(() => import('./pages/SeoPages'));
const Legal = lazyPage(() => import('./pages/Legal'));
const Login = lazyPage(() => import('./pages/Login'));
const Invite = lazyPage(() => import('./pages/Invite'));
const Onboarding = lazyPage(() => import('./pages/Onboarding'));
const Dashboard = lazyPage(() => import('./pages/Dashboard'));
const Editor = lazyPage(() => import('./pages/Editor'));
const Analytics = lazyPage(() => import('./pages/Analytics'));
const Growth = lazyPage(() => import('./pages/Growth'));
const Team = lazyPage(() => import('./pages/Team'));
const Billing = lazyPage(() => import('./pages/Billing'));
const Settings = lazyPage(() => import('./pages/Settings'));

function PageLoader() {
  return (
    <div style={{ display: 'grid', placeItems: 'center', minHeight: '60dvh' }}>
      <Spinner size={28} label="Chargement" />
    </div>
  );
}

/** Attend la fin du chargement de session avant de décider : sinon /app clignote au F5. */
function RequireAuth() {
  const { status } = useSession();
  const location = useLocation();

  if (status === 'loading') return <PageLoader />;
  if (status === 'anonymous') {
    const next = encodeURIComponent(location.pathname + location.search);
    return <Navigate to={`/login?next=${next}`} replace />;
  }
  return <Outlet />;
}

function NotFound() {
  return (
    <main style={{ maxWidth: 640, margin: '0 auto', padding: '96px 20px' }}>
      <h1>Page introuvable</h1>
      <p>
        Ce lien ne mène nulle part. <Link to="/">Revenir à l’accueil</Link>
      </p>
    </main>
  );
}

export default function App() {
  return (
    <AppErrorBoundary>
      <Suspense fallback={<PageLoader />}>
        <AnalyticsLifecycle />
        <Routes>
          <Route path="/" element={<Landing />} />
          <Route path="/pricing" element={<Pricing />} />
          <Route path="/campagnes-signature-email" element={<SeoPages />} />
          <Route path="/generateur-signature-email" element={<SeoPages />} />
          <Route path="/gestion-signatures-email-entreprise" element={<SeoPages />} />
          <Route path="/signature-email-animee" element={<SeoPages />} />
          <Route path="/signature-email-outlook" element={<SeoPages />} />
          <Route path="/signature-email-gmail" element={<SeoPages />} />
          <Route path="/login" element={<Login />} />
          <Route path="/invite/:token" element={<Invite />} />
          {/* Legal gère ses sous-pages (CGU, confidentialité, mentions) via useParams()['*']. */}
          <Route path="/legal/*" element={<Legal />} />

          <Route element={<RequireAuth />}>
            {/* §6bis.6 : `analyze` est publique, `generate` exige un compte. L'écran est donc
              derrière la session — la marque analysée avant l'inscription le traverse par
              sessionStorage (`rememberBrand`). Hors /app : c'est l'accueil d'un nouveau
              compte, pas un onglet du tableau de bord. */}
            <Route path="/onboarding" element={<Onboarding />} />
            <Route path="/app" element={<Dashboard />} />
            <Route path="/app/editor/:id" element={<Editor />} />
            <Route path="/app/analytics/:id" element={<Analytics />} />
            <Route path="/app/growth" element={<Growth />} />
            <Route path="/app/team" element={<Team />} />
            <Route path="/app/billing" element={<Billing />} />
            <Route path="/app/settings" element={<Settings />} />
          </Route>

          <Route path="*" element={<NotFound />} />
        </Routes>
      </Suspense>
    </AppErrorBoundary>
  );
}
