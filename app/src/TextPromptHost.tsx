// In-app replacement for window.prompt (see dialogs.ts for why). Kept out of
// the lazy modals chunk so it is available as soon as App mounts.
import { useEffect, useState } from 'react';
import type { ThemeTokens } from './theme';
import { pillBtn } from './components';
import { subscribeTextPrompt, settleTextPrompt, type TextRequest } from './dialogs';

// In-app replacement for window.prompt (see dialogs.ts for why). Mounted once
// in App; renders the request queued by askText(). Enter submits, Esc / the
// backdrop / Cancel resolve null.
export function TextPromptHost({ t }: { t: ThemeTokens }) {
  const [req, setReq] = useState<TextRequest | null>(null);
  useEffect(() => subscribeTextPrompt(setReq), []);
  // Keyed per request so the input starts at that request's default value.
  return req ? <PromptDialog key={reqKey(req)} t={t} req={req} /> : null;
}

const keys = new WeakMap<TextRequest, number>();
let nextKey = 1;
function reqKey(r: TextRequest) {
  if (!keys.has(r)) keys.set(r, nextKey++);
  return keys.get(r)!;
}

function PromptDialog({ t, req }: { t: ThemeTokens; req: TextRequest }) {
  const [value, setValue] = useState(req.defaultValue);
  const cancel = () => settleTextPrompt(null);
  return (
    <div onClick={cancel} style={{ position: 'absolute', inset: 0, background: 'rgba(0,0,0,0.45)', display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 300, backdropFilter: 'blur(2px)' }}>
      <div onClick={e => e.stopPropagation()}>
      <form
        role="dialog" aria-modal="true" aria-labelledby="text-prompt-msg"
        onSubmit={e => { e.preventDefault(); settleTextPrompt(value); }}
        onKeyDown={e => { if (e.key === 'Escape') { e.preventDefault(); cancel(); } }}
        style={{ width: 460, background: t.surface, border: `1px solid ${t.border}`, borderRadius: 12, padding: 20, boxShadow: '0 24px 80px rgba(0,0,0,0.4)' }}>
        <div id="text-prompt-msg" style={{ fontSize: 13, color: t.fg1, whiteSpace: 'pre-wrap', marginBottom: 12 }}>{req.message}</div>
        <input
          autoFocus aria-labelledby="text-prompt-msg" value={value}
          onChange={e => setValue(e.target.value)}
          onFocus={e => e.currentTarget.select()}
          style={{ width: '100%', boxSizing: 'border-box', padding: '8px 10px', fontSize: 13, fontFamily: t.mono, color: t.fg1, background: t.surfaceAlt, border: `1px solid ${t.border}`, borderRadius: 6, outline: 'none' }} />
        <div style={{ display: 'flex', gap: 8, justifyContent: 'flex-end', marginTop: 14 }}>
          <button type="button" style={pillBtn(t)} onClick={cancel}>Cancel</button>
          <button type="submit" style={pillBtn(t, t.accent)}>OK</button>
        </div>
      </form>
      </div>
    </div>
  );
}
