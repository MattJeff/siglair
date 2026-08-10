/**
 * L'éditeur. Portage React de docs/reference/editor-original.html.
 *
 * Trois différences structurelles avec la maquette, et elles commandent tout le reste :
 *  1. l'état vit dans l'API, pas dans localStorage — un réducteur unique + sauvegarde auto ;
 *  2. les médias sont des `assetId` résolus vers une URL, plus jamais des data-URL ;
 *  3. l'aperçu et l'export viennent du serveur (contrat §2), ce canvas ne sert qu'à manipuler.
 */
import { useCallback, useEffect, useMemo, useReducer, useRef, useState } from 'react';
import { Link, useParams } from 'react-router-dom';
import { Button } from '../components/Button';
import { Spinner } from '../components/Spinner';
import { useToast } from '../components/Toast';
import { ApiError, getSignature, listAssets, uploadAsset } from '../lib/api';
import { getAnalytics } from '../lib/analytics';
import { useSession } from '../lib/session';
import type { Asset, Signature } from '../lib/types';
import { Assets } from '../editor/Assets';
import { AiAssistant } from '../editor/AiAssistant';
import { BrandingUpsell } from '../editor/BrandingUpsell';
import { Campaigns } from '../editor/Campaigns';
import { Canvas } from '../editor/Canvas';
import { ExportModal } from '../editor/ExportModal';
import { Inspector } from '../editor/Inspector';
import { Layers } from '../editor/Layers';
import { Palette } from '../editor/Palette';
import { PreviewModal } from '../editor/PreviewModal';
import { PublishModal } from '../editor/PublishModal';
import { TemplateGallery } from '../editor/TemplateGallery';
import { Timeline } from '../editor/Timeline';
import { Toolbar } from '../editor/Toolbar';
import {
  canRedo,
  canUndo,
  compatibility,
  initialEditorState,
  reducer,
  selectedElement,
  toLocalInput,
} from '../editor/state';
import { useAutosave } from '../editor/useAutosave';
import s from '../editor/editor.module.css';
/*
 * Contrat §3.2 : le CSS des presets est écrit UNE fois, dans src/render/anim.css côté Rust,
 * et inclus par html.rs dans le document rendu. Le canvas d'édition consomme le MÊME fichier
 * plutôt qu'une copie : deux jeux de keyframes finissent toujours par diverger, et l'écart se
 * verrait chez le destinataire, pas ici. La vérité du rendu reste POST /api/preview.
 */
import '../../../src/render/anim.css';

const LEFT_TABS = [
  { id: 'ai', label: 'IA' },
  { id: 'design', label: 'Design' },
  { id: 'templates', label: 'Modèles' },
  { id: 'assets', label: 'Médias' },
  { id: 'campaigns', label: 'Campagne' },
] as const;

type LeftTab = (typeof LEFT_TABS)[number]['id'];

const ZOOMS = [0.5, 0.75, 1, 1.25, 1.5];

export default function Editor() {
  const { id = '' } = useParams<{ id: string }>();
  const [signature, setSignature] = useState<Signature | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    getSignature(id)
      .then((loaded) => {
        if (alive) setSignature(loaded);
      })
      .catch((cause: unknown) => {
        if (!alive) return;
        setError(cause instanceof ApiError ? cause.message : 'Cette signature est introuvable.');
      });
    return () => {
      alive = false;
    };
  }, [id]);

  if (error) {
    return (
      <main className={s.centered}>
        <div>
          <h1>Signature indisponible</h1>
          <p>{error}</p>
          <Link to="/app">
            <Button variant="ghost">Revenir à mes signatures</Button>
          </Link>
        </div>
      </main>
    );
  }

  if (!signature) {
    return (
      <main className={s.centered}>
        <Spinner size={28} label="Chargement de la signature" />
      </main>
    );
  }

  // Monté seulement une fois le document connu : la sauvegarde automatique part alors
  // d'une valeur identique au serveur et n'écrit pas un document vide au démarrage.
  return <EditorShell key={signature.id} signature={signature} />;
}

function EditorShell({ signature }: { signature: Signature }) {
  const toast = useToast();
  const { limits, user, refresh, plan, features } = useSession();
  const [state, dispatch] = useReducer(reducer, signature.doc, initialEditorState);
  const [name, setName] = useState(signature.name);
  const [assets, setAssets] = useState<Asset[]>([]);
  const [tab, setTab] = useState<LeftTab>(plan === 'free' ? 'design' : 'ai');
  const [zoom, setZoom] = useState(1);
  const [view, setView] = useState<'desktop' | 'mobile'>('desktop');
  const [grid, setGrid] = useState(true);
  const [playing, setPlaying] = useState(true);
  const [restartKey, setRestartKey] = useState(0);
  const [modal, setModal] = useState<'preview' | 'export' | 'publish' | 'branding' | null>(null);
  const [publicSlug, setPublicSlug] = useState(signature.public_slug);
  const [simulatedDate, setSimulatedDate] = useState(() => toLocalInput(new Date()));

  const patch = useMemo(() => ({ doc: state.doc, name }), [state.doc, name]);
  const save = useAutosave(signature.id, patch);
  const assetMap = useMemo(() => new Map(assets.map((asset) => [asset.id, asset])), [assets]);
  const report = compatibility(state.doc);
  const selected = selectedElement(state);

  useEffect(() => {
    getAnalytics().track('editor_opened', {
      signature_id: signature.id,
      source: 'unknown',
    });
  }, [signature.id]);

  useEffect(() => {
    let alive = true;
    listAssets()
      .then((list) => {
        if (alive) setAssets(list);
      })
      .catch(() => {
        if (alive) toast('La bibliothèque de médias n’a pas pu être chargée.', 'error');
      });
    return () => {
      alive = false;
    };
  }, [toast]);

  const uploadFiles = useCallback(
    async (files: File[]): Promise<Asset[]> => {
      const supported = files.filter(
        (file) => file.type.startsWith('image/') || file.type.startsWith('video/'),
      );
      if (supported.length === 0) {
        toast('Choisissez une image, un GIF ou une vidéo.', 'error');
        return [];
      }

      const uploaded: Asset[] = [];
      for (const file of supported) {
        try {
          uploaded.push(await uploadAsset(file));
        } catch (cause) {
          toast(cause instanceof ApiError ? cause.message : `Impossible d’importer ${file.name}.`, 'error');
        }
      }
      if (uploaded.length > 0) {
        for (const asset of uploaded) {
          getAnalytics().track('asset_uploaded', {
            signature_id: signature.id,
            asset_kind: asset.kind,
          });
        }
        setAssets((current) => [
          ...uploaded,
          ...current.filter((asset) => !uploaded.some((next) => next.id === asset.id)),
        ]);
        await refresh();
        toast(
          uploaded.length === 1 ? 'Média importé' : `${uploaded.length} médias importés`,
          'success',
        );
      }
      return uploaded;
    },
    [refresh, signature.id, toast],
  );

  async function uploadAt(files: File[], point: { x: number; y: number }) {
    const uploaded = await uploadFiles(files);
    uploaded.forEach((asset, index) => {
      const ratio = asset.width && asset.height ? asset.width / asset.height : asset.kind === 'video' ? 16 / 9 : 1;
      const maxWidth = asset.kind === 'video' ? 200 : 160;
      const maxHeight = asset.kind === 'video' ? 120 : 150;
      let width = maxWidth;
      let height = width / ratio;
      if (height > maxHeight) {
        height = maxHeight;
        width = height * ratio;
      }
      dispatch({
        type: 'add',
        kind: asset.kind,
        assetId: asset.id,
        x: point.x + index * 12,
        y: point.y + index * 12,
        w: Math.max(40, Math.round(width)),
        h: Math.max(40, Math.round(height)),
      });
    });
  }

  // Raccourcis clavier. L'état courant passe par une ref : réabonner la fenêtre
  // à chaque frappe ne servirait à rien.
  const stateRef = useRef(state);
  stateRef.current = state;

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const target = event.target;
      if (
        target instanceof HTMLElement &&
        (target.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName))
      ) {
        return;
      }

      const current = stateRef.current;
      const element = current.doc.elements.find((e) => e.id === current.selectedId) ?? null;
      const command = event.ctrlKey || event.metaKey;

      if (command && event.key.toLowerCase() === 'z') {
        event.preventDefault();
        dispatch({ type: event.shiftKey ? 'redo' : 'undo' });
        return;
      }
      if (command && event.key.toLowerCase() === 'd') {
        event.preventDefault();
        if (element) dispatch({ type: 'duplicate', id: element.id });
        return;
      }
      if ((event.key === 'Delete' || event.key === 'Backspace') && element) {
        event.preventDefault();
        dispatch({ type: 'remove', id: element.id });
        return;
      }
      if (!element || element.locked) return;

      const step = event.shiftKey ? 10 : 1;
      const nudge: Record<string, { x?: number; y?: number }> = {
        ArrowLeft: { x: element.x - step },
        ArrowRight: { x: element.x + step },
        ArrowUp: { y: element.y - step },
        ArrowDown: { y: element.y + step },
      };
      const move = nudge[event.key];
      if (!move) return;
      event.preventDefault();
      dispatch({ type: 'update', id: element.id, patch: move, coalesce: `nudge:${element.id}` });
    };

    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, []);

  function changeView(next: 'desktop' | 'mobile') {
    setView(next);
    setZoom(next === 'mobile' ? 0.75 : 1);
  }

  /** Export et publication lisent le document STOCKÉ : on écrit avant d'ouvrir. */
  async function openAfterSave(target: 'export' | 'publish') {
    await save.flush();
    getAnalytics().track('signature_saved', { signature_id: signature.id });
    setModal(target);
  }

  return (
    <div className={s.app}>
      <Toolbar
        name={name}
        onNameChange={setName}
        save={save}
        canUndo={canUndo(state)}
        canRedo={canRedo(state)}
        onUndo={() => dispatch({ type: 'undo' })}
        onRedo={() => dispatch({ type: 'redo' })}
        view={view}
        onViewChange={changeView}
        grid={grid}
        onGridChange={setGrid}
        analyticsHref={limits && limits.analytics_days > 0 ? `/app/analytics/${signature.id}` : null}
        onPreview={() => {
          getAnalytics().track('editor_preview_opened', {
            signature_id: signature.id,
            mode: view,
          });
          setModal('preview');
        }}
        onExport={() => void openAfterSave('export')}
        onPublish={() => void openAfterSave('publish')}
      />

      <main className={s.workspace}>
        <aside className={`${s.sidebar} ${s.left}`}>
          <div className={s.tabs} role="tablist" aria-label="Outils">
            {LEFT_TABS.map((item) => (
              <button
                key={item.id}
                type="button"
                role="tab"
                id={`tab-${item.id}`}
                aria-selected={tab === item.id}
                aria-controls="panel-left"
                onClick={() => setTab(item.id)}
              >
                {item.label}
              </button>
            ))}
          </div>

          <div className={s.panel} role="tabpanel" id="panel-left" aria-labelledby={`tab-${tab}`}>
            {tab === 'ai' && (
              <AiAssistant
                signatureId={signature.id}
                selectedId={state.selectedId}
                plan={plan}
                providerAvailable={features?.ai_provider !== false}
                save={save}
                dispatch={dispatch}
              />
            )}

            {tab === 'design' && (
              <>
                <div className={s.sectionHead}>
                  <h3>Ajouter</h3>
                  <span>clic ou glisser</span>
                </div>
                <Palette dispatch={dispatch} />
                <div className={`${s.sectionHead} ${s.gap}`}>
                  <h3>Calques</h3>
                  <span>{state.doc.elements.length}</span>
                </div>
                <Layers
                  doc={state.doc}
                  selectedId={state.selectedId}
                  dispatch={dispatch}
                  profile={signature.profile}
                  branding={limits?.branding ?? false}
                  onBrandingAttempt={() => setModal('branding')}
                />
              </>
            )}

            {tab === 'templates' && (
              <>
                <div className={s.sectionHead}>
                  <h3>Modèles</h3>
                  <span>1 clic</span>
                </div>
                <TemplateGallery
                  dispatch={dispatch}
                  branding={limits?.branding ?? false}
                  signatureId={signature.id}
                />
                <p className={`${s.note} ${s.gap}`}>
                  Un modèle remplace le document courant. Ctrl+Z revient en arrière.
                </p>
              </>
            )}

            {tab === 'assets' && (
              <Assets
                assets={assets}
                onAssetsChange={setAssets}
                onUploadFiles={uploadFiles}
                dispatch={dispatch}
              />
            )}

            {tab === 'campaigns' && (
              <Campaigns
                simulatedDate={simulatedDate}
                onSimulatedDateChange={setSimulatedDate}
              />
            )}
          </div>
        </aside>

        <section className={s.stage}>
          <div className={s.stagebar}>
            <div>
              <strong>Canvas</strong>
              <span className={s.stageSize}>
                {state.doc.canvas.width} × {state.doc.canvas.height} px
                {selected ? ` · ${selected.w} × ${selected.h}` : ''}
              </span>
            </div>
            <div className={s.stageControls}>
              <button
                type="button"
                className={s.tinyBtn}
                aria-pressed={playing}
                onClick={() => setPlaying((value) => !value)}
              >
                {playing ? '❚❚ Pause' : '▶ Lecture'}
              </button>
              <button type="button" className={s.tinyBtn} onClick={() => setRestartKey((key) => key + 1)}>
                ↻ Rejouer
              </button>
              <label className={s.inlineLabel}>
                Zoom
                <select
                  name="canvas-zoom"
                  className={s.inlineSelect}
                  value={zoom}
                  onChange={(event) => setZoom(Number(event.target.value))}
                >
                  {ZOOMS.map((value) => (
                    <option key={value} value={value}>
                      {Math.round(value * 100)} %
                    </option>
                  ))}
                </select>
              </label>
            </div>
          </div>

          <Canvas
            doc={state.doc}
            selectedId={state.selectedId}
            dispatch={dispatch}
            profile={signature.profile}
            assets={assetMap}
            zoom={zoom}
            grid={grid}
            playing={playing}
            restartKey={restartKey}
            branding={limits?.branding ?? false}
            onBrandingAttempt={() => setModal('branding')}
            onUploadFiles={(files, point) => void uploadAt(files, point)}
          />

          <Timeline doc={state.doc} dispatch={dispatch} profile={signature.profile} />

          <div className={s.compat}>
            <span className={s.compatLeft}>
              <span
                className={[
                  s.compatDot,
                  report.level === 'warn' ? s.compatWarn : null,
                  report.level === 'risky' ? s.compatRisky : null,
                ]
                  .filter(Boolean)
                  .join(' ')}
                aria-hidden="true"
              />
              <strong>Compatibilité {report.label}</strong>
              <span>{report.score}/100</span>
            </span>
            <span>
              {report.issues.length > 0
                ? `À surveiller : ${report.issues.join(', ')}`
                : 'Aucun point de vigilance.'}
            </span>
          </div>
        </section>

        <aside className={`${s.sidebar} ${s.right}`}>
          <Inspector state={state} dispatch={dispatch} assets={assets} onUploadFiles={uploadFiles} />
        </aside>
      </main>

      <PreviewModal
        open={modal === 'preview'}
        onClose={() => setModal(null)}
        doc={state.doc}
        profile={signature.profile}
        user={user}
        branding={limits?.branding ?? false}
      />
      <BrandingUpsell open={modal === 'branding'} onClose={() => setModal(null)} />
      <ExportModal
        open={modal === 'export'}
        onClose={() => setModal(null)}
        signatureId={signature.id}
        doc={state.doc}
        hostedAllowed={limits?.hosted_gif ?? false}
        branding={limits?.branding ?? false}
      />
      <PublishModal
        open={modal === 'publish'}
        onClose={() => setModal(null)}
        signatureId={signature.id}
        publicSlug={publicSlug}
        hostedAllowed={limits?.hosted_gif ?? false}
        onPublished={setPublicSlug}
      />
    </div>
  );
}
