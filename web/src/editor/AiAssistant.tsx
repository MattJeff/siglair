import { useState } from 'react';
import type { Dispatch, FormEvent } from 'react';
import { ArrowUp, LockKeyhole, Sparkles, Undo2 } from 'lucide-react';
import { Link } from 'react-router-dom';
import { ApiError, editSignatureWithAi } from '../lib/api';
import { getAnalytics } from '../lib/analytics';
import type { Plan } from '../lib/types';
import type { Action } from './state';
import type { Autosave } from './useAutosave';
import s from './editor.module.css';

interface Message {
  id: number;
  role: 'user' | 'assistant';
  text: string;
}

const SUGGESTIONS = [
  'Rends cette signature plus premium et plus lisible.',
  'Ajoute un CTA animé pour prendre rendez-vous.',
  'Crée une version claire avec un dégradé de marque.',
];

export function AiAssistant({
  signatureId,
  selectedId,
  plan,
  providerAvailable,
  save,
  dispatch,
}: {
  signatureId: string;
  selectedId: string | null;
  plan: Plan | null;
  providerAvailable: boolean;
  save: Autosave;
  dispatch: Dispatch<Action>;
}) {
  const [prompt, setPrompt] = useState('');
  const [messages, setMessages] = useState<Message[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const paid = plan === 'pro' || plan === 'team';

  const send = async (event?: FormEvent, suggestion?: string) => {
    event?.preventDefault();
    const message = (suggestion ?? prompt).trim();
    if (!message || busy || !paid || !providerAvailable) return;
    setBusy(true);
    setError('');
    setPrompt('');
    setMessages((current) => [...current, { id: Date.now(), role: 'user', text: message }]);
    const startedAt = performance.now();
    const intent = selectedId ? 'selected_element' : 'full_canvas';
    getAnalytics().track('ai_prompt_submitted', {
      signature_id: signatureId,
      intent,
      prompt_length_bucket: message.length < 80 ? 'short' : message.length < 300 ? 'medium' : 'long',
    });
    try {
      await save.flush();
      const result = await editSignatureWithAi(signatureId, message, selectedId);
      dispatch({ type: 'replaceDoc', doc: result.doc });
      getAnalytics().track('ai_edit_completed', {
        signature_id: signatureId,
        intent,
        latency_ms: Math.round(performance.now() - startedAt),
        changed_elements: result.doc.elements.length,
      });
      setMessages((current) => [
        ...current,
        { id: Date.now() + 1, role: 'assistant', text: result.reply },
      ]);
    } catch (cause) {
      getAnalytics().track('ai_edit_failed', {
        signature_id: signatureId,
        intent,
        latency_ms: Math.round(performance.now() - startedAt),
        error_code: cause instanceof ApiError ? cause.code : 'unknown',
      });
      setError(cause instanceof ApiError ? cause.message : 'Le copilote ne répond pas pour l’instant.');
    } finally {
      setBusy(false);
    }
  };

  if (!paid) {
    return (
      <div className={s.aiGate}>
        <span className={s.aiGateIcon} aria-hidden="true">
          <LockKeyhole size={20} />
        </span>
        <h3>Copilote IA</h3>
        <p>Pro et Team peuvent créer, réorganiser, styliser et animer tout le canvas par message.</p>
        <Link className={s.aiUpgrade} to="/app/billing">
          Passer à Pro
        </Link>
      </div>
    );
  }

  return (
    <div className={s.aiPanel}>
      <div className={s.aiHead}>
        <span className={s.aiMark} aria-hidden="true">
          <Sparkles size={17} />
        </span>
        <div>
          <h3>Copilote IA</h3>
          <span>{selectedId ? 'Élément sélectionné pris en compte' : 'Canvas complet'}</span>
        </div>
      </div>

      {!providerAvailable && (
        <p className={`${s.aiNotice} ${s.aiError}`} role="alert">
          Le fournisseur IA n’est pas configuré sur cette instance.
        </p>
      )}

      <div className={s.aiMessages} aria-live="polite">
        {messages.length === 0 && providerAvailable && (
          <div className={s.aiWelcome}>
            <Sparkles size={18} aria-hidden="true" />
            <strong>Quelle direction voulez-vous ?</strong>
            <span>Le résultat s’applique au canvas et reste annulable.</span>
          </div>
        )}
        {messages.map((message) => (
          <p key={message.id} className={message.role === 'user' ? s.aiUser : s.aiReply}>
            {message.text}
          </p>
        ))}
        {busy && (
          <div className={s.aiThinking} role="status">
            <span />
            <span />
            <span />
            Composition en cours
          </div>
        )}
      </div>

      {messages.length === 0 && providerAvailable && (
        <div className={s.aiSuggestions}>
          {SUGGESTIONS.map((suggestion) => (
            <button key={suggestion} type="button" onClick={() => void send(undefined, suggestion)}>
              {suggestion}
            </button>
          ))}
        </div>
      )}

      {error && (
        <p className={`${s.aiNotice} ${s.aiError}`} role="alert">
          {error}
        </p>
      )}

      <form className={s.aiComposer} onSubmit={(event) => void send(event)}>
        <label className={s.srOnly} htmlFor="ai-prompt">
          Demande au copilote
        </label>
        <textarea
          id="ai-prompt"
          value={prompt}
          disabled={busy || !providerAvailable}
          maxLength={2000}
          rows={3}
          placeholder="Ex. Aligne le logo à gauche, ajoute mon LinkedIn et anime le CTA…"
          onChange={(event) => setPrompt(event.currentTarget.value)}
          onKeyDown={(event) => {
            if ((event.metaKey || event.ctrlKey) && event.key === 'Enter') void send();
          }}
        />
        <button type="submit" disabled={!prompt.trim() || busy || !providerAvailable} title="Appliquer">
          <ArrowUp size={17} />
          <span className={s.srOnly}>Appliquer</span>
        </button>
      </form>

      {messages.some((message) => message.role === 'assistant') && (
        <p className={s.aiUndo}>
          <Undo2 size={13} aria-hidden="true" /> Ctrl+Z annule la dernière proposition.
        </p>
      )}
    </div>
  );
}
