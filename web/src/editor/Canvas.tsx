/**
 * Canvas d'édition. DOM React ordinaire : il sert à MANIPULER, pas à rendre (contrat §2).
 * Le HTML de vérité (aperçu, export, GIF) est produit par le serveur.
 *
 * Le déplacement, le redimensionnement et la rotation écrivent directement dans le style du
 * nœud pendant le geste et ne produisent qu'UNE action de réducteur au relâchement : sinon
 * l'application entière se re-rendrait soixante fois par seconde pour un glissement.
 */
import { useRef, useState } from 'react';
import { LockKeyhole } from 'lucide-react';
import type {
  CSSProperties,
  Dispatch,
  KeyboardEvent as ReactKeyboardEvent,
  PointerEvent as ReactPointerEvent,
} from 'react';
import type { Asset, Doc, Element, Profile } from '../lib/types';
import { FREE_BRANDING_LABEL } from './BrandingUpsell';
import { TYPE_LABELS, elementLabel, isElementType, resolveTokens } from './state';
import type { Action } from './state';
import s from './editor.module.css';

/** Autorise les variables CSS dans un style inline sans passer par `any`. */
type Style = CSSProperties & { [key: `--${string}`]: string | number };

interface CanvasProps {
  doc: Doc;
  selectedId: string | null;
  dispatch: Dispatch<Action>;
  profile: Profile;
  assets: Map<string, Asset>;
  zoom: number;
  grid: boolean;
  playing: boolean;
  /** Incrémenté par « Rejouer » : remonte les nœuds, donc relance les animations. */
  restartKey: number;
  branding: boolean;
  onBrandingAttempt: () => void;
  onUploadFiles?: (files: File[], point: { x: number; y: number }) => void;
}

const TRANSPARENT_BACKGROUND = new Set(['text', 'image', 'video']);
const EDITABLE_TYPES = new Set<Element['type']>(['text', 'button', 'badge', 'banner']);
const RESIZE_DIRECTIONS = ['nw', 'n', 'ne', 'e', 'se', 's', 'sw', 'w'] as const;
type ResizeDirection = (typeof RESIZE_DIRECTIONS)[number];

const snap = (value: number, enabled: boolean): number =>
  enabled ? Math.round(value / 5) * 5 : Math.round(value);

function elementStyle(element: Element, index: number, playing: boolean): Style {
  const { anim } = element;
  const animated = anim.preset !== 'none';
  return {
    left: element.x,
    top: element.y,
    width: element.w,
    height: element.h,
    zIndex: index + 1,
    opacity: element.opacity,
    color: element.color,
    background:
      element.type === 'divider'
        ? element.background
        : TRANSPARENT_BACKGROUND.has(element.type)
          ? 'transparent'
          : element.background,
    borderRadius: element.radius,
    fontSize: element.fontSize,
    fontWeight: element.fontWeight,
    textAlign: element.align,
    justifyContent:
      element.align === 'center' ? 'center' : element.align === 'right' ? 'flex-end' : 'flex-start',
    transform: `rotate(${element.rotation}deg)`,
    cursor: element.locked ? 'not-allowed' : 'move',
    ...(animated
      ? {
          animationDuration: `${anim.duration}s`,
          animationDelay: `${anim.delay}s`,
          animationIterationCount: anim.iterations === 'infinite' ? 'infinite' : Number(anim.iterations),
          animationTimingFunction: anim.easing,
          animationDirection: anim.direction,
          animationFillMode: 'both',
          animationPlayState: playing ? 'running' : 'paused',
          '--intensity': anim.intensity,
        }
      : {}),
  };
}

export function Canvas({
  doc,
  selectedId,
  dispatch,
  profile,
  assets,
  zoom,
  grid,
  playing,
  restartKey,
  branding,
  onBrandingAttempt,
  onUploadFiles,
}: CanvasProps) {
  // Le geste lit le zoom courant sans devoir se réabonner à chaque changement.
  const zoomRef = useRef(zoom);
  zoomRef.current = zoom;
  const [editingId, setEditingId] = useState<string | null>(null);
  const [dropActive, setDropActive] = useState(false);

  function startGesture(event: ReactPointerEvent<HTMLButtonElement>, element: Element) {
    if (event.button !== 0) return;
    if (selectedId !== element.id) dispatch({ type: 'select', id: element.id });
    if (element.locked) return;

    const target = event.target instanceof HTMLElement ? event.target : null;
    if (target?.isContentEditable) return;
    const handle = target?.closest<HTMLElement>('[data-handle]')?.dataset.handle ?? 'move';
    const direction = handle.startsWith('resize-')
      ? (handle.slice('resize-'.length) as ResizeDirection)
      : null;
    const mode = handle === 'rotate' ? 'rotate' : direction ? 'resize' : 'move';

    event.preventDefault();
    const node = event.currentTarget;
    const startX = event.clientX;
    const startY = event.clientY;
    const rect = node.getBoundingClientRect();
    const centerX = rect.left + rect.width / 2;
    const centerY = rect.top + rect.height / 2;
    const startAngle = Math.atan2(startY - centerY, startX - centerX);
    const ratio = element.w / Math.max(1, element.h);
    const size = node.querySelector<HTMLElement>('[data-size-readout]');

    let patch: Partial<Element> = {};

    const onMove = (move: PointerEvent) => {
      const dx = (move.clientX - startX) / zoomRef.current;
      const dy = (move.clientY - startY) / zoomRef.current;

      if (mode === 'move') {
        patch = {
          x: snap(element.x + dx, !move.altKey),
          y: snap(element.y + dy, !move.altKey),
        };
        node.style.left = `${patch.x ?? 0}px`;
        node.style.top = `${patch.y ?? 0}px`;
        return;
      }
      if (mode === 'resize' && direction) {
        let x = element.x;
        let y = element.y;
        let w = element.w;
        let h = element.h;

        if (direction.includes('e')) w = element.w + dx;
        if (direction.includes('s')) h = element.h + dy;
        if (direction.includes('w')) {
          x = element.x + dx;
          w = element.w - dx;
        }
        if (direction.includes('n')) {
          y = element.y + dy;
          h = element.h - dy;
        }

        if (move.shiftKey && direction.length === 2) {
          if (Math.abs(w - element.w) >= Math.abs(h - element.h)) h = w / ratio;
          else w = h * ratio;
          if (direction.includes('w')) x = element.x + element.w - w;
          if (direction.includes('n')) y = element.y + element.h - h;
        }

        const min = 8;
        if (w < min) {
          if (direction.includes('w')) x = element.x + element.w - min;
          w = min;
        }
        if (h < min) {
          if (direction.includes('n')) y = element.y + element.h - min;
          h = min;
        }

        patch = {
          x: snap(x, !move.altKey),
          y: snap(y, !move.altKey),
          w: snap(w, !move.altKey),
          h: snap(h, !move.altKey),
        };
        node.style.left = `${patch.x}px`;
        node.style.top = `${patch.y}px`;
        node.style.width = `${patch.w}px`;
        node.style.height = `${patch.h}px`;
        if (size) size.textContent = `${patch.w} × ${patch.h}`;
        return;
      }
      const angle = Math.atan2(move.clientY - centerY, move.clientX - centerX) - startAngle;
      const degrees = element.rotation + (angle * 180) / Math.PI;
      const snapped = move.shiftKey ? Math.round(degrees / 15) * 15 : Math.round(degrees);
      patch = { rotation: ((snapped % 360) + 360) % 360 };
      node.style.transform = `rotate(${patch.rotation ?? 0}deg)`;
    };

    const onUp = () => {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
      window.removeEventListener('pointercancel', onUp);
      if (Object.keys(patch).length > 0) dispatch({ type: 'update', id: element.id, patch });
    };

    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
    window.addEventListener('pointercancel', onUp);
  }

  const canvasStyle: CSSProperties = {
    width: doc.canvas.width,
    height: doc.canvas.height,
    borderRadius: doc.canvas.radius,
    background: doc.canvas.bg,
    backgroundImage: doc.canvas.bgImage
      ? `linear-gradient(rgba(0,0,0,${doc.canvas.overlay}),rgba(0,0,0,${doc.canvas.overlay})),url("${doc.canvas.bgImage}")`
      : undefined,
  };
  const signatureStyle: CSSProperties = {
    width: doc.canvas.width,
    transform: `scale(${zoom})`,
  };

  return (
    <div className={s.canvasWrap}>
      {/* data-motion="keep" : les animations de la signature SONT le contenu, elles
          survivent à prefers-reduced-motion (docs/DESIGN.md §Accessibilité). */}
      <div className={s.canvasSignature} style={signatureStyle}>
        <div
          className={[s.canvas, grid ? s.grid : null].filter(Boolean).join(' ')}
          style={canvasStyle}
          data-motion="keep"
        onPointerDown={(event) => {
          if (event.target === event.currentTarget) dispatch({ type: 'select', id: null });
        }}
        onDragOver={(event) => event.preventDefault()}
        onDragEnter={(event) => {
          if (event.dataTransfer.types.includes('Files')) setDropActive(true);
        }}
        onDragLeave={(event) => {
          if (event.target === event.currentTarget) setDropActive(false);
        }}
        onDrop={(event) => {
          event.preventDefault();
          setDropActive(false);
          const rect = event.currentTarget.getBoundingClientRect();
          const point = {
            x: Math.round((event.clientX - rect.left) / zoom),
            y: Math.round((event.clientY - rect.top) / zoom),
          };
          const files = Array.from(event.dataTransfer.files);
          if (files.length > 0) {
            onUploadFiles?.(files, point);
            return;
          }
          const kind = event.dataTransfer.getData('text/plain');
          if (!isElementType(kind)) return;
          dispatch({
            type: 'add',
            kind,
            x: point.x,
            y: point.y,
          });
        }}
        >
        {dropActive && <div className={s.canvasDropHint}>Déposez vos images, GIF ou vidéos</div>}
        {doc.elements.map((element, index) => {
          if (element.hidden) return null;
          const selected = element.id === selectedId;
          const asset = element.assetId ? assets.get(element.assetId) : undefined;
          const animated = element.anim.preset !== 'none';

          return (
            <button
              type="button"
              key={`${element.id}:${restartKey}`}
              className={[
                s.ce,
                animated ? `anim-${element.anim.preset}` : null,
                selected ? s.ceSelected : null,
                element.locked ? s.ceLocked : null,
              ]
                .filter(Boolean)
                .join(' ')}
              style={elementStyle(element, index, playing)}
              aria-pressed={selected}
              aria-label={`${TYPE_LABELS[element.type]} : ${elementLabel(element, profile)}`}
              onPointerDown={(event) => startGesture(event, element)}
              onDoubleClick={(event) => {
                if (!element.locked && EDITABLE_TYPES.has(element.type)) {
                  event.preventDefault();
                  event.stopPropagation();
                  setEditingId(element.id);
                }
              }}
              onFocus={() => {
                if (selectedId !== element.id) dispatch({ type: 'select', id: element.id });
              }}
            >
              {element.type === 'image' &&
                (asset ? (
                  <img src={asset.url} alt="" style={{ borderRadius: element.radius }} />
                ) : (
                  <span className={s.mediaPlaceholder}>Image à choisir</span>
                ))}

              {element.type === 'video' &&
                (asset ? (
                  <video
                    src={asset.url}
                    loop
                    muted
                    playsInline
                    autoPlay={playing}
                    style={{ borderRadius: element.radius }}
                    ref={(node) => {
                      if (!node) return;
                      if (playing) void node.play().catch(() => undefined);
                      else node.pause();
                    }}
                  />
                ) : (
                  <span className={s.mediaPlaceholder}>Vidéo à choisir</span>
                ))}

              {element.type !== 'image' &&
                element.type !== 'video' &&
                element.type !== 'shape' &&
                element.type !== 'divider' && (
                  <span
                    className={s.ceText}
                    contentEditable={editingId === element.id}
                    suppressContentEditableWarning
                    ref={(node) => {
                      if (!node || editingId !== element.id || document.activeElement === node) return;
                      node.focus();
                      const selection = window.getSelection();
                      const range = document.createRange();
                      range.selectNodeContents(node);
                      selection?.removeAllRanges();
                      selection?.addRange(range);
                    }}
                    onPointerDown={(event) => {
                      if (editingId === element.id) event.stopPropagation();
                    }}
                    onBlur={(event) => {
                      if (editingId !== element.id) return;
                      const content = event.currentTarget.textContent ?? '';
                      setEditingId(null);
                      if (content !== element.content) {
                        dispatch({ type: 'update', id: element.id, patch: { content } });
                      }
                    }}
                    onKeyDown={(event: ReactKeyboardEvent<HTMLSpanElement>) => {
                      if (event.key === 'Escape') {
                        event.preventDefault();
                        event.currentTarget.textContent = element.content;
                        event.currentTarget.blur();
                      }
                      if (event.key === 'Enter' && !event.shiftKey) {
                        event.preventDefault();
                        event.currentTarget.blur();
                      }
                    }}
                  >
                    {editingId === element.id ? element.content : resolveTokens(element.content, profile)}
                  </span>
                )}

              {animated && <span className={s.playBadge}>✦ {element.anim.preset}</span>}

              {selected && !element.locked && (
                <>
                  <span className={s.rotationStem} aria-hidden="true" />
                  {RESIZE_DIRECTIONS.map((direction) => (
                    <span
                      key={direction}
                      className={`${s.handle} ${s[`handle${direction.toUpperCase()}` as keyof typeof s]}`}
                      data-handle={`resize-${direction}`}
                      aria-hidden="true"
                    />
                  ))}
                  <span className={`${s.handle} ${s.handleRotate}`} data-handle="rotate" aria-hidden="true" />
                  <span className={s.sizeReadout} data-size-readout aria-hidden="true">
                    {element.w} × {element.h}
                  </span>
                </>
              )}
            </button>
          );
        })}
        </div>
        {branding && (
          <button
            type="button"
            className={s.freeBranding}
            onClick={onBrandingAttempt}
            title="Retirer la mention avec Pro"
          >
            <span>{FREE_BRANDING_LABEL}</span>
            <LockKeyhole size={11} aria-hidden="true" />
          </button>
        )}
      </div>
    </div>
  );
}
