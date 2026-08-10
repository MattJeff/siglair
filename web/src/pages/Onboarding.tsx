/**
 * Parcours connecté « colle ton site » — contrat §6bis.
 *
 *   marque reconnue (modifiable) → coordonnées → 3 propositions → choix → ÉDITEUR NORMAL
 *
 * Le dernier maillon n'est pas négociable : l'IA supprime la page blanche, elle ne remplace
 * pas l'éditeur. D'où le rappel « Vous pourrez tout modifier ensuite. » à chaque étape — c'est
 * ce qui lève la crainte de se retrouver enfermé dans un résultat généré.
 */
import { useEffect, useRef, useState } from 'react';
import type { FormEvent } from 'react';
import { Link, useNavigate, useSearchParams } from 'react-router-dom';
import { Button } from '../components/Button';
import { Field } from '../components/Field';
import { Input } from '../components/Input';
import { Spinner } from '../components/Spinner';
import { AppShell } from '../components/app/AppShell';
import { PENDING_NEXT, apiMessage, errorCode } from '../components/app/helpers';
import { BrandHero } from '../components/marketing/BrandHero';
import { BrandRecap } from '../onboarding/BrandRecap';
import { VariantCards } from '../onboarding/VariantCards';
import type { Proposal } from '../onboarding/VariantCards';
import {
  claimOnboardingDraft,
  forgetBrand,
  forgetHandoff,
  generateVariants,
  logoSrc,
  pickVariant,
  readBrand,
  readHandoff,
  rememberBrand,
} from '../onboarding/api';
import type { Brand } from '../onboarding/api';
import { useSession } from '../lib/session';
import type { Profile } from '../lib/types';
import s from '../onboarding/onboarding.module.css';

/** Les seuls champs demandés. Chaque champ ajouté ici coûte des inscriptions. */
const FIELDS = [
  { key: 'name', label: 'Nom', type: 'text', autoComplete: 'name' },
  { key: 'role', label: 'Fonction', type: 'text', autoComplete: 'organization-title' },
  { key: 'email', label: 'Email', type: 'email', autoComplete: 'email' },
  { key: 'phone', label: 'Téléphone', type: 'tel', autoComplete: 'tel' },
  { key: 'linkedin', label: 'LinkedIn', type: 'url', autoComplete: 'url' },
] as const;

type FormKey = (typeof FIELDS)[number]['key'];
type Form = Record<FormKey, string>;

function applyBrandDefaults(form: Form, brand: Brand | null): Form {
  if (!brand) return form;
  return {
    ...form,
    email: form.email || brand.contacts?.email || '',
    phone: form.phone || brand.contacts?.phone || '',
    linkedin: form.linkedin || brand.socials?.linkedin || '',
  };
}

export default function Onboarding() {
  const { user, refresh, features } = useSession();
  const navigate = useNavigate();
  const [params] = useSearchParams();

  const [brand, setBrandState] = useState<Brand | null>(() => readBrand());
  const [form, setForm] = useState<Form>(() =>
    applyBrandDefaults(
      {
        name: user?.name ?? '',
        role: '',
        email: user?.email ?? '',
        phone: '',
        linkedin: '',
      },
      brand,
    ),
  );
  const [step, setStep] = useState<'form' | 'generating' | 'choose'>('form');
  const [proposals, setProposals] = useState<Proposal[]>([]);
  /** Profil réellement envoyé : les aperçus doivent montrer ce qui a été composé. */
  const [sentProfile, setSentProfile] = useState<Profile>({});
  const [picking, setPicking] = useState<number | null>(null);
  const [error, setError] = useState('');
  const [quota, setQuota] = useState(false);
  const [usedFallback, setUsedFallback] = useState(false);
  const handoff = params.get('handoff') || readHandoff();
  const [claiming, setClaiming] = useState(Boolean(handoff));
  const claimStarted = useRef(false);

  useEffect(() => {
    if (!handoff || claimStarted.current) return;
    claimStarted.current = true;
    // Le magic link sait désormais revenir ici directement. L'ancien relais via /app ne doit
    // pas ressurgir lors d'une prochaine visite du tableau de bord.
    try {
      sessionStorage.removeItem(PENDING_NEXT);
    } catch {
      /* stockage indisponible : aucun impact sur la réclamation serveur */
    }
    setClaiming(true);
    setError('');
    void claimOnboardingDraft(handoff)
      .then(async ({ id }) => {
        forgetBrand();
        forgetHandoff();
        await refresh();
        navigate(`/app/editor/${id}`, { replace: true });
      })
      .catch((cause: unknown) => {
        forgetHandoff();
        setError(apiMessage(cause));
        setClaiming(false);
      });
  }, [handoff, navigate, refresh]);

  // La marque édite en place et survit à un rechargement de l'onglet.
  const setBrand = (next: Brand) => {
    setBrandState(next);
    setForm((current) => applyBrandDefaults(current, next));
    rememberBrand(next);
  };

  const generate = async (event: FormEvent) => {
    event.preventDefault();
    if (!brand) return;
    setError('');
    setQuota(false);

    // Le profil ne quitte pas notre infrastructure : le serveur n'envoie que la marque au
    // modèle et injecte les coordonnées ensuite, localement (§6bis.3).
    const profile: Profile = {};
    for (const { key } of FIELDS) {
      const value = form[key].trim();
      if (value) profile[key] = value;
    }
    // Déduits de la marque analysée, pas demandés à l'utilisateur : sans eux les boutons
    // « Site web » des propositions n'auraient aucune cible et disparaîtraient.
    if (brand.site) profile.website = brand.site;
    if (brand.name.trim()) profile.company = brand.name.trim();
    if (brand.tagline.trim()) profile.tagline = brand.tagline.trim();
    if (!profile.email && brand.contacts?.email) profile.email = brand.contacts.email;
    if (!profile.phone && brand.contacts?.phone) profile.phone = brand.contacts.phone;
    if (!profile.linkedin && brand.socials?.linkedin) profile.linkedin = brand.socials.linkedin;
    if (brand.contacts?.whatsapp) profile.whatsapp = brand.contacts.whatsapp;
    setSentProfile(profile);

    setStep('generating');
    try {
      const result = await generateVariants(brand, profile);
      setProposals(result.variants);
      setUsedFallback(result.source === 'fallback');
      setStep('choose');
    } catch (cause) {
      setQuota(errorCode(cause) === 'quota_exceeded');
      setError(apiMessage(cause));
      setStep('form');
    }
  };

  const pick = async (index: number) => {
    const chosen = proposals.find((p) => p.index === index);
    if (!chosen) return;
    setError('');
    setPicking(index);
    try {
      // Le document composé repart tel quel : le serveur le revalide (`Doc::validate`) avant
      // de créer la signature. Sans lui, `pick` enregistrerait un document vide.
      const { id } = await pickVariant(index, chosen.doc, sentProfile, chosen.name);
      forgetBrand();
      await refresh();
      navigate(`/app/editor/${id}`, { replace: true });
    } catch (cause) {
      setError(apiMessage(cause));
      setPicking(null);
    }
  };

  const logo = logoSrc(brand?.logo ?? null);

  if (claiming) {
    return (
      <AppShell
        title="Votre signature est prête"
        subtitle="Nous la rattachons à votre compte avant d’ouvrir le canvas."
      >
        <section className={`${s.card} ${s.waiting}`} aria-live="polite">
          <h2 className={s.cardTitle}>
            <Spinner size={18} /> Ouverture de votre création…
          </h2>
          <p className={s.muted}>
            Logo, couleurs, contenus et animations sont déjà dans le document. Vous arrivez
            directement dans l’éditeur.
          </p>
          <div className={s.bar} aria-hidden="true">
            <span className={s.barFill} />
          </div>
        </section>
      </AppShell>
    );
  }

  return (
    <AppShell
      title="Votre signature, à partir de votre site"
      subtitle="Trois propositions bâties sur votre marque. Vous pourrez tout modifier ensuite."
    >
      <div className={s.stack}>
        {error && (
          <div className={quota ? `${s.notice} ${s.upsell}` : `${s.notice} ${s.alert}`} role="alert">
            <p>
              {quota && <strong>Envie d’une autre direction ? </strong>}
              {error}
            </p>
            {quota && (
              <Link className={s.linkBtn} to="/app/billing">
                Voir les abonnements
              </Link>
            )}
          </div>
        )}

        {/* Arrivée directe sur /onboarding, sans être passé par la vitrine. */}
        {!brand && (
          <section className={s.card}>
            <h2 className={s.cardTitle}>Collez l’adresse de votre site</h2>
            <p className={s.muted}>
              On y récupère votre logo, vos couleurs et votre nom de marque. Vous pourrez tout
              corriger juste après.
            </p>
            <BrandHero compact onBrand={setBrand} />
          </section>
        )}

        {brand && step === 'form' && (
          <>
            <BrandRecap brand={brand} onChange={setBrand} />

            <form className={s.card} onSubmit={(event) => void generate(event)} noValidate>
              <h2 className={s.cardTitle}>Vos coordonnées</h2>
              <p className={s.muted}>
                Ce qui apparaîtra dans la signature. Laissez vide ce que vous ne voulez pas
                afficher : un champ absent ne laisse ni trou ni ligne vide.
              </p>

              <div className={s.fields}>
                {FIELDS.map((f) => (
                  <Field key={f.key} label={f.label}>
                    <Input
                      type={f.type}
                      value={form[f.key]}
                      required={f.key === 'name'}
                      autoComplete={f.autoComplete}
                      placeholder={f.key === 'linkedin' ? 'https://linkedin.com/in/…' : undefined}
                      onChange={(event) =>
                        setForm({ ...form, [f.key]: event.currentTarget.value })
                      }
                    />
                  </Field>
                ))}
              </div>

              <div className={s.actions}>
                <Button type="submit">
                  <span aria-hidden="true">✨</span>{' '}
                  {features?.ai_provider === false
                    ? 'Créer mes trois propositions'
                    : 'Générer avec l’IA'}
                </Button>
                <p className={s.reassure}>Vous pourrez tout modifier ensuite.</p>
              </div>
            </form>
          </>
        )}

        {brand && step === 'generating' && (
          <section className={`${s.card} ${s.waiting}`} aria-live="polite">
            <div className={s.waitBrand}>
              {logo && <img className={s.waitLogo} src={logo} alt="" />}
              <div>
                <p className={s.waitName}>{brand.name || 'Votre marque'}</p>
                <ul className={s.waitColors}>
                  {brand.colors.map((color, index) => (
                    <li key={index} className={s.waitColor} style={{ background: color }} />
                  ))}
                </ul>
              </div>
            </div>

            <h2 className={s.cardTitle}>
              <Spinner size={18} /> Nous dessinons trois directions à partir de votre marque.
            </h2>
            <p className={s.muted}>
              Comptez une quinzaine de secondes : trois maquettes complètes sont composées, avec
              leurs animations, puis rendues par notre moteur. Ne fermez pas cette page.
            </p>
            {/* Indicateur indéterminé : on ne sait pas où en est le modèle, on ne l'invente pas. */}
            <div className={s.bar} aria-hidden="true">
              <span className={s.barFill} />
            </div>
            <p className={s.reassure}>Vous pourrez tout modifier ensuite.</p>
          </section>
        )}

        {brand && step === 'choose' && (
          <section className={s.chooseWrap}>
            <div className={s.chooseHead}>
              <h2 className={s.cardTitle}>Trois directions. Une seule à garder.</h2>
              <p className={s.muted}>
                Chacune est une vraie signature, animée et à sa taille réelle. Choisissez la plus
                proche : l’éditeur fait le reste.
              </p>
              <p className={s.reassure}>Vous pourrez tout modifier ensuite.</p>
              {usedFallback && (
                <p className={s.muted} role="status">
                  Le fournisseur IA n’a pas répondu. Le moteur Siglair a préparé ces directions
                  localement et votre quota IA n’a pas été débité.
                </p>
              )}
            </div>

            <VariantCards
              proposals={proposals}
              profile={sentProfile}
              picking={picking}
              onPick={(index) => void pick(index)}
            />
          </section>
        )}
      </div>
    </AppShell>
  );
}
