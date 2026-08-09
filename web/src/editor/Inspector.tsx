/**
 * Inspecteur : onglet Style, onglet Animation. Tous les champs du contrat §3, et rien d'autre.
 * Les listes déroulantes sont peuplées depuis les tableaux `as const` de lib/types :
 * une copie locale finirait par diverger de ce que le serveur accepte.
 */
import { useState } from 'react';
import type { Dispatch } from 'react';
import { Field } from '../components/Field';
import { Input } from '../components/Input';
import { Select } from '../components/Select';
import {
  ALIGNS,
  ANIM_DIRECTIONS,
  ANIM_ITERATIONS,
  ANIM_PRESETS,
  EASINGS,
} from '../lib/types';
import type { Align, AnimDirection, AnimIterations, AnimPreset, Asset, Easing, Element } from '../lib/types';
import { TYPE_LABELS, selectedElement } from './state';
import type { Action, EditorState } from './state';
import s from './editor.module.css';

const number = (value: string, fallback: number): number => {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : fallback;
};

const PRESET_LABELS: Record<AnimPreset, [icon: string, label: string]> = {
  none: ['∅', 'Aucune'],
  pulse: ['◉', 'Pulsation'],
  glow: ['✦', 'Halo'],
  float: ['↕', 'Flottement'],
  rotate: ['⟳', 'Rotation'],
  bounce: ['⌁', 'Rebond'],
  zoom: ['⊕', 'Zoom'],
  fade: ['◌', 'Fondu'],
  reveal: ['▰', 'Révélation'],
  shimmer: ['✧', 'Reflet'],
  flicker: ['⚡', 'Clignotement'],
  swing: ['⌇', 'Balancement'],
  slide: ['→', 'Glissement'],
  draw: ['◯', 'Tracé'],
};

const EASING_LABELS: Record<Easing, string> = {
  linear: 'Linéaire',
  ease: 'Douce',
  'ease-in': 'Départ lent',
  'ease-out': 'Arrivée lente',
  'ease-in-out': 'Lissée',
};

const DIRECTION_LABELS: Record<AnimDirection, string> = {
  normal: 'Normale',
  reverse: 'Inversée',
  alternate: 'Alternée',
};

const ALIGN_LABELS: Record<Align, string> = { left: 'Gauche', center: 'Centre', right: 'Droite' };

const FONT_WEIGHTS = ['400', '500', '600', '700', '800'];

const NO_CONTENT = new Set<Element['type']>(['image', 'video', 'shape', 'divider']);
const NO_HREF = new Set<Element['type']>(['shape', 'divider', 'video']);

interface InspectorProps {
  state: EditorState;
  dispatch: Dispatch<Action>;
  assets: Asset[];
}

export function Inspector({ state, dispatch, assets }: InspectorProps) {
  const [tab, setTab] = useState<'style' | 'animate'>('style');
  const element = selectedElement(state);
  const { canvas } = state.doc;

  const patch = (id: string, value: Partial<Element>, coalesce?: string) =>
    dispatch({ type: 'update', id, patch: value, coalesce });

  return (
    <>
      <div className={s.rightHead}>
        <div>
          <small>INSPECTEUR</small>
          <h2>{element ? TYPE_LABELS[element.type] : 'Canvas'}</h2>
        </div>
        <div className={s.topGroup}>
          <button
            type="button"
            className={s.iconBtn}
            disabled={!element}
            aria-label="Dupliquer l’élément"
            title="Dupliquer (Ctrl+D)"
            onClick={() => element && dispatch({ type: 'duplicate', id: element.id })}
          >
            ⧉
          </button>
          <button
            type="button"
            className={s.iconBtn}
            disabled={!element}
            aria-label="Supprimer l’élément"
            title="Supprimer (Suppr)"
            onClick={() => element && dispatch({ type: 'remove', id: element.id })}
          >
            ⌫
          </button>
        </div>
      </div>

      <div className={s.tabs} role="tablist" aria-label="Inspecteur">
        <button
          type="button"
          role="tab"
          id="tab-style"
          aria-selected={tab === 'style'}
          aria-controls="panel-inspector"
          onClick={() => setTab('style')}
        >
          Style
        </button>
        <button
          type="button"
          role="tab"
          id="tab-animate"
          aria-selected={tab === 'animate'}
          aria-controls="panel-inspector"
          onClick={() => setTab('animate')}
        >
          Animation
        </button>
      </div>

      <div
        className={s.inspector}
        role="tabpanel"
        id="panel-inspector"
        aria-labelledby={tab === 'style' ? 'tab-style' : 'tab-animate'}
      >
        {tab === 'style' && !element && (
          <>
            <div className={s.group}>
              <span className={s.groupTitle}>Dimensions</span>
              <div className={s.row2}>
                <Field label="Largeur">
                  <Input
                    type="number"
                    min={120}
                    max={1200}
                    value={canvas.width}
                    onChange={(e) =>
                      dispatch({
                        type: 'updateCanvas',
                        patch: { width: number(e.target.value, canvas.width) },
                        coalesce: 'canvas.width',
                      })
                    }
                  />
                </Field>
                <Field label="Hauteur">
                  <Input
                    type="number"
                    min={60}
                    max={800}
                    value={canvas.height}
                    onChange={(e) =>
                      dispatch({
                        type: 'updateCanvas',
                        patch: { height: number(e.target.value, canvas.height) },
                        coalesce: 'canvas.height',
                      })
                    }
                  />
                </Field>
              </div>
            </div>

            <div className={s.group}>
              <span className={s.groupTitle}>Arrière-plan</span>
              <Field label="Couleur">
                <Input
                  type="color"
                  value={canvas.bg}
                  onChange={(e) =>
                    dispatch({ type: 'updateCanvas', patch: { bg: e.target.value }, coalesce: 'canvas.bg' })
                  }
                />
              </Field>
              <Field label="Image de fond" hint="URL http(s) publique. Les adresses privées sont refusées.">
                <Input
                  type="url"
                  value={canvas.bgImage}
                  placeholder="https://"
                  onChange={(e) =>
                    dispatch({
                      type: 'updateCanvas',
                      patch: { bgImage: e.target.value },
                      coalesce: 'canvas.bgImage',
                    })
                  }
                />
              </Field>
              <div className={s.row2}>
                <Field label={`Voile ${Math.round(canvas.overlay * 100)} %`}>
                  <Input
                    type="range"
                    min={0}
                    max={0.95}
                    step={0.05}
                    value={canvas.overlay}
                    onChange={(e) =>
                      dispatch({
                        type: 'updateCanvas',
                        patch: { overlay: number(e.target.value, canvas.overlay) },
                        coalesce: 'canvas.overlay',
                      })
                    }
                  />
                </Field>
                <Field label="Rayon">
                  <Input
                    type="number"
                    min={0}
                    max={60}
                    value={canvas.radius}
                    onChange={(e) =>
                      dispatch({
                        type: 'updateCanvas',
                        patch: { radius: number(e.target.value, canvas.radius) },
                        coalesce: 'canvas.radius',
                      })
                    }
                  />
                </Field>
              </div>
            </div>

            <p className={s.note}>
              Sélectionnez un élément du canvas pour modifier son style et son animation.
            </p>
          </>
        )}

        {tab === 'style' && element && (
          <>
            <div className={s.group}>
              <span className={s.groupTitle}>Position et taille</span>
              <div className={s.row4}>
                <Field label="X">
                  <Input
                    type="number"
                    value={element.x}
                    onChange={(e) => patch(element.id, { x: number(e.target.value, element.x) }, `x:${element.id}`)}
                  />
                </Field>
                <Field label="Y">
                  <Input
                    type="number"
                    value={element.y}
                    onChange={(e) => patch(element.id, { y: number(e.target.value, element.y) }, `y:${element.id}`)}
                  />
                </Field>
                <Field label="L">
                  <Input
                    type="number"
                    min={1}
                    value={element.w}
                    onChange={(e) => patch(element.id, { w: number(e.target.value, element.w) }, `w:${element.id}`)}
                  />
                </Field>
                <Field label="H">
                  <Input
                    type="number"
                    min={1}
                    value={element.h}
                    onChange={(e) => patch(element.id, { h: number(e.target.value, element.h) }, `h:${element.id}`)}
                  />
                </Field>
              </div>
              <div className={s.row2}>
                <Field label="Rotation (°)">
                  <Input
                    type="number"
                    min={0}
                    max={359}
                    value={element.rotation}
                    onChange={(e) =>
                      patch(element.id, { rotation: number(e.target.value, element.rotation) }, `rot:${element.id}`)
                    }
                  />
                </Field>
                <Field label={`Opacité ${Math.round(element.opacity * 100)} %`}>
                  <Input
                    type="range"
                    min={0}
                    max={1}
                    step={0.05}
                    value={element.opacity}
                    onChange={(e) =>
                      patch(element.id, { opacity: number(e.target.value, element.opacity) }, `op:${element.id}`)
                    }
                  />
                </Field>
              </div>
            </div>

            <div className={s.group}>
              <span className={s.groupTitle}>Contenu</span>
              {!NO_CONTENT.has(element.type) && (
                <Field label="Texte" hint="Jetons acceptés : {{name}}, {{role}}, {{email}}…">
                  <textarea
                    className={s.textarea}
                    rows={3}
                    value={element.content}
                    onChange={(e) => patch(element.id, { content: e.target.value }, `content:${element.id}`)}
                  />
                </Field>
              )}
              {!NO_HREF.has(element.type) && (
                <Field label="Lien" hint="http, https, mailto ou tel. Les clics sont comptés.">
                  <Input
                    type="text"
                    value={element.href}
                    placeholder="https://"
                    onChange={(e) => patch(element.id, { href: e.target.value }, `href:${element.id}`)}
                  />
                </Field>
              )}
              {(element.type === 'image' || element.type === 'video') && (
                <Field label="Média" hint="Envoyez vos fichiers depuis l’onglet Médias.">
                  <Select
                    value={element.assetId ?? ''}
                    onChange={(e) => patch(element.id, { assetId: e.target.value || null })}
                  >
                    <option value="">Aucun</option>
                    {assets
                      .filter((asset) => asset.kind === element.type)
                      .map((asset) => (
                        <option key={asset.id} value={asset.id}>
                          {asset.filename}
                        </option>
                      ))}
                  </Select>
                </Field>
              )}
            </div>

            <div className={s.group}>
              <span className={s.groupTitle}>Style</span>
              {!NO_CONTENT.has(element.type) && (
                <>
                  <div className={s.row2}>
                    <Field label="Taille du texte">
                      <Input
                        type="number"
                        min={6}
                        max={72}
                        value={element.fontSize}
                        onChange={(e) =>
                          patch(element.id, { fontSize: number(e.target.value, element.fontSize) }, `fs:${element.id}`)
                        }
                      />
                    </Field>
                    <Field label="Graisse">
                      <Select
                        value={element.fontWeight}
                        onChange={(e) => patch(element.id, { fontWeight: e.target.value })}
                      >
                        {FONT_WEIGHTS.map((weight) => (
                          <option key={weight} value={weight}>
                            {weight}
                          </option>
                        ))}
                      </Select>
                    </Field>
                  </div>
                  <div className={s.row2}>
                    <Field label="Couleur du texte">
                      <Input
                        type="color"
                        value={element.color}
                        onChange={(e) => patch(element.id, { color: e.target.value }, `color:${element.id}`)}
                      />
                    </Field>
                    <Field label="Alignement">
                      <Select
                        value={element.align}
                        onChange={(e) => patch(element.id, { align: e.target.value as Align })}
                      >
                        {ALIGNS.map((align) => (
                          <option key={align} value={align}>
                            {ALIGN_LABELS[align]}
                          </option>
                        ))}
                      </Select>
                    </Field>
                  </div>
                </>
              )}
              <div className={s.row2}>
                {element.type !== 'text' && element.type !== 'image' && element.type !== 'video' && (
                  <Field label="Fond">
                    <Input
                      type="color"
                      value={element.background}
                      onChange={(e) => patch(element.id, { background: e.target.value }, `bg:${element.id}`)}
                    />
                  </Field>
                )}
                <Field label="Rayon">
                  <Input
                    type="number"
                    min={0}
                    max={99}
                    value={element.radius}
                    onChange={(e) =>
                      patch(element.id, { radius: number(e.target.value, element.radius) }, `radius:${element.id}`)
                    }
                  />
                </Field>
              </div>
            </div>

            <div className={s.group}>
              <span className={s.groupTitle}>Calque</span>
              <div className={s.row2}>
                <button
                  type="button"
                  className={s.tinyBtn}
                  onClick={() => dispatch({ type: 'reorder', id: element.id, to: 'front' })}
                >
                  Premier plan
                </button>
                <button
                  type="button"
                  className={s.tinyBtn}
                  onClick={() => dispatch({ type: 'reorder', id: element.id, to: 'back' })}
                >
                  Arrière-plan
                </button>
              </div>
              <label className={s.switch}>
                <span>Verrouiller</span>
                <input
                  type="checkbox"
                  checked={element.locked}
                  onChange={(e) => patch(element.id, { locked: e.target.checked })}
                />
              </label>
              <label className={s.switch}>
                <span>Masquer</span>
                <input
                  type="checkbox"
                  checked={element.hidden}
                  onChange={(e) => patch(element.id, { hidden: e.target.checked })}
                />
              </label>
            </div>
          </>
        )}

        {tab === 'animate' && !element && (
          <p className={s.note}>Sélectionnez un élément pour l’animer.</p>
        )}

        {tab === 'animate' && element && (
          <>
            <div className={s.group}>
              <span className={s.groupTitle}>Effet</span>
              <div className={s.presetGrid}>
                {ANIM_PRESETS.map((preset) => {
                  const [icon, label] = PRESET_LABELS[preset];
                  return (
                    <button
                      key={preset}
                      type="button"
                      className={[s.preset, element.anim.preset === preset ? s.presetActive : null]
                        .filter(Boolean)
                        .join(' ')}
                      aria-pressed={element.anim.preset === preset}
                      onClick={() => dispatch({ type: 'updateAnim', id: element.id, patch: { preset } })}
                    >
                      <i aria-hidden="true">{icon}</i>
                      <span>{label}</span>
                    </button>
                  );
                })}
              </div>
            </div>

            <div className={s.group}>
              <span className={s.groupTitle}>Rythme</span>
              <div className={s.row2}>
                <Field label="Durée (s)">
                  <Input
                    type="number"
                    min={0.1}
                    max={20}
                    step={0.1}
                    value={element.anim.duration}
                    onChange={(e) =>
                      dispatch({
                        type: 'updateAnim',
                        id: element.id,
                        patch: { duration: number(e.target.value, element.anim.duration) },
                        coalesce: `dur:${element.id}`,
                      })
                    }
                  />
                </Field>
                <Field label="Délai (s)">
                  <Input
                    type="number"
                    min={0}
                    max={20}
                    step={0.1}
                    value={element.anim.delay}
                    onChange={(e) =>
                      dispatch({
                        type: 'updateAnim',
                        id: element.id,
                        patch: { delay: number(e.target.value, element.anim.delay) },
                        coalesce: `delay:${element.id}`,
                      })
                    }
                  />
                </Field>
              </div>
              <div className={s.row2}>
                <Field label="Répétitions">
                  <Select
                    value={element.anim.iterations}
                    onChange={(e) =>
                      dispatch({
                        type: 'updateAnim',
                        id: element.id,
                        patch: { iterations: e.target.value as AnimIterations },
                      })
                    }
                  >
                    {ANIM_ITERATIONS.map((value) => (
                      <option key={value} value={value}>
                        {value === 'infinite' ? 'En boucle' : `${value} ×`}
                      </option>
                    ))}
                  </Select>
                </Field>
                <Field label="Accélération">
                  <Select
                    value={element.anim.easing}
                    onChange={(e) =>
                      dispatch({ type: 'updateAnim', id: element.id, patch: { easing: e.target.value as Easing } })
                    }
                  >
                    {EASINGS.map((easing) => (
                      <option key={easing} value={easing}>
                        {EASING_LABELS[easing]}
                      </option>
                    ))}
                  </Select>
                </Field>
              </div>
              <Field label={`Intensité ${element.anim.intensity.toFixed(1)}`}>
                <Input
                  type="range"
                  min={0.1}
                  max={2}
                  step={0.1}
                  value={element.anim.intensity}
                  onChange={(e) =>
                    dispatch({
                      type: 'updateAnim',
                      id: element.id,
                      patch: { intensity: number(e.target.value, element.anim.intensity) },
                      coalesce: `int:${element.id}`,
                    })
                  }
                />
              </Field>
              <Field label="Direction">
                <Select
                  value={element.anim.direction}
                  onChange={(e) =>
                    dispatch({
                      type: 'updateAnim',
                      id: element.id,
                      patch: { direction: e.target.value as AnimDirection },
                    })
                  }
                >
                  {ANIM_DIRECTIONS.map((direction) => (
                    <option key={direction} value={direction}>
                      {DIRECTION_LABELS[direction]}
                    </option>
                  ))}
                </Select>
              </Field>
            </div>

            <div className={s.group}>
              <span className={s.groupTitle}>Rendu dans les clients mail</span>
              <div className={s.chipGrid}>
                <div className={s.chip}>
                  <strong>Gmail</strong>
                  <small className={s.chipOk}>GIF animé ✓</small>
                </div>
                <div className={s.chip}>
                  <strong>Apple Mail</strong>
                  <small className={s.chipOk}>GIF animé ✓</small>
                </div>
                <div className={s.chip}>
                  <strong>Outlook web</strong>
                  <small className={s.chipOk}>GIF animé ✓</small>
                </div>
                <div className={s.chip}>
                  <strong>Outlook ancien</strong>
                  <small className={s.chipWarn}>Première image</small>
                </div>
              </div>
            </div>
          </>
        )}
      </div>
    </>
  );
}
