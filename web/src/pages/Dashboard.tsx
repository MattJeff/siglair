import { useCallback, useEffect, useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { Button } from '../components/Button';
import { Field } from '../components/Field';
import { Modal } from '../components/Modal';
import { Select } from '../components/Select';
import { Spinner } from '../components/Spinner';
import { useToast } from '../components/Toast';
import { AppShell } from '../components/app/AppShell';
import { ConfirmDialog } from '../components/app/ConfirmDialog';
import { PENDING_NEXT, apiMessage, formatDate } from '../components/app/helpers';
// Les modèles sont ceux de l'éditeur : une seconde table de modèles finirait par diverger.
import { TEMPLATES, docFromTemplate } from '../editor/templates';
import type { Template } from '../editor/templates';
import {
  createSignature,
  deleteSignature,
  duplicateSignature,
  exportSignature,
  getSignatureStatus,
  listSignatures,
  signatureThumbUrl,
} from '../lib/api';
import { useSession } from '../lib/session';
import type { ExportMode, Signature, SignatureStatus } from '../lib/types';
import s from './app.module.css';

const POLL_MS = 2000;
/** Au-delà, on arrête de sonder : un job bloqué ne doit pas faire tourner l'onglet à vie. */
const POLL_MAX_MS = 120_000;

type Visual = 'draft' | 'publishing' | 'published' | 'failed' | 'stalled';

function visualState(
  sig: Signature,
  status: SignatureStatus | undefined,
  stalled: boolean,
): Visual {
  const job = status?.job?.status;
  if (job === 'failed') return 'failed';
  if (job === 'queued' || job === 'running') return stalled ? 'stalled' : 'publishing';
  if (sig.published_render_id !== null) return 'published';
  // Un slug sans rendu : la publication a été lancée, on ne sait pas encore où elle en est.
  if (sig.public_slug !== null) return stalled ? 'stalled' : 'publishing';
  return 'draft';
}

const BADGE: Record<Visual, { label: string; cls: string }> = {
  draft: { label: 'Brouillon', cls: '' },
  publishing: { label: 'Publication en cours', cls: 'badgeWork' },
  published: { label: 'Publiée', cls: 'badgeOk' },
  failed: { label: 'Échec de publication', cls: 'badgeErr' },
  stalled: { label: 'Publication très longue', cls: 'badgeWarn' },
};

export default function Dashboard() {
  const { limits, usage, refresh } = useSession();
  const navigate = useNavigate();
  const toast = useToast();

  const [items, setItems] = useState<Signature[] | null>(null);
  const [loadError, setLoadError] = useState('');
  const [statuses, setStatuses] = useState<Record<string, SignatureStatus>>({});
  const [stalled, setStalled] = useState(false);
  const [creating, setCreating] = useState(false);
  const [busyId, setBusyId] = useState('');
  const [embed, setEmbed] = useState<Signature | null>(null);
  const [toDelete, setToDelete] = useState<Signature | null>(null);

  // Destination mémorisée avant la connexion (le serveur ramène toujours sur /app).
  useEffect(() => {
    const next = sessionStorage.getItem(PENDING_NEXT);
    if (!next) return;
    sessionStorage.removeItem(PENDING_NEXT);
    if (next !== '/app') navigate(next, { replace: true });
  }, [navigate]);

  const load = useCallback(async () => {
    try {
      setItems(await listSignatures());
      setLoadError('');
    } catch (e) {
      setLoadError(apiMessage(e));
      setItems([]);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const pendingIds = (items ?? [])
    .filter((sig) => {
      const job = statuses[sig.id]?.job?.status;
      if (job === 'done' || job === 'failed') return false;
      if (job === 'queued' || job === 'running') return true;
      return sig.public_slug !== null && sig.published_render_id === null;
    })
    .map((sig) => sig.id);
  const pendingKey = pendingIds.join(',');

  useEffect(() => {
    const ids = pendingKey.split(',').filter(Boolean);
    if (ids.length === 0) {
      setStalled(false);
      return;
    }
    let alive = true;
    const startedAt = Date.now();

    const tick = async () => {
      const pairs = await Promise.all(
        ids.map(async (id) => [id, await getSignatureStatus(id).catch(() => null)] as const),
      );
      if (!alive) return;
      let finished = false;
      setStatuses((prev) => {
        const next = { ...prev };
        for (const [id, st] of pairs) {
          if (!st) continue;
          if (st.job?.status === 'done' || st.render !== null) finished = true;
          next[id] = st;
        }
        return next;
      });
      // Un job terminé change la vignette et l'état publié : on relit la liste.
      if (finished) void load();
    };

    void tick();
    const timer = window.setInterval(() => {
      if (Date.now() - startedAt > POLL_MAX_MS) {
        window.clearInterval(timer);
        setStalled(true);
        return;
      }
      void tick();
    }, POLL_MS);

    return () => {
      alive = false;
      window.clearInterval(timer);
    };
  }, [pendingKey, load]);

  const create = async (template?: Template) => {
    setCreating(true);
    try {
      const sig = await createSignature(
        template
          ? { name: template.name, doc: docFromTemplate(template) }
          : { name: 'Nouvelle signature' },
      );
      await refresh();
      navigate(`/app/editor/${sig.id}`);
    } catch (e) {
      toast(apiMessage(e), 'error');
    } finally {
      setCreating(false);
    }
  };

  const duplicate = async (sig: Signature) => {
    setBusyId(sig.id);
    try {
      await duplicateSignature(sig.id);
      await load();
      await refresh();
      toast('Copie créée.', 'success');
    } catch (e) {
      toast(apiMessage(e), 'error');
    } finally {
      setBusyId('');
    }
  };

  const confirmDelete = async () => {
    if (!toDelete) return;
    setBusyId(toDelete.id);
    try {
      await deleteSignature(toDelete.id);
      setToDelete(null);
      await load();
      await refresh();
      toast('Signature supprimée.', 'success');
    } catch (e) {
      toast(apiMessage(e), 'error');
    } finally {
      setBusyId('');
    }
  };

  const atQuota =
    limits !== null &&
    usage !== null &&
    limits.signatures !== null &&
    usage.signatures >= limits.signatures;

  return (
    <AppShell
      title="Mes signatures"
      subtitle={
        limits && usage
          ? `${usage.signatures} signature${usage.signatures > 1 ? 's' : ''} sur ${
              limits.signatures === null ? 'un nombre illimité' : limits.signatures
            }`
          : undefined
      }
      actions={
        <Button loading={creating} onClick={() => void create()}>
          Nouvelle signature
        </Button>
      }
    >
      <div className={s.stack}>
        {atQuota && (
          <div className={`${s.notice} ${s.upsell}`}>
            <span>
              Votre plan comprend {limits?.signatures} signature
              {(limits?.signatures ?? 0) > 1 ? 's' : ''}. Passez au plan supérieur pour en créer
              d’autres — et obtenir l’URL hébergée qui se met à jour toute seule.
            </span>
            <Link className={s.linkBtn} to="/app/billing">
              Voir les plans
            </Link>
          </div>
        )}

        {loadError && (
          <p className={`${s.notice} ${s.alert}`} role="alert">
            {loadError}
          </p>
        )}

        {items === null && (
          <div className={s.center}>
            <Spinner size={28} label="Chargement des signatures" />
          </div>
        )}

        {items !== null && items.length === 0 && !loadError && (
          <div className={s.empty}>
            <h2 className={s.emptyTitle}>Aucune signature pour l’instant</h2>
            <p className={s.muted}>
              Choisissez un modèle : il s’ouvre dans l’éditeur, tout y est déplaçable,
              recolorable et réanimable.
            </p>
            <div className={s.templates}>
              {TEMPLATES.map((t) => (
                <button
                  key={t.id}
                  type="button"
                  className={s.templateCard}
                  disabled={creating}
                  onClick={() => void create(t)}
                >
                  <span className={s.templateThumb} style={{ background: t.thumb }} />
                  <span className={s.rowName}>{t.name}</span>
                  <span className={s.muted}>{t.description}</span>
                </button>
              ))}
            </div>
            <p className={s.muted}>Ou partez de votre marque, sans page blanche :</p>
            <div className={s.actions}>
              <a className={s.linkBtn} href="/">
                Partir de mon site web
              </a>
              <Button variant="ghost" loading={creating} onClick={() => void create()}>
                Signature vierge
              </Button>
            </div>
          </div>
        )}

        {items !== null && items.length > 0 && (
          <ul className={`${s.rows} ${s.plainList}`}>
            {items.map((sig) => {
              const state = visualState(sig, statuses[sig.id], stalled);
              const badge = BADGE[state];
              const error = statuses[sig.id]?.job?.error;
              return (
                <li className={s.row} key={sig.id}>
                  {sig.public_slug && sig.published_render_id ? (
                    <img
                      className={s.thumb}
                      src={signatureThumbUrl(sig.id)}
                      alt=""
                      loading="lazy"
                      width={136}
                      height={58}
                    />
                  ) : (
                    <div className={`${s.thumb} ${s.thumbEmpty}`} aria-hidden="true">
                      {state === 'publishing' ? <Spinner size={16} /> : 'Non publiée'}
                    </div>
                  )}

                  <div className={s.rowMain}>
                    <p className={s.rowName}>{sig.name}</p>
                    <p className={s.muted}>
                      <span className={`${s.badge} ${badge.cls ? s[badge.cls] : ''}`}>
                        {badge.label}
                      </span>{' '}
                      Modifiée le {formatDate(sig.updated_at)}
                      {sig.kind === 'org_template' && ' · Modèle d’équipe'}
                    </p>
                    {state === 'failed' && error && (
                      <p className={s.muted} role="alert">
                        {error} Corrigez la signature puis republiez-la depuis l’éditeur.
                      </p>
                    )}
                    {state === 'stalled' && (
                      <p className={s.muted}>
                        Le rendu prend plus de deux minutes. Rechargez la page pour en avoir le
                        cœur net.
                      </p>
                    )}
                  </div>

                  <div className={s.actions}>
                    <Button onClick={() => setEmbed(sig)}>Copier pour ma messagerie</Button>
                    <Link className={s.linkBtn} to={`/app/editor/${sig.id}`}>
                      Éditer
                    </Link>
                    {/* Même règle que l'éditeur : sans rétention servie par l'API, la page
                        Statistiques n'a rien à montrer. Un seul comportement dans les deux zones. */}
                    {(limits?.analytics_days ?? 0) > 0 && (
                      <Link className={s.linkBtn} to={`/app/analytics/${sig.id}`}>
                        Statistiques
                      </Link>
                    )}
                    <Button
                      variant="ghost"
                      loading={busyId === sig.id}
                      onClick={() => void duplicate(sig)}
                    >
                      Dupliquer
                    </Button>
                    <Button variant="danger" onClick={() => setToDelete(sig)}>
                      Supprimer
                    </Button>
                  </div>
                </li>
              );
            })}
          </ul>
        )}
      </div>

      <EmbedModal
        signature={embed}
        onClose={() => setEmbed(null)}
        hostedAllowed={limits?.hosted_gif === true}
      />

      <ConfirmDialog
        open={toDelete !== null}
        title="Supprimer cette signature ?"
        confirmLabel="Supprimer"
        danger
        loading={busyId !== '' && busyId === toDelete?.id}
        onClose={() => setToDelete(null)}
        onConfirm={() => void confirmDelete()}
      >
        <p>
          « {toDelete?.name} » sera supprimée. Si elle est publiée, son URL cessera de répondre :
          la signature disparaîtra aussi des emails déjà envoyés qui la chargent.
        </p>
      </ConfirmDialog>
    </AppShell>
  );
}

/* ------------------------------------------------------------------ */
/* Modale « coller dans ma messagerie »                                */
/* ------------------------------------------------------------------ */

interface Client {
  id: string;
  name: string;
  mode: ExportMode;
  steps: string[];
  note: string;
}

const CLIENTS: Client[] = [
  {
    id: 'gmail',
    name: 'Gmail',
    mode: 'hosted',
    steps: [
      'Ouvrez Gmail, cliquez sur la roue dentée en haut à droite puis sur « Voir tous les paramètres ».',
      'Onglet « Général », descendez jusqu’à la section « Signature », créez-en une ou sélectionnez-la.',
      'Cliquez dans le cadre de la signature et collez (Ctrl+V, ou Cmd+V sur Mac).',
      'Descendez tout en bas de la page et cliquez sur « Enregistrer les modifications ».',
    ],
    note: 'Gmail affiche les GIF animés. Avec l’URL hébergée, changer de signature plus tard ne demande pas de revenir ici.',
  },
  {
    id: 'outlook',
    name: 'Outlook',
    mode: 'safe',
    steps: [
      'Outlook sur le web : roue dentée → « Courrier » → « Rédiger et répondre ».',
      'Outlook classique (Windows) : « Fichier » → « Options » → « Courrier » → « Signatures… ».',
      'Collez dans l’éditeur de signature (Ctrl+V).',
      'Enregistrez, puis envoyez-vous un email de test.',
    ],
    note: 'Outlook classique utilise le moteur de rendu de Word : il affiche la première image, fixe. C’est prévu, et c’est pour ça que l’export simplifié existe.',
  },
  {
    id: 'apple',
    name: 'Apple Mail',
    mode: 'hosted',
    steps: [
      'Mail → « Réglages… » → onglet « Signatures ».',
      'Sélectionnez votre compte à gauche, puis cliquez sur « + » pour créer une signature.',
      'Décochez « Toujours utiliser la police par défaut » sous la zone de saisie.',
      'Collez la signature (Cmd+V), puis fermez la fenêtre.',
    ],
    note: 'Apple Mail affiche les GIF animés. La protection de la confidentialité d’Apple précharge les images : vos statistiques d’ouverture y sont surestimées.',
  },
];

const MODE_LABEL: Record<ExportMode, string> = {
  hosted: 'URL hébergée — recommandé, se met à jour sans recoller',
  freeform: 'HTML complet — mise en page dans l’email',
  safe: 'HTML simplifié — compatible Outlook classique',
};

function EmbedModal({
  signature,
  onClose,
  hostedAllowed,
}: {
  signature: Signature | null;
  onClose: () => void;
  hostedAllowed: boolean;
}) {
  const toast = useToast();
  const [clientId, setClientId] = useState(CLIENTS[0].id);
  const [mode, setMode] = useState<ExportMode>(CLIENTS[0].mode);
  const [html, setHtml] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');

  const client = CLIENTS.find((c) => c.id === clientId) ?? CLIENTS[0];
  const published = signature !== null && signature.published_render_id !== null;
  const hostedBlocked = mode === 'hosted' && (!hostedAllowed || !published);

  const id = signature?.id ?? '';
  useEffect(() => {
    if (!id || hostedBlocked) {
      setHtml('');
      return;
    }
    let alive = true;
    setLoading(true);
    setError('');
    exportSignature(id, mode)
      .then((r) => {
        if (alive) setHtml(r.html);
      })
      .catch((e: unknown) => {
        if (alive) setError(apiMessage(e));
      })
      .finally(() => {
        if (alive) setLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [id, mode, hostedBlocked]);

  const pick = (next: Client) => {
    setClientId(next.id);
    setMode(next.mode === 'hosted' && (!hostedAllowed || !published) ? 'freeform' : next.mode);
  };

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(html);
      toast('Signature copiée. Collez-la dans votre messagerie.', 'success');
    } catch {
      toast('Copie automatique refusée par le navigateur : sélectionnez le code et copiez-le.', 'error');
    }
  };

  return (
    <Modal
      open={signature !== null}
      onClose={onClose}
      title={`Coller « ${signature?.name ?? ''} » dans ma messagerie`}
    >
      <div className={s.stack}>
        <div className={s.tabs} role="group" aria-label="Client de messagerie">
          {CLIENTS.map((c) => (
            <button
              key={c.id}
              type="button"
              className={s.tab}
              aria-pressed={c.id === clientId}
              onClick={() => pick(c)}
            >
              {c.name}
            </button>
          ))}
        </div>

        <Field label="Format d’export" hint={client.note}>
          <Select value={mode} onChange={(e) => setMode(e.currentTarget.value as ExportMode)}>
            {(Object.keys(MODE_LABEL) as ExportMode[]).map((m) => (
              <option key={m} value={m}>
                {MODE_LABEL[m]}
              </option>
            ))}
          </Select>
        </Field>

        <ol className={s.steps}>
          {client.steps.map((step) => (
            <li key={step}>{step}</li>
          ))}
        </ol>

        {hostedBlocked ? (
          <p className={`${s.notice} ${s.upsell}`}>
            {published
              ? 'L’URL hébergée fait partie des plans payants. C’est elle qui permet de changer de signature sans jamais recoller le code.'
              : 'Publiez d’abord cette signature depuis l’éditeur : l’URL hébergée est créée à la publication.'}
          </p>
        ) : (
          <>
            {loading && <Spinner size={20} label="Préparation du code" />}
            {error && (
              <p className={`${s.notice} ${s.alert}`} role="alert">
                {error}
              </p>
            )}
            {!loading && !error && (
              <>
                <label className={s.muted} htmlFor="embed-html">
                  Code à coller
                </label>
                <textarea
                  id="embed-html"
                  className={s.code}
                  value={html}
                  readOnly
                  spellCheck={false}
                  onFocus={(e) => e.currentTarget.select()}
                />
                <Button onClick={() => void copy()}>Copier le code</Button>
              </>
            )}
          </>
        )}
      </div>
    </Modal>
  );
}
