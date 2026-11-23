// Tiny toast queue. One global hook, one renderer; auto-dismiss after 4s.
// Used to surface action failures (start/stop/restart/delete) that would
// otherwise only land in the console.

import { useEffect, useState } from 'react';
import type { ThemeTokens } from './theme';
import { plainText } from './plainText';

type Toast = { id: number; message: string; kind: 'error' | 'info' };

let nextId = 1;
let listeners: ((t: Toast[]) => void)[] = [];
let queue: Toast[] = [];

function notify() {
  for (const l of listeners) l(queue);
}

export function toast(message: string, kind: 'error' | 'info' = 'error') {
  const t: Toast = { id: nextId++, message: plainText(message), kind };
  queue = [...queue, t];
  notify();
  setTimeout(() => {
    queue = queue.filter(x => x.id !== t.id);
    notify();
  }, 4000);
}

// Wrap an action invocation so failures auto-toast with a label.
export function withToast<T>(label: string, p: Promise<T>): Promise<T> {
  return p.catch(e => {
    toast(`${label} failed: ${typeof e === 'string' ? e : (e?.message ?? String(e))}`);
    throw e;
  });
}

export function ToastTray({ t }: { t: ThemeTokens }) {
  const [items, setItems] = useState<Toast[]>(queue);
  useEffect(() => {
    const fn = (q: Toast[]) => setItems(q);
    listeners.push(fn);
    return () => { listeners = listeners.filter(x => x !== fn); };
  }, []);
  // The polite live region stays mounted even when empty: screen readers only
  // announce additions to a region that already existed. Errors use role=alert
  // (assertive), info toasts role=status.
  return (
    <div aria-live="polite" aria-relevant="additions" style={{ position: 'absolute', bottom: 18, right: 18, display: 'flex', flexDirection: 'column', gap: 8, zIndex: 200, maxWidth: 420 }}>
      {items.map(item => (
        <div key={item.id} role={item.kind === 'error' ? 'alert' : 'status'} style={{
          padding: '10px 14px',
          background: t.surface,
          border: `1px solid ${item.kind === 'error' ? t.danger : t.border}`,
          borderLeft: `3px solid ${item.kind === 'error' ? t.danger : t.accent}`,
          borderRadius: 6,
          color: t.fg1,
          fontSize: 12,
          fontFamily: t.mono,
          boxShadow: '0 8px 24px rgba(0,0,0,0.25)',
        }}>{item.message}</div>
      ))}
    </div>
  );
}
