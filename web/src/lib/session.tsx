/**
 * Contexte unique de l'application : qui est connecté, quel plan, quelles limites.
 * Charge /api/me et /api/config une fois au démarrage.
 * Trois états explicites — un écran qui suppose l'utilisateur connecté plante au rechargement.
 */
import { createContext, useCallback, useContext, useEffect, useState } from 'react';
import type { ReactNode } from 'react';
import { getConfig, getMe } from './api';
import type { Features, Limits, Me, Org, Plan, PlanInfo, Usage, User } from './types';

export interface Session {
  status: 'loading' | 'authenticated' | 'anonymous';
  user: User | null;
  orgs: Org[];
  currentOrg: Org | null;
  plan: Plan | null;
  limits: Limits | null;
  usage: Usage | null;
  /** null si /api/config n'a pas répondu : masquer les boutons concernés. */
  features: Features | null;
  /** Catalogue tarifaire servi par l'API — ne jamais coder un prix en dur. */
  plans: PlanInfo[];
  /** À appeler après toute écriture qui change l'usage ou le plan. */
  refresh: () => Promise<void>;
}

const SessionContext = createContext<Session | null>(null);

export function SessionProvider({ children }: { children: ReactNode }) {
  const [me, setMe] = useState<Me | null>(null);
  const [features, setFeatures] = useState<Features | null>(null);
  const [plans, setPlans] = useState<PlanInfo[]>([]);
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async () => {
    setMe(await getMe());
  }, []);

  useEffect(() => {
    let alive = true;
    // Aucun des deux appels ne doit pouvoir bloquer le démarrage : API injoignable ou en 500,
    // la landing et /pricing doivent quand même s'afficher. On dégrade en « déconnecté »
    // plutôt que de laisser l'application sur un spinner définitif.
    void Promise.all([getMe().catch(() => null), getConfig().catch(() => null)]).then(([nextMe, config]) => {
      if (!alive) return;
      setMe(nextMe);
      setFeatures(config);
      setPlans(config?.plans ?? []);
      setLoading(false);
    });
    return () => {
      alive = false;
    };
  }, []);

  const orgs = me?.orgs ?? [];
  const value: Session = {
    status: loading ? 'loading' : me ? 'authenticated' : 'anonymous',
    user: me?.user ?? null,
    orgs,
    currentOrg: orgs.find((o) => o.id === me?.current_org) ?? orgs[0] ?? null,
    plan: me?.plan ?? null,
    limits: me?.limits ?? null,
    usage: me?.usage ?? null,
    features,
    plans,
    refresh,
  };

  return <SessionContext.Provider value={value}>{children}</SessionContext.Provider>;
}

export function useSession(): Session {
  const ctx = useContext(SessionContext);
  if (!ctx) throw new Error('useSession doit être utilisé dans <SessionProvider>');
  return ctx;
}
