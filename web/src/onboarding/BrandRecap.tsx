/**
 * Récapitulatif de la marque reconnue, modifiable (contrat §6bis, étape 1).
 *
 * Ce qui est corrigé ici part dans `POST /api/onboarding/generate` : l'extraction est
 * déterministe, donc parfois à côté (un favicon pris pour un logo, un gris de fond pris pour
 * une couleur de marque). Laisser l'utilisateur corriger coûte trois contrôles et évite trois
 * propositions bâties sur une mauvaise base.
 */
import { useId, useState } from 'react';
import { Field } from '../components/Field';
import { Input } from '../components/Input';
import { MAX_LOGO_BYTES, logoSrc, readLogoFile } from './api';
import type { Brand } from './api';
import s from './onboarding.module.css';

interface BrandRecapProps {
  brand: Brand;
  onChange: (brand: Brand) => void;
}

export function BrandRecap({ brand, onChange }: BrandRecapProps) {
  const fileId = useId();
  const [error, setError] = useState('');
  const logo = logoSrc(brand.logo);
  const contacts = [
    { label: 'Email', value: brand.contacts?.email },
    { label: 'Téléphone', value: brand.contacts?.phone },
    { label: 'WhatsApp', value: brand.contacts?.whatsapp },
    { label: 'LinkedIn', value: brand.socials?.linkedin },
  ].filter((item): item is { label: string; value: string } => Boolean(item.value));

  const setColor = (index: number, value: string) => {
    const colors = brand.colors.map((c, i) => (i === index ? value : c));
    onChange({ ...brand, colors });
  };

  const removeColor = (index: number) => {
    onChange({ ...brand, colors: brand.colors.filter((_, i) => i !== index) });
  };

  const pickLogo = async (file: File | undefined) => {
    if (!file) return;
    setError('');
    try {
      // Le nouveau logo remplace l'ancien : l'`assetId` sera (re)créé par le serveur.
      onChange({ ...brand, logo: await readLogoFile(file), logo_asset_id: null });
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : 'Cette image n’a pas pu être lue.');
    }
  };

  return (
    <section className={s.card} aria-labelledby="recap-title">
      <h2 className={s.cardTitle} id="recap-title">
        Votre marque
      </h2>
      <p className={s.muted}>
        Reconnue depuis {brand.site || 'votre site'}. Corrigez ce qui ne va pas — le reste se
        règle ensuite dans l’éditeur.
      </p>

      <div className={s.recap}>
        <div className={s.logoBox}>
          {logo ? (
            <img className={s.logo} src={logo} alt={`Logo de ${brand.name}`} />
          ) : (
            <p className={s.noLogo}>Aucun logo</p>
          )}
          {/* Le champ précède son <label> : c'est ce qui permet à celui-ci de porter
              l'anneau de focus clavier du champ, qui est invisible. */}
          <input
            id={fileId}
            className={s.srOnly}
            type="file"
            accept="image/png,image/jpeg,image/webp,image/gif"
            onChange={(event) => {
              void pickLogo(event.currentTarget.files?.[0]);
              // Réinitialisé : rechoisir le même fichier après une erreur doit relancer l'événement.
              event.currentTarget.value = '';
            }}
          />
          <label className={s.fileBtn} htmlFor={fileId}>
            {logo ? 'Remplacer' : 'Ajouter un logo'}
          </label>
          {logo && (
            <button
              className={s.linkBtn}
              type="button"
              onClick={() => onChange({ ...brand, logo: null, logo_asset_id: null })}
            >
              Retirer
            </button>
          )}
        </div>

        <div className={s.recapFields}>
          <Field label="Nom de la marque" hint="Il apparaît dans la signature.">
            <Input
              value={brand.name}
              maxLength={60}
              onChange={(event) => onChange({ ...brand, name: event.currentTarget.value })}
            />
          </Field>

          <fieldset className={s.fieldset}>
            <legend className={s.legend}>Couleurs détectées</legend>
            {brand.colors.length === 0 ? (
              <p className={s.muted}>
                Aucune couleur n’a été détectée. Les propositions partiront de notre palette par
                défaut.
              </p>
            ) : (
              <ul className={s.swatches}>
                {brand.colors.map((color, index) => (
                  // La position EST l'identité : deux couleurs peuvent devenir identiques
                  // pendant l'édition, et la première sert d'accent principal.
                  <li className={s.swatchRow} key={index}>
                    <input
                      className={s.swatchInput}
                      type="color"
                      value={color}
                      aria-label={`Couleur ${index + 1}`}
                      onChange={(event) => setColor(index, event.currentTarget.value)}
                    />
                    <button
                      className={s.swatchRemove}
                      type="button"
                      onClick={() => removeColor(index)}
                    >
                      Retirer<span className={s.srOnly}> la couleur {index + 1}</span>
                    </button>
                  </li>
                ))}
              </ul>
            )}
            <p className={s.muted}>La première couleur sert d’accent principal.</p>
          </fieldset>

          {contacts.length > 0 && (
            <div className={s.detected}>
              <p className={s.legend}>Contacts récupérés</p>
              <ul className={s.detectedList}>
                {contacts.map((item) => (
                  <li key={item.label}>
                    <span>{item.label}</span>
                    <strong>{item.value}</strong>
                  </li>
                ))}
              </ul>
            </div>
          )}
        </div>
      </div>

      {error && (
        <p className={s.errorText} role="alert">
          {error}
        </p>
      )}
      <p className={s.muted}>
        Formats acceptés : PNG, JPEG, WebP, GIF — jusqu’à {MAX_LOGO_BYTES / (1024 * 1024)} Mo.
      </p>
    </section>
  );
}
