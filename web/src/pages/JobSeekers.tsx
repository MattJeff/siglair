import { useEffect, useMemo, useRef, useState } from 'react';
import type { FormEvent } from 'react';
import {
  BarChart3,
  BriefcaseBusiness,
  Check,
  Code2,
  ContactRound,
  FileUp,
  LoaderCircle,
  Sparkles,
} from 'lucide-react';
import { Link, useNavigate } from 'react-router-dom';
import { SiteFooter, SiteHeader, usePageMeta } from '../components/marketing/Chrome';
import { TEMPLATES, docFromTemplate } from '../editor/templates';
import type { Template } from '../editor/templates';
import { apiMessage } from '../components/app/helpers';
import { createJobSeekerDraft, previewJobSeekerDoc } from '../lib/api';
import { getAnalytics } from '../lib/analytics';
import type { Doc, Profile } from '../lib/types';
import { LOGIN_THEN_ONBOARDING, rememberHandoff } from '../onboarding/api';
import s from './job-seekers.module.css';

const META_TITLE = "Signature email pour recherche d'emploi | Siglair";
const META_DESCRIPTION =
  'Créez une signature de candidature avec CV, LinkedIn, portfolio ou GitHub. Aperçu réel, PDF facultatif, édition complète et installation Gmail ou Outlook.';
const FRAME_RESET =
  '<style>html,body{margin:0!important;padding:0!important;overflow:hidden!important;background:transparent!important}</style>';
const BRANDING_HEIGHT = 34;

const STYLE_IDS = ['recruiter-ready', 'developer-proof', 'portfolio-motion'] as const;
const STYLES = STYLE_IDS.map((id) => TEMPLATES.find((template) => template.id === id)).filter(
  (template): template is Template => Boolean(template),
);

type Category = 'general' | 'developer' | 'designer' | 'marketing' | 'student';

type Values = {
  name: string;
  role: string;
  email: string;
  category: Category;
  linkedin: string;
  portfolio: string;
  github: string;
  cv: string;
  availability: string;
  location: string;
  tagline: string;
  university: string;
  graduation: string;
};

const EMPTY: Values = {
  name: '',
  role: '',
  email: '',
  category: 'general',
  linkedin: '',
  portfolio: '',
  github: '',
  cv: '',
  availability: '',
  location: '',
  tagline: '',
  university: '',
  graduation: '',
};

const DEMO: Profile = {
  name: 'Lucas Martin',
  role: 'Junior Product Designer',
  email: 'lucas@example.com',
  linkedin: 'https://linkedin.com',
  portfolio: 'https://example.com',
  github: 'https://github.com',
  cv: 'https://example.com/cv.pdf',
  availability: 'Disponible dès septembre',
  location: 'Paris · hybride',
  tagline: 'Interfaces produit, prototypage et design systems.',
  university: 'École de design',
  graduation: 'Master · Promotion 2026',
};

function normalizeUrl(value: string): string {
  const trimmed = value.trim();
  if (!trimmed) return '';
  try {
    return new URL(/^https?:\/\//i.test(trimmed) ? trimmed : `https://${trimmed}`).toString();
  } catch {
    return trimmed;
  }
}

function profileFrom(values: Values, preview = false, hasCvFile = false): Profile {
  const profile: Profile = {};
  const assign = (key: keyof Profile, value: string) => {
    const clean = value.trim();
    if (clean) profile[key] = clean;
  };
  assign('name', values.name);
  assign('role', values.role);
  assign('email', values.email);
  assign('linkedin', normalizeUrl(values.linkedin));
  assign('portfolio', normalizeUrl(values.portfolio));
  assign('github', normalizeUrl(values.github));
  assign('cv', normalizeUrl(values.cv));
  assign('availability', values.availability);
  assign('location', values.location);
  assign('tagline', values.tagline);
  assign('university', values.university);
  assign('graduation', values.graduation);
  if (preview && hasCvFile && !profile.cv) profile.cv = DEMO.cv;
  if (!preview) return profile;
  return { ...DEMO, ...profile };
}

function SignatureFrame({ doc, profile }: { doc: Doc; profile: Profile }) {
  const host = useRef<HTMLDivElement>(null);
  const previewReported = useRef(false);
  const [html, setHtml] = useState('');
  const [scale, setScale] = useState(1);

  useEffect(() => {
    let alive = true;
    const timer = window.setTimeout(() => {
      void previewJobSeekerDoc(doc, profile)
        .then((result) => {
          if (alive) {
            setHtml(result.html);
            if (!previewReported.current) {
              previewReported.current = true;
              getAnalytics().track('preview_viewed', { origin: 'job_seeker' });
            }
          }
        })
        .catch(() => {
          if (alive) setHtml('');
        });
    }, 180);
    return () => {
      alive = false;
      window.clearTimeout(timer);
    };
  }, [doc, profile]);

  useEffect(() => {
    const node = host.current;
    if (!node) return;
    const update = () => setScale(Math.min(1, node.clientWidth / doc.canvas.width));
    update();
    const observer = new ResizeObserver(update);
    observer.observe(node);
    return () => observer.disconnect();
  }, [doc.canvas.width]);

  return (
    <div
      className={s.previewHost}
      ref={host}
      style={{ height: (doc.canvas.height + BRANDING_HEIGHT) * scale }}
    >
      <div
        className={s.previewScale}
        style={{
          width: doc.canvas.width,
          height: doc.canvas.height + BRANDING_HEIGHT,
          transform: `scale(${scale})`,
        }}
      >
        {html ? (
          <iframe
            title="Aperçu réel de la signature de candidature"
            sandbox=""
            width={doc.canvas.width}
            height={doc.canvas.height + BRANDING_HEIGHT}
            srcDoc={`${FRAME_RESET}${html}`}
            data-motion="keep"
          />
        ) : (
          <span className={s.previewLoading}>
            <LoaderCircle size={18} aria-hidden="true" /> Préparation de l’aperçu
          </span>
        )}
      </div>
    </div>
  );
}

export default function JobSeekers() {
  usePageMeta(META_TITLE, META_DESCRIPTION);
  const navigate = useNavigate();
  const [values, setValues] = useState<Values>(EMPTY);
  const [styleId, setStyleId] = useState<(typeof STYLE_IDS)[number]>('recruiter-ready');
  const [cvFile, setCvFile] = useState<File | null>(null);
  const [advanced, setAdvanced] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const generationAttempt = useRef(0);

  const template = STYLES.find((item) => item.id === styleId) ?? STYLES[0];
  const doc = useMemo(() => docFromTemplate(template), [template]);
  const previewProfile = useMemo(
    () => profileFrom(values, true, Boolean(cvFile)),
    [values, cvFile],
  );

  const change = (key: keyof Values, value: string) =>
    setValues((current) => ({ ...current, [key]: value }));

  const changeCategory = (category: Category) => {
    setValues((current) => ({ ...current, category }));
    setStyleId(
      category === 'developer'
        ? 'developer-proof'
        : category === 'designer' || category === 'marketing'
          ? 'portfolio-motion'
          : 'recruiter-ready',
    );
  };

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (busy) return;
    setError('');
    if (!values.name.trim() || !values.role.trim() || !values.email.trim()) {
      setError('Renseignez votre nom, le métier recherché et votre email.');
      return;
    }
    if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(values.email.trim())) {
      setError('Renseignez une adresse email valide.');
      return;
    }
    if (cvFile && (cvFile.type !== 'application/pdf' || cvFile.size > 5 * 1024 * 1024)) {
      setError('Le CV doit être un fichier PDF de 5 Mo maximum.');
      return;
    }

    const profile = profileFrom(values);
    const startedAt = performance.now();
    setBusy(true);
    generationAttempt.current += 1;
    getAnalytics().track('generation_started', {
      origin: 'job_seeker',
      attempt: generationAttempt.current,
    });
    getAnalytics().track('claim_clicked', { placement: 'job_seeker' });
    try {
      const draft = await createJobSeekerDraft(
        doc,
        profile,
        `Candidature - ${values.role.trim()}`,
        cvFile,
      );
      rememberHandoff(draft.handoff);
      getAnalytics().track('generation_completed', {
        origin: 'job_seeker',
        latency_ms: Math.round(performance.now() - startedAt),
        logo_detected: false,
        colors_detected: false,
        contacts_detected: Boolean(profile.email || profile.linkedin),
      });
      navigate(LOGIN_THEN_ONBOARDING);
    } catch (cause) {
      getAnalytics().track('generation_failed', {
        origin: 'job_seeker',
        latency_ms: Math.round(performance.now() - startedAt),
        error_code: 'job_draft_failed',
        recoverable: true,
      });
      setError(apiMessage(cause));
      setBusy(false);
    }
  };

  return (
    <div className={s.page}>
      <SiteHeader />
      <main id="contenu">
        <section className={s.intro}>
          <p className={s.eyebrow}>Siglair for Job Seekers</p>
          <h1>Chaque candidature mérite une fin d’email professionnelle.</h1>
          <p className={s.lead}>
            Réunissez CV, LinkedIn, portfolio ou GitHub dans une signature conçue pour le métier
            que vous cherchez. Sans promettre une réponse, elle donne au recruteur un prochain
            geste clair.
          </p>
          <ul className={s.proofLine}>
            <li><Check size={15} /> Première signature gratuite</li>
            <li><Check size={15} /> Aucun paiement demandé</li>
            <li><Check size={15} /> Gmail et Outlook</li>
          </ul>
        </section>

        <section className={s.builder} aria-label="Générateur de signature pour candidature">
          <form className={s.form} onSubmit={(event) => void submit(event)} noValidate>
            <div className={s.formHead}>
              <span>01</span>
              <div>
                <h2>Votre profil candidat</h2>
                <p>Seuls les champs renseignés apparaîtront.</p>
              </div>
            </div>

            <div className={s.fieldGrid}>
              <label>
                <span>Nom complet</span>
                <input value={values.name} required autoComplete="name" placeholder="Lucas Martin" onChange={(e) => change('name', e.currentTarget.value)} />
              </label>
              <label>
                <span>Métier recherché</span>
                <input value={values.role} required placeholder="Junior Product Designer" onChange={(e) => change('role', e.currentTarget.value)} />
              </label>
              <label>
                <span>Email affiché</span>
                <input type="email" value={values.email} required autoComplete="email" placeholder="lucas@email.com" onChange={(e) => change('email', e.currentTarget.value)} />
              </label>
              <label>
                <span>Profil</span>
                <select
                  value={values.category}
                  onChange={(e) => changeCategory(e.currentTarget.value as Category)}
                >
                  <option value="general">Généraliste</option>
                  <option value="developer">Développement</option>
                  <option value="designer">Design</option>
                  <option value="marketing">Marketing</option>
                  <option value="student">Étudiant</option>
                </select>
              </label>
              <label>
                <span><ContactRound size={14} /> LinkedIn</span>
                <input type="url" value={values.linkedin} placeholder="linkedin.com/in/…" onChange={(e) => change('linkedin', e.currentTarget.value)} />
              </label>
              <label>
                <span>Portfolio</span>
                <input type="url" value={values.portfolio} placeholder="portfolio.fr" onChange={(e) => change('portfolio', e.currentTarget.value)} />
              </label>
            </div>

            <button className={s.more} type="button" aria-expanded={advanced} onClick={() => setAdvanced((value) => !value)}>
              {advanced ? 'Masquer les détails' : 'Ajouter CV, GitHub et disponibilité'}
            </button>

            {advanced && (
              <div className={s.fieldGrid}>
                <label>
                  <span><Code2 size={14} /> GitHub</span>
                  <input type="url" value={values.github} placeholder="github.com/…" onChange={(e) => change('github', e.currentTarget.value)} />
                </label>
                <label>
                  <span>Disponibilité</span>
                  <input value={values.availability} placeholder="Disponible dès septembre" onChange={(e) => change('availability', e.currentTarget.value)} />
                </label>
                <label>
                  <span>Localisation</span>
                  <input value={values.location} placeholder="Paris · hybride" onChange={(e) => change('location', e.currentTarget.value)} />
                </label>
                <label>
                  <span>Spécialités</span>
                  <input value={values.tagline} placeholder="React, TypeScript, design systems" onChange={(e) => change('tagline', e.currentTarget.value)} />
                </label>
                <label>
                  <span>École ou université</span>
                  <input value={values.university} placeholder="Université Paris-Saclay" onChange={(e) => change('university', e.currentTarget.value)} />
                </label>
                <label>
                  <span>Diplôme et promotion</span>
                  <input value={values.graduation} placeholder="Master · Promotion 2026" onChange={(e) => change('graduation', e.currentTarget.value)} />
                </label>
                <label>
                  <span>Lien vers le CV</span>
                  <input type="url" value={values.cv} disabled={Boolean(cvFile)} placeholder="https://…/cv.pdf" onChange={(e) => change('cv', e.currentTarget.value)} />
                </label>
                <label className={s.fileField}>
                  <span><FileUp size={14} /> Ou importer le PDF</span>
                  <input
                    type="file"
                    accept="application/pdf"
                    onChange={(e) => {
                      const file = e.currentTarget.files?.[0] ?? null;
                      setCvFile(file);
                      if (file) change('cv', '');
                    }}
                  />
                  <em>{cvFile ? cvFile.name : 'PDF · 5 Mo maximum'}</em>
                </label>
              </div>
            )}

            {error && <p className={s.error} role="alert">{error}</p>}
            <button className={s.submit} type="submit" disabled={busy}>
              {busy ? <LoaderCircle className={s.spin} size={18} /> : <Sparkles size={18} />}
              {busy ? 'Préparation de votre signature…' : 'Créer cette signature gratuitement'}
            </button>
            <p className={s.privacy}>
              Un PDF importé devient un lien public de la signature. Sans création de compte, le
              brouillon et le fichier expirent après 24 heures.
            </p>
          </form>

          <div className={s.previewPanel}>
            <div className={s.previewHead}>
              <span>02</span>
              <div>
                <h2>Aperçu réel</h2>
                <p>Le même moteur que la signature publiée.</p>
              </div>
            </div>
            <div className={s.styleSwitch} aria-label="Style de signature">
              {STYLES.map((item) => (
                <button
                  type="button"
                  key={item.id}
                  aria-pressed={styleId === item.id}
                  onClick={() => {
                    setStyleId(item.id as (typeof STYLE_IDS)[number]);
                    getAnalytics().track('template_selected', {
                      origin: 'job_seeker',
                      template_id: item.id,
                    });
                  }}
                >
                  <strong>{item.name}</strong>
                  <span>{item.tag}</span>
                </button>
              ))}
            </div>
            <div className={s.mail}>
              <div className={s.mailTop}>
                <span>L</span>
                <div><strong>Lucas Martin</strong><small>à un recruteur · maintenant</small></div>
              </div>
              <p>Bonjour,</p>
              <p>Je vous adresse ma candidature. Vous trouverez mes liens juste ci-dessous.</p>
              <SignatureFrame doc={doc} profile={previewProfile} />
            </div>
            <p className={s.previewNote}>Votre résultat s’ouvre ensuite dans le canvas complet.</p>
          </div>
        </section>

        <section className={s.benefits}>
          <header>
            <p className={s.eyebrow}>Pensée pour l’objectif</p>
            <h2>La bonne preuve, pour le bon métier.</h2>
          </header>
          <div className={s.benefitGrid}>
            <article><BriefcaseBusiness /><h3>Étudiant ou jeune diplômé</h3><p>Université, diplôme, disponibilité, LinkedIn et CV dans une hiérarchie sobre.</p></article>
            <article><Code2 /><h3>Développeur</h3><p>GitHub, stack et projets deviennent les actions principales, sans noyer le contact.</p></article>
            <article><Sparkles /><h3>Designer ou créatif</h3><p>Le portfolio passe en premier, avec un rendu lisible même si l’animation reste figée.</p></article>
          </div>
        </section>

        <section className={s.measureBand}>
          <div>
            <BarChart3 size={28} />
            <p className={s.eyebrow}>Après l’envoi</p>
            <h2>Mesurez ce que les recruteurs ouvrent vraiment.</h2>
          </div>
          <p>
            Avec Pro, Siglair distingue les clics vers votre CV, LinkedIn, GitHub et votre
            portfolio. Les ouvertures d’image restent indicatives; les clics sont le signal utile.
          </p>
          <Link to="/pricing">Voir les analytics Pro</Link>
        </section>

        <section className={s.faq}>
          <p className={s.eyebrow}>Questions fréquentes</p>
          <h2>Avant de commencer à candidater.</h2>
          <details open><summary>Cette signature augmente-t-elle mon taux de réponse ?</summary><p>Siglair ne promet pas un taux de réponse. La signature améliore la présentation et rend vos preuves plus accessibles; la décision reste celle du recruteur.</p></details>
          <details><summary>Mon CV devient-il public ?</summary><p>Oui si vous l’importez ou fournissez un lien : le bouton de la signature doit être accessible au recruteur. Utilisez uniquement une version destinée à être partagée.</p></details>
          <details><summary>Puis-je tout modifier après la création ?</summary><p>Oui. La signature rejoint l’éditeur Siglair avec ses textes, couleurs, dimensions, boutons et animations modifiables.</p></details>
          <details><summary>Le plan gratuit suffit-il pour candidater ?</summary><p>Le plan Free permet de créer et héberger une signature avec la mention Powered by siglair.com. Le suivi détaillé des clics et le retrait de la mention sont inclus dans Pro.</p></details>
        </section>
      </main>
      <SiteFooter />
    </div>
  );
}
