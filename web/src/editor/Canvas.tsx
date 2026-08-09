/**
 * Canvas d'édition. DOM React ordinaire : il sert à MANIPULER, pas à rendre (contrat §2).
 * Le HTML de vérité (aperçu, export, GIF) est produit par le serveur.
 *
 * Le déplacement, le redimensionnement et la rotation écrivent directement dans le style du
 * nœud pendant le geste et ne produisent qu'UNE action de réducteur au relâchement : sinon
 * l'application entière se re-rendrait soixante fois par seconde pour un glissement.
 */
import { useRef } from 'react';
import type { CSSProperties, Dispatch, PointerEvent as ReactPointerEvent } from 'react';
import type { Asset, Doc, Element, Profile } from '../lib/types';
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
}

const TRANSPARENT_BACKGROUND = new Set(['text', 'image', 'video']);

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
}: CanvasProps) {
  // Le geste lit le zoom courant sans devoir se réabonner à chaque changement.
  const zoomRef = useRef(zoom);
  zoomRef.current = zoom;

  function startGesture(event: ReactPointerEvent<HTMLButtonElement>, element: Element) {
    if (event.button !== 0) return;
    if (selectedId !== element.id) dispatch({ type: 'select', id: element.id });
    if (element.locked) return;

    const target = event.target instanceof HTMLElement ? event.target : null;
    const mode = target?.dataset.handle === 'resize' ? 'resize' : target?.dataset.handle === 'rotate' ? 'rotate' : 'move';

    event.preventDefault();
    const node = event.currentTarget;
    const startX = event.clientX;
    const startY = event.clientY;
    const rect = node.getBoundingClientRect();
    const centerX = rect.left + rect.width / 2;
    const centerY = rect.top + rect.height / 2;
    const startAngle = Math.atan2(startY - centerY, startX - centerX);

    let patch: Partial<Element> = {};

    const onMove = (move: PointerEvent) => {
      const dx = (move.clientX - startX) / zoomRef.current;
      const dy = (move.clientY - startY) / zoomRef.current;

      if (mode === 'move') {
        patch = { x: Math.round(element.x + dx), y: Math.round(element.y + dy) };
        node.style.left = `${patch.x ?? 0}px`;
        node.style.top = `${patch.y ?? 0}px`;
        return;
      }
      if (mode === 'resize') {
        patch = { w: Math.max(8, Math.round(element.w + dx)), h: Math.max(8, Math.round(element.h + dy)) };
        node.style.width = `${patch.w ?? 0}px`;
        node.style.height = `${patch.h ?? 0}px`;
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
      if (Object.keys(patch).length > 0) dispatch({ type: 'update', id: element.id, patch });
    };

    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
  }

  const canvasStyle: CSSProperties = {
    width: doc.canvas.width,
    height: doc.canvas.height,
    borderRadius: doc.canvas.radius,
    backgroundColor: doc.canvas.bg,
    backgroundImage: doc.canvas.bgImage
      ? `linear-gradient(rgba(0,0,0,${doc.canvas.overlay}),rgba(0,0,0,${doc.canvas.overlay})),url("${doc.canvas.bgImage}")`
      : undefined,
    transform: `scale(${zoom})`,
  };

  return (
    <div className={s.canvasWrap}>
      {/* data-motion="keep" : les animations de la signature SONT le contenu, elles
          survivent à prefers-reduced-motion (docs/DESIGN.md §Accessibilité). */}
      <div
        className={[s.canvas, grid ? s.grid : null].filter(Boolean).join(' ')}
        style={canvasStyle}
        data-motion="keep"
        onPointerDown={(event) => {
          if (event.target === event.currentTarget) dispatch({ type: 'select', id: null });
        }}
        onDragOver={(event) => event.preventDefault()}
        onDrop={(event) => {
          event.preventDefault();
          const kind = event.dataTransfer.getData('text/plain');
          if (!isElementType(kind)) return;
          const rect = event.currentTarget.getBoundingClientRect();
          dispatch({
            type: 'add',
            kind,
            x: Math.round((event.clientX - rect.left) / zoom),
            y: Math.round((event.clientY - rect.top) / zoom),
          });
        }}
      >
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
                element.type !== 'divider' &&
                resolveTokens(element.content, profile)}

              {animated && <span className={s.playBadge}>✦ {element.anim.preset}</span>}

              {selected && !element.locked && (
                <>
                  {/* Affordances souris. L'équivalent clavier est l'inspecteur
                      (X, Y, L, H, rotation), d'où aria-hidden plutôt qu'un bouton mort. */}
                  <span className={`${s.handle} ${s.handleResize}`} data-handle="resize" aria-hidden="true" />
                  <span className={`${s.handle} ${s.handleRotate}`} data-handle="rotate" aria-hidden="true" />
                </>
              )}
            </button>
          );
        })}
      </div>
    </div>
  );
}
