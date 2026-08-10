/**
 * Les modèles. Appliquer remplace le document — l'action reste annulable (Ctrl+Z).
 * Fichier nommé TemplateGallery et non Templates : `templates.ts` existe déjà à côté et
 * les systèmes de fichiers insensibles à la casse (macOS, Windows) confondent les deux.
 */
import type { Dispatch } from 'react';
import type { CSSProperties } from 'react';
import type { Element } from '../lib/types';
import { TEMPLATES, docFromTemplate } from './templates';
import type { Template } from './templates';
import type { Action } from './state';
import { FREE_BRANDING_LABEL } from './BrandingUpsell';
import { getAnalytics } from '../lib/analytics';
import { resolveTokens } from './state';
import s from './editor.module.css';

const SAMPLE = {
  name: 'Camille Martin',
  role: 'CEO · Studio North',
  email: 'camille@studio.co',
  phone: '+33 6 12 34 56 78',
  website: 'studio.co',
  linkedin: 'LinkedIn',
  whatsapp: 'WhatsApp',
  tagline: 'Designing ideas people remember.',
  company: 'Studio North',
};

const EMPTY_TYPES = new Set<Element['type']>(['shape', 'divider']);

const DEFAULT_SIZE: Record<Element['type'], { w: number; h: number }> = {
  text: { w: 180, h: 32 },
  image: { w: 72, h: 72 },
  video: { w: 180, h: 96 },
  shape: { w: 120, h: 72 },
  button: { w: 120, h: 36 },
  badge: { w: 82, h: 24 },
  banner: { w: 220, h: 52 },
  divider: { w: 180, h: 2 },
};

const DEFAULT_BACKGROUND: Partial<Record<Element['type'], string>> = {
  shape: '#18233a',
  button: '#2563eb',
  badge: '#17233c',
  banner: '#111c32',
  divider: '#64748b',
};

function canvasTextColor(background: string) {
  const hex = background.match(/#[0-9a-fA-F]{6}/)?.[0];
  if (!hex) return '#f8fafc';
  const rgb = [1, 3, 5].map((start) => Number.parseInt(hex.slice(start, start + 2), 16));
  const luminance = (rgb[0] * 299 + rgb[1] * 587 + rgb[2] * 114) / 1000;
  return luminance > 164 ? '#0f172a' : '#f8fafc';
}

export function TemplatePreview({ template, branding = false }: { template: Template; branding?: boolean }) {
  return (
    <span
      className={s.templateThumb}
      style={{
        aspectRatio: `${template.canvas.width} / ${template.canvas.height}`,
        background: template.canvas.bg,
        borderRadius: Math.max(4, template.canvas.radius * 0.45),
      }}
      aria-hidden="true"
    >
      {template.elements.map((element, index) => {
        const size = DEFAULT_SIZE[element.type];
        const background = ['text', 'image', 'video'].includes(element.type)
          ? 'transparent'
          : (element.background ?? DEFAULT_BACKGROUND[element.type]);
        const style: CSSProperties = {
          left: `${((element.x ?? 0) / template.canvas.width) * 100}%`,
          top: `${((element.y ?? 0) / template.canvas.height) * 100}%`,
          width: `${((element.w ?? size.w) / template.canvas.width) * 100}%`,
          height: `${((element.h ?? size.h) / template.canvas.height) * 100}%`,
          zIndex: index + 1,
          color: element.color ?? canvasTextColor(template.canvas.bg),
          background,
          borderRadius: Math.max(0, (element.radius ?? 0) * 0.45),
          fontSize: Math.max(5.5, (element.fontSize ?? 12) * 0.42),
          fontWeight: element.fontWeight ?? 500,
          opacity: element.opacity ?? 1,
          justifyContent:
            element.align === 'center' ? 'center' : element.align === 'right' ? 'flex-end' : 'flex-start',
          transform: `rotate(${element.rotation ?? 0}deg)`,
        };
        return (
          <span
            key={`${template.id}-${index}`}
            className={s.templatePreviewElement}
            data-type={element.type}
            style={style}
          >
            {element.type === 'image' ? (
              <span className={s.templateAvatar}>CM</span>
            ) : EMPTY_TYPES.has(element.type) ? null : (
              resolveTokens(element.content ?? '', SAMPLE)
            )}
          </span>
        );
      })}
      {branding && <span className={s.templateBranding}>{FREE_BRANDING_LABEL}</span>}
    </span>
  );
}

export function TemplateGallery({
  dispatch,
  branding,
  signatureId,
}: {
  dispatch: Dispatch<Action>;
  branding: boolean;
  signatureId?: string;
}) {
  return (
    <div className={s.templateGrid}>
      {TEMPLATES.map((template) => (
        <button
          key={template.id}
          type="button"
          className={s.template}
          onClick={() => {
            getAnalytics().track('template_selected', {
              ...(signatureId ? { signature_id: signatureId } : {}),
              template_id: template.id,
            });
            dispatch({ type: 'replaceDoc', doc: docFromTemplate(template) });
          }}
        >
          <TemplatePreview template={template} branding={branding} />
          <span className={s.templateMeta}>
            <span className={s.templateTitle}>
              <b>{template.name}</b>
              <em>{template.tag}</em>
            </span>
            <small>{template.description}</small>
          </span>
        </button>
      ))}
    </div>
  );
}
