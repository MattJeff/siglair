import { Crown, LockKeyhole } from 'lucide-react';
import { Link } from 'react-router-dom';
import { Button } from '../components/Button';
import { Modal } from '../components/Modal';
import s from './editor.module.css';

export const FREE_BRANDING_LABEL = 'Powered by siglair.com';

export function BrandingUpsell({ open, onClose }: { open: boolean; onClose: () => void }) {
  return (
    <Modal open={open} onClose={onClose} title="Retirer la mention Siglair">
      <div className={s.brandingUpsell}>
        <span className={s.brandingUpsellIcon} aria-hidden="true">
          <LockKeyhole size={22} />
        </span>
        <div>
          <h3>Une signature 100 % à votre marque</h3>
          <p>
            La mention reste fixée aux signatures gratuites. Pro la retire immédiatement de
            l’éditeur, des aperçus et des prochains exports.
          </p>
        </div>
        <div className={s.brandingSample}>
          <span>{FREE_BRANDING_LABEL}</span>
          <small>Inclus avec Free</small>
        </div>
        <div className={s.brandingUpsellActions}>
          <Link className={s.brandingUpgrade} to="/app/billing" onClick={onClose}>
            <Crown size={17} aria-hidden="true" />
            Passer à Pro
          </Link>
          <Button variant="ghost" onClick={onClose}>
            Garder la mention
          </Button>
        </div>
      </div>
    </Modal>
  );
}
