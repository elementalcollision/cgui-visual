// Shared "failed to load" / "nothing here" states for list views, so a CLI
// failure (e.g. the runtime's API server being stopped) is never rendered
// as an empty list.

import { useCallback, useEffect, useRef, useState } from 'react';
import type { ReactNode } from 'react';
import type { ThemeTokens } from './theme';
import { api } from './api';
import { Icon, pillBtn } from './components';
import { withToast } from './toast';
import { plainText } from './plainText';

export function errMessage(e: unknown): string {
  if (typeof e === 'string') return plainText(e);
  if (e && typeof (e as { message?: unknown }).message === 'string') return plainText((e as { message: string }).message);
  return plainText(String(e));
}

// Text Apple's CLI prints for every subcommand while the services are down:
// "Plugins are unavailable. Start the container system services and retry:
// container system start".
export function looksLikeServicesDown(message: string): boolean {
  return /plugins are unavailable|container system start/i.test(message);
}

/**
 * Load a resource and track failure separately from emptiness. `data` is
 * undefined until the first success; `error` is the message of the latest
 * failure (cleared on the next success). Failures never toast: the view
 * renders the state itself so repeated polls can't spam.
 */
export function useLoad<T>(fetcher: () => Promise<T>, reloadKey: unknown = 0) {
  const [data, setData] = useState<T | undefined>(undefined);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const seq = useRef(0);
  const fetchRef = useRef(fetcher);
  fetchRef.current = fetcher;

  const reload = useCallback(async () => {
    const mine = ++seq.current;
    setLoading(true);
    try {
      const v = await fetchRef.current();
      if (mine !== seq.current) return;
      setData(v);
      setError(null);
    } catch (e) {
      if (mine !== seq.current) return;
      setError(errMessage(e));
    } finally {
      if (mine === seq.current) setLoading(false);
    }
  }, []);

  useEffect(() => { reload(); return () => { seq.current++; }; }, [reload, reloadKey]);
  return { data, error, loading, reload };
}

export function EmptyState({ t, children }: { t: ThemeTokens; children: ReactNode }) {
  return (
    <div role="status" style={{ padding: '56px 24px', textAlign: 'center', color: t.fg3, fontSize: 13 }}>
      {children}
    </div>
  );
}

/**
 * Explicit failure state. When the runtime services are stopped (detected
 * from the CLI's message, or via the existing system_status probe) it says
 * so and offers "Start services"; otherwise it shows the CLI's message.
 */
export function LoadErrorState({ t, message, onRetry }: {
  t: ThemeTokens; message: string; onRetry: () => void | Promise<unknown>;
}) {
  const [down, setDown] = useState(() => looksLikeServicesDown(message));
  const [starting, setStarting] = useState(false);

  useEffect(() => {
    if (looksLikeServicesDown(message)) { setDown(true); return; }
    setDown(false);
    let cancelled = false;
    (async () => {
      try {
        if (await api.runtimeAvailable() && !(await api.systemStatus()) && !cancelled) setDown(true);
      } catch { /* probe failed; keep the generic error */ }
    })();
    return () => { cancelled = true; };
  }, [message]);

  const start = () => {
    setStarting(true);
    withToast('start services', api.systemStart())
      .then(() => onRetry())
      .catch(() => {})
      .finally(() => setStarting(false));
  };

  return (
    <div role="alert" style={{ padding: '56px 24px', textAlign: 'center' }}>
      <div style={{ display: 'inline-flex', alignItems: 'center', gap: 8, fontSize: 15, fontWeight: 600, color: t.fg1 }}>
        <Icon name="info" size={16} color={down ? t.warning : t.danger} />
        {down ? "Container services aren't running" : "Couldn't load data from the container runtime"}
      </div>
      <div style={{ marginTop: 8, fontSize: 12, color: t.fg3, maxWidth: 520, marginLeft: 'auto', marginRight: 'auto' }}>
        {down
          ? 'Start the runtime services to see your containers, images, volumes and networks.'
          : 'This is a failure, not an empty list.'}
      </div>
      {!down && (
        <pre style={{ margin: '12px auto 0', maxWidth: 640, whiteSpace: 'pre-wrap', textAlign: 'left', fontFamily: t.mono, fontSize: 11, color: t.fg2, background: t.surfaceAlt, border: `1px solid ${t.border}`, borderRadius: 6, padding: '8px 10px' }}>{message}</pre>
      )}
      <div style={{ marginTop: 16, display: 'inline-flex', gap: 8 }}>
        {down && (
          <button style={{ ...pillBtn(t, t.success), opacity: starting ? 0.5 : 1 }} disabled={starting} onClick={start}>
            <Icon name="play" size={12} color={t.success} />Start services
          </button>
        )}
        <button style={pillBtn(t)} onClick={() => { void onRetry(); }}>Retry</button>
      </div>
    </div>
  );
}
