import { useEffect } from 'react';
import { useLocation } from 'react-router-dom';
import { analyticsPagePath, getAnalytics } from '../lib/analytics';
import { useSession } from '../lib/session';

/** Mesure la navigation SPA et garde le transport actif sans bloquer le rendu. */
export function AnalyticsLifecycle() {
  const location = useLocation();
  const { status } = useSession();
  const authenticated = status === 'authenticated';

  useEffect(() => {
    const analytics = getAnalytics();
    analytics.start();
    return () => analytics.stop();
  }, []);

  useEffect(() => {
    if (status === 'loading') return;
    const analytics = getAnalytics();
    analytics.track('page_viewed', { page: analyticsPagePath(location.pathname), authenticated });
    if (location.pathname === '/') analytics.track('landing_viewed', {});
  }, [authenticated, location.pathname, status]);

  return null;
}
