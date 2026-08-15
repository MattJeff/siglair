/**
 * « Collez votre site » — la première interaction du produit (DESIGN.md §Le héros change,
 * contrat §6bis). Fonctionne sans compte : POST /api/onboarding/analyze.
 *
 * Aucune progression scénarisée. Tant que l'appel court, une seule étape est affichée, et
 * c'est la vraie ; dès qu'il rend la main, la liste dit ce qui a réellement été trouvé —
 * y compris « aucun logo ». Une fausse barre de progression sur le premier geste demandé
 * décrédibilise tout ce que l'interface dira ensuite.
 *
 * Les échecs sont fréquents (site injoignable, domaine interne refusé, pas de logo) : chacun
 * garde une porte de sortie, on ne bloque jamais le visiteur sur cet écran.
 */
import { useId, useRef, useState } from 'react';
import type { CSSProperties, FormEvent } from 'react';
import { useNavigate } from 'react-router-dom';
import { Spinner } from '../Spinner';
import { apiMessage, errorCode } from '../app/helpers';
import {
  LOGIN_THEN_ONBOARDING,
  analyzeBrand,
  createOnboardingDraft,
  logoSrc,
  minimalBrand,
  normalizeUrl,
  rememberBrand,
  rememberHandoff,
} from '../../onboarding/api';
import type { Brand } from '../../onboarding/api';
import { getAnalytics } from '../../lib/analytics';
import s from './BrandHero.module.css';

export interface BrandHeroProps {
  /** Version resserrée pour /onboarding : pas de récapitulatif, le parent prend la suite. */
  compact?: boolean;
  /**
   * Appelé avec la marque retenue (analysée, ou de repli après un échec).
   * Absent : on mémorise la marque et on envoie vers la connexion.
   */
  onBrand?: (brand: Brand) => void;
}

function colorParts(hex: string): [number, number, number] | null {
  const clean = hex.trim().replace(/^#/, '');
  if (!/^[0-9a-f]{6}$/i.test(clean)) return null;
  return [
    Number.parseInt(clean.slice(0, 2), 16),
    Number.parseInt(clean.slice(2, 4), 16),
    Number.parseInt(clean.slice(4, 6), 16),
  ];
}

function luma(hex: string): number {
  const parts = colorParts(hex);
  if (!parts) return 0;
  const [r, g, b] = parts;
  return (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255;
}

function saturation(hex: string): number {
  const parts = colorParts(hex);
  if (!parts) return 0;
  const [r, g, b] = parts.map((v) => v / 255);
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  return max === 0 ? 0 : (max - min) / max;
}

function signaturePalette(colors: string[]) {
  const accent =
    colors.find((color) => luma(color) > 0.18 && luma(color) < 0.86 && saturation(color) > 0.24) ||
    colors.find((color) => luma(color) > 0.18 && luma(color) < 0.86) ||
    '#315cff';
  const accent2 =
    colors.find((color) => color !== accent && luma(color) > 0.22 && saturation(color) > 0.18) ||
    '#3bb8ff';
  const bg = colors.find((color) => luma(color) < 0.18) || '#0b1220';
  return { accent, accent2, bg };
}

function host(site: string): string {
  try {
    return new URL(site).hostname.replace(/^www\./, '');
  } catch {
    return site.replace(/^https?:\/\//, '').replace(/\/.*$/, '');
  }
}

export function BrandHero({ compact = false, onBrand }: BrandHeroProps) {
  const navigate = useNavigate();
  const inputId = useId();
  const [url, setUrl] = useState('');
  const [busy, setBusy] = useState(false);
  const [brand, setBrand] = useState<Brand | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState('');
  const [errorHelp, setErrorHelp] = useState('');
  const [fallback, setFallback] = useState(false);
  const [handoff, setHandoff] = useState<string | null>(null);
  const focusTracked = useRef(false);
  const inputTracked = useRef(false);
  const attempt = useRef(0);

  const prepareDraft = async (found: Brand): Promise<string> => {
    const draft = await createOnboardingDraft(found);
    setHandoff(draft.handoff);
    rememberHandoff(draft.handoff);
    return draft.handoff;
  };

  /** Le document est déjà composé avant la connexion ; seul son jeton traverse le mail. */
  const accept = async (found: Brand) => {
    getAnalytics().track('claim_clicked', { placement: compact ? 'editor' : 'hero' });
    if (onBrand) {
      onBrand(found);
      return;
    }
    setBusy(true);
    try {
      rememberBrand(found);
      if (!handoff) await prepareDraft(found);
      navigate(LOGIN_THEN_ONBOARDING);
    } catch (cause) {
      setError(apiMessage(cause));
      setErrorHelp('La marque est reconnue, mais la signature n’a pas encore pu être préparée. Réessayez.');
    } finally {
      setBusy(false);
    }
  };

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (busy) return;
    const target = normalizeUrl(url);
    if (!target) {
      setBrand(null);
      setError('Entrez l’adresse de votre site, par exemple votreentreprise.com.');
      setErrorHelp('Collez une URL publique, pas une page interne ou un raccourci.');
      return;
    }
    setError('');
    setErrorHelp('');
    setBrand(null);
    setNotice(null);
    setFallback(false);
    setHandoff(null);
    setBusy(true);
    const startedAt = performance.now();
    attempt.current += 1;
    const analytics = getAnalytics();
    analytics.track('url_submitted', {
      placement: compact ? 'editor' : 'hero',
      had_protocol: /^https?:\/\//i.test(url.trim()),
    });
    analytics.track('generation_started', {
      origin: compact ? 'editor' : 'landing',
      attempt: attempt.current,
    });
    try {
      const { brand: found, notice: found_notice } = await analyzeBrand(target);
      const foundContacts = [
        found.contacts?.email,
        found.contacts?.phone,
        found.contacts?.whatsapp,
        found.socials?.linkedin,
      ].filter(Boolean).length;
      analytics.track('generation_completed', {
        origin: compact ? 'editor' : 'landing',
        latency_ms: Math.round(performance.now() - startedAt),
        logo_detected: Boolean(found.logo_asset_id),
        colors_detected: found.colors.length > 0,
        contacts_detected: foundContacts > 0,
      });
      analytics.track('preview_viewed', { origin: compact ? 'editor' : 'landing' });
      setNotice(found_notice);
      setFallback(false);
      if (compact) onBrand?.(found);
      else {
        try {
          await prepareDraft(found);
        } catch (cause) {
          setNotice(
            `${found_notice ? `${found_notice} ` : ''}${apiMessage(cause)} Vous pouvez réessayer avec Continuer.`,
          );
        }
        setBrand(found);
      }
    } catch (cause) {
      const message = apiMessage(cause);
      const code = errorCode(cause);
      analytics.track('generation_failed', {
        origin: compact ? 'editor' : 'landing',
        latency_ms: Math.round(performance.now() - startedAt),
        error_code: code || 'unknown',
        recoverable: code !== 'validation',
      });
      // URL invalide ou domaine privé : on corrige, on ne fabrique pas une marque depuis un
      // intranet. Les pannes du service ou le rate limit ne doivent pas ressembler à un site
      // vide : on affiche l'état réel et on garde une sortie manuelle.
      if (
        code === 'validation' &&
        (message.includes("URL valide") ||
          message.includes("adresse de votre site") ||
          message.includes("domaine est refusé"))
      ) {
        setError(message);
        setErrorHelp(
          "Corrigez l’adresse ou utilisez l’URL publique de votre site. L’analyse ne lit jamais les pages internes ou privées.",
        );
        return;
      }
      if (code === 'internal' || code === 'rate_limited') {
        setError(
          code === 'rate_limited'
            ? message
            : "Le service d'analyse ne répond pas pour l'instant. Réessayez dans un moment ou continuez sans analyse automatique.",
        );
        setErrorHelp(
          "L’adresse peut être correcte : c’est l’analyse automatique qui n’a pas rendu de résultat.",
        );
        return;
      }

      const fallbackBrand = minimalBrand(target);
      setFallback(true);
      setNotice(
        "L'analyse automatique n'a pas pu récupérer ce site. On part quand même de son nom de domaine : vous pourrez ajouter le logo, les couleurs et les liens dans l'éditeur.",
      );
      if (compact) onBrand?.(fallbackBrand);
      else {
        try {
          await prepareDraft(fallbackBrand);
        } catch {
          // Le récapitulatif reste utile et le bouton Continuer pourra réessayer la préparation.
        }
        setBrand(fallbackBrand);
      }
    } finally {
      setBusy(false);
    }
  };

  // `logo_asset_id` est la seule vérité : un fichier trouvé mais illisible (SVG, HTML servi
  // à la place de l'image) n'est pas enregistré, et ne sera donc pas dans la signature.
  const logo = brand?.logo_asset_id ? logoSrc(brand.logo) : null;
  const colors = brand?.colors ?? [];
  const contactCount = brand
    ? [
        brand.contacts?.email,
        brand.contacts?.phone,
        brand.contacts?.whatsapp,
        brand.socials?.linkedin,
      ].filter(Boolean).length
    : 0;

  return (
    <div className={compact ? `${s.hero} ${s.compact}` : s.hero}>
      <form className={s.form} onSubmit={(event) => void submit(event)} noValidate>
        <label className={s.label} htmlFor={inputId}>
          L’adresse de votre site
        </label>
        <div className={s.bar}>
          <input
            id={inputId}
            className={s.input}
            type="url"
            inputMode="url"
            autoComplete="url"
            spellCheck={false}
            placeholder="https://votreentreprise.com"
            value={url}
            onFocus={() => {
              if (focusTracked.current) return;
              focusTracked.current = true;
              getAnalytics().track('url_input_focused', {
                placement: compact ? 'editor' : 'hero',
              });
            }}
            onChange={(event) => {
              const value = event.currentTarget.value;
              setUrl(value);
              if (!inputTracked.current && value.trim()) {
                inputTracked.current = true;
                getAnalytics().track('url_entered', {
                  placement: compact ? 'editor' : 'hero',
                  input_method: 'unknown',
                });
              }
            }}
            aria-describedby={`${inputId}-sub`}
            aria-invalid={error ? true : undefined}
          />
          <button className={s.go} type="submit" disabled={busy}>
            {busy ? <Spinner size={16} /> : <span aria-hidden="true">✨</span>}
            Générer
          </button>
        </div>
        <p className={s.sub} id={`${inputId}-sub`}>
          L’IA la dessine. Vous gardez la main sur tout.
        </p>
      </form>

      {/* Une seule étape pendant l'appel : c'est la seule qui soit vraie. */}
      {busy && (
        <p className={s.working} role="status">
          <Spinner size={16} />
          Analyse de votre marque…
        </p>
      )}

      {error && !busy && !brand && (
        <div className={`${s.panel} ${s.panelError}`} role="alert">
          <p className={s.errorText}>{error}</p>
          {errorHelp && <p className={s.escapeText}>{errorHelp}</p>}
          <button
            className={s.escape}
            type="button"
            onClick={() => void accept(minimalBrand(normalizeUrl(url)))}
          >
            Continuer sans mon site →
          </button>
        </div>
      )}

      {brand && !busy && (
        <div className={`${s.panel} ${fallback ? s.panelFallback : ''}`}>
          <div className={s.panelBody}>
            <div className={s.panelCopy}>
              <ul className={s.found}>
                <li className={fallback ? s.miss : s.ok}>
                  {fallback ? 'Départ proposé :' : 'Marque reconnue :'}{' '}
                  <strong>{brand.name || 'votre marque'}</strong>
                </li>
                <li className={logo ? s.ok : s.miss}>
                  {logo ? 'Logo récupéré' : 'Aucun logo trouvé'}
                </li>
                <li className={colors.length > 0 ? s.ok : s.miss}>
                  {colors.length > 0
                    ? `${colors.length} couleur${colors.length > 1 ? 's' : ''} détectée${colors.length > 1 ? 's' : ''}`
                    : 'Aucune couleur détectée'}
                </li>
                <li className={contactCount > 0 ? s.ok : s.miss}>
                  {contactCount > 0
                    ? `${contactCount} contact${contactCount > 1 ? 's' : ''} récupéré${contactCount > 1 ? 's' : ''}`
                    : 'Aucun contact public détecté'}
                </li>
              </ul>

              <div className={s.preview}>
                {logo && (
                  <img className={s.logo} src={logo} alt={`Logo de ${brand.name}`} loading="lazy" />
                )}
                {colors.length > 0 && (
                  <ul className={s.swatches} aria-label="Couleurs détectées">
                    {colors.map((color) => (
                      <li
                        key={color}
                        className={s.swatch}
                        style={{ background: color }}
                        title={color}
                      >
                        <span className={s.srOnly}>{color}</span>
                      </li>
                    ))}
                  </ul>
                )}
              </div>

              {brand.tagline && <p className={s.tagline}>{brand.tagline}</p>}
              {notice && <p className={s.escapeText}>{notice}</p>}

              <div className={s.panelActions}>
                <button className={s.continue} type="button" onClick={() => void accept(brand)}>
                  Ouvrir ma signature →
                </button>
                <p className={s.escapeText}>
                  {logo
                    ? 'Votre signature est déjà préparée. Après connexion, elle s’ouvrira directement dans l’éditeur.'
                    : 'La composition est déjà préparée. Après connexion, ajoutez votre logo directement dans l’éditeur.'}
                </p>
              </div>
            </div>

            <SignaturePreview brand={brand} logo={logo} colors={colors} />
          </div>
        </div>
      )}
    </div>
  );
}

function SignaturePreview({
  brand,
  logo,
  colors,
}: {
  brand: Brand;
  logo: string | null;
  colors: string[];
}) {
  const palette = signaturePalette(colors);
  const style = {
    '--sig-accent': palette.accent,
    '--sig-accent-2': palette.accent2,
    '--sig-bg': palette.bg,
  } as CSSProperties;
  const brandName = brand.name || 'Votre marque';
  const site = host(brand.site);
  const contact = brand.contacts?.email || brand.contacts?.phone || site;
  const initials = brandName
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((part) => part[0])
    .join('')
    .toUpperCase();

  return (
    <aside className={s.signaturePreview} style={style} aria-label="Signature générée">
      <div className={s.signatureCard}>
        <div className={s.signatureTop}>
          <div className={s.signatureLogo}>
            {logo ? <img src={logo} alt="" loading="lazy" /> : <span>{initials || 'S'}</span>}
          </div>
          <div className={s.signatureRule} aria-hidden="true" />
          <div className={s.signatureIdentity}>
            {/* Le nom et le poste ne sont pas analysables depuis un site : un « Camille Martin »
                en dur donnait « Camille Martin — CEO — SonEntreprise » au visiteur, à côté de son
                vrai logo. Champ à remplir, pas donnée récupérée. */}
            <strong className={s.toFill}>Votre nom</strong>
            <span className={s.toFill}>Votre poste — {brandName}</span>
            {brand.tagline && <small>{brand.tagline}</small>}
            <em>{contact}</em>
          </div>
        </div>

        <div className={s.signatureButtons}>
          {site && <span>Site web</span>}
          {brand.socials?.linkedin && <span>LinkedIn</span>}
          <span>Book a call</span>
        </div>

        <span className={s.signatureAccent} aria-hidden="true" />
      </div>
    </aside>
  );
}
