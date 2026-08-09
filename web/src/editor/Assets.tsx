/**
 * Bibliothèque de médias. Le document ne stocke qu'un `assetId` (contrat §3) :
 * plus aucune data-URL, l'octet vit dans /api/assets et est servi par URL.
 */
import { useRef, useState } from 'react';
import type { Dispatch } from 'react';
import { Button } from '../components/Button';
import { useToast } from '../components/Toast';
import { formatBytes } from '../components/app/helpers';
import { ApiError, deleteAsset, listAssets, uploadAsset } from '../lib/api';
import { useSession } from '../lib/session';
import type { Asset } from '../lib/types';
import type { Action } from './state';
import s from './editor.module.css';

interface AssetsProps {
  assets: Asset[];
  onAssetsChange: (assets: Asset[]) => void;
  dispatch: Dispatch<Action>;
}

export function Assets({ assets, onAssetsChange, dispatch }: AssetsProps) {
  const toast = useToast();
  const { limits, usage, refresh } = useSession();
  const imageInput = useRef<HTMLInputElement>(null);
  const videoInput = useRef<HTMLInputElement>(null);
  const [busy, setBusy] = useState(false);

  const used = usage?.assets_bytes ?? 0;
  const allowed = limits?.assets_bytes ?? 0;
  const ratio = allowed > 0 ? Math.min(1, used / allowed) : 0;
  const full = allowed > 0 && used >= allowed;

  async function upload(file: File | undefined) {
    if (!file) return;
    setBusy(true);
    try {
      const asset = await uploadAsset(file);
      onAssetsChange([asset, ...assets]);
      await refresh();
      toast('Média ajouté à la bibliothèque', 'success');
    } catch (error) {
      toast(error instanceof ApiError ? error.message : "L'envoi du média a échoué.", 'error');
    } finally {
      setBusy(false);
    }
  }

  async function remove(asset: Asset) {
    try {
      await deleteAsset(asset.id);
      onAssetsChange(await listAssets());
      await refresh();
      toast('Média supprimé', 'success');
    } catch (error) {
      // 409 : l'asset est utilisé par une signature. Le message serveur est déjà en français.
      toast(error instanceof ApiError ? error.message : 'Suppression impossible.', 'error');
    }
  }

  return (
    <>
      <div className={s.sectionHead}>
        <h3>Bibliothèque</h3>
        <span>{assets.length}</span>
      </div>

      <div className={s.stack}>
        {/* Aucune taille maximale écrite ici : la limite par fichier n'est pas servie par
            l'API, et un chiffre en dur finirait par mentir. Le serveur renvoie son message. */}
        <p className={s.note}>PNG, JPG, GIF et vidéo. Le GIF reste animé dans l’aperçu.</p>
        <div className={s.row2}>
          <Button variant="ghost" loading={busy} disabled={full} onClick={() => imageInput.current?.click()}>
            Image / GIF
          </Button>
          <Button variant="ghost" loading={busy} disabled={full} onClick={() => videoInput.current?.click()}>
            Vidéo
          </Button>
        </div>
        <input
          ref={imageInput}
          type="file"
          accept="image/*"
          hidden
          onChange={(event) => {
            void upload(event.target.files?.[0]);
            event.target.value = '';
          }}
        />
        <input
          ref={videoInput}
          type="file"
          accept="video/*"
          hidden
          onChange={(event) => {
            void upload(event.target.files?.[0]);
            event.target.value = '';
          }}
        />

        {limits && (
          <div>
            <div className={s.sectionHead}>
              <span>
                {formatBytes(used)} sur {formatBytes(allowed)}
              </span>
              {full && <span>Espace plein</span>}
            </div>
            <span className={s.quota}>
              <span
                className={[s.quotaFill, full ? s.quotaFull : null].filter(Boolean).join(' ')}
                style={{ width: `${Math.round(ratio * 100)}%` }}
              />
            </span>
            {full && (
              <p className={`${s.note} ${s.noteWarn} ${s.gap}`}>
                Votre espace média est plein. Supprimez un fichier, ou passez à un plan supérieur
                depuis la page Abonnement.
              </p>
            )}
          </div>
        )}
      </div>

      <div className={`${s.sectionHead} ${s.gap}`}>
        <h3>Médias</h3>
        <span>cliquez pour insérer</span>
      </div>

      {assets.length === 0 ? (
        <p className={s.note}>Aucun média pour l’instant.</p>
      ) : (
        <div className={s.assetList}>
          {assets.map((asset) => (
            <div key={asset.id} className={s.asset}>
              <button
                type="button"
                className={s.assetPick}
                onClick={() => dispatch({ type: 'add', kind: asset.kind, assetId: asset.id })}
              >
                <span className={s.srOnly}>Insérer {asset.filename}</span>
                {asset.kind === 'video' ? (
                  <video src={asset.url} muted playsInline />
                ) : (
                  <img src={asset.url} alt="" />
                )}
              </button>
              <span className={s.assetKind} aria-hidden="true">
                {asset.kind === 'video' ? 'VIDÉO' : 'IMG'}
              </span>
              <button
                type="button"
                className={s.assetRemove}
                aria-label={`Supprimer ${asset.filename}`}
                onClick={() => void remove(asset)}
              >
                ✕
              </button>
            </div>
          ))}
        </div>
      )}
    </>
  );
}
