import type { SignatureStatus } from '../lib/types';

export type PublicationPhase = 'idle' | 'checking' | 'queued' | 'rendering' | 'done' | 'error';

/** L'échec du dernier job prime sur un ancien rendu encore en ligne. */
export function publicationPhase(status: SignatureStatus): PublicationPhase {
  if (status.job?.status === 'failed') return 'error';
  if (status.job?.status === 'queued') return 'queued';
  if (status.job?.status === 'running') return 'rendering';
  if (status.render !== null) return 'done';
  return 'idle';
}
