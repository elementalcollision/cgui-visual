import { useEffect, useState } from 'react';
import { api } from '../api';
import { Icon, Bar, iconBtn, pillBtn } from '../components';
import type { ThemeTokens } from '../theme';
import { Backdrop } from './shared';

export function PullModal({ t, reference, onClose }: {
  t: ThemeTokens;
  /** When the caller already knows what to pull (e.g. an "Update image"
   *  affordance from elsewhere), seed the input. Empty by default so
   *  the modal opens idle and the user types their own reference. */
  reference?: string;
  onClose: () => void;
}) {
  const [input, setInput] = useState(reference ?? '');
  const [submitted, setSubmitted] = useState<string | null>(null);
  const [progress, setProgress] = useState(0);
  const [lines, setLines] = useState<string[]>([]);
  const [done, setDone] = useState<boolean | null>(null);

  // Subscribe once. Backend events fire only after startPull, so it's
  // safe to attach listeners in the idle phase too — they just won't
  // see anything until the user clicks Pull.
  useEffect(() => {
    let cancelled = false;
    let unlistenLine: (() => void) | null = null;
    let unlistenDone: (() => void) | null = null;
    api.onPullLine(line => {
      if (cancelled) return;
      setLines(L => [...L, line]);
      const m = line.match(/(\d+)%/);
      if (m) setProgress(Number(m[1]));
    }).then(fn => { if (cancelled) fn(); else unlistenLine = fn; });
    api.onPullDone(ok => {
      if (cancelled) return;
      setDone(ok);
      if (ok) setProgress(100);
    }).then(fn => { if (cancelled) fn(); else unlistenDone = fn; });
    return () => { cancelled = true; unlistenLine?.(); unlistenDone?.(); };
  }, []);

  const startPull = () => {
    const ref = input.trim();
    if (!ref) return;
    setSubmitted(ref);
    setDone(null);
    setProgress(0);
    setLines([]);
    api.startPull(ref).catch(e => console.warn('start_pull failed', e));
  };

  const onKey = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') { e.preventDefault(); startPull(); }
  };

  const headerTitle = submitted == null ? 'Pull image'
    : done === true   ? 'Pulled'
    : done === false  ? 'Pull failed'
    :                   'Pulling image';

  return (
    <Backdrop title={headerTitle} onClose={onClose}>
      <div style={{ width: 640, background: t.surface, border: `1px solid ${t.border}`, borderRadius: 12, overflow: 'hidden', display: 'flex', flexDirection: 'column', boxShadow: '0 24px 80px rgba(0,0,0,0.4)' }}>
        <div style={{ padding: '16px 22px', borderBottom: `1px solid ${t.border}`, display: 'flex', alignItems: 'center', gap: 12 }}>
          <Icon name="download" size={18} color={t.accent} />
          <div style={{ flex: 1, minWidth: 0 }}>
            <div style={{ fontSize: 15, fontWeight: 600, color: t.fg1 }}>{headerTitle}</div>
            {submitted && (
              <div style={{ fontSize: 12, color: t.fg3, fontFamily: t.mono, marginTop: 2, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{submitted}</div>
            )}
          </div>
          <button onClick={onClose} style={iconBtn()} aria-label="Close"><Icon name="x" size={16} color={t.fg2} /></button>
        </div>

        {/* Idle phase: input + Pull. Once submitted we swap in the
            progress UI; the idle controls stay hidden because re-firing
            mid-pull would race the running child. */}
        {submitted == null ? (
          <div style={{ padding: '18px 22px', display: 'flex', flexDirection: 'column', gap: 10 }}>
            <label style={{ fontSize: 11, fontWeight: 600, letterSpacing: '0.08em', textTransform: 'uppercase', color: t.fg3 }}>
              Image reference
            </label>
            <input
              autoFocus
              value={input}
              onChange={e => setInput(e.target.value)}
              onKeyDown={onKey}
              placeholder="alpine:latest  or  ghcr.io/owner/repo:tag"
              spellCheck={false}
              style={{
                padding: '10px 12px',
                background: t.bg, color: t.fg1,
                border: `1px solid ${t.border}`, borderRadius: 6,
                fontFamily: t.mono, fontSize: 13, outline: 'none',
              }}
            />
            <div style={{ fontSize: 11, color: t.fg3, lineHeight: 1.5 }}>
              Pulls the image and its layers via <code style={{ fontFamily: t.mono, color: t.fg2 }}>container image pull</code>.
              Looking to import a stack from a <code style={{ fontFamily: t.mono, color: t.fg2 }}>docker-compose.yml</code>?
              Use <span style={{ color: t.fg2 }}>Stacks → Import compose</span> instead.
            </div>
          </div>
        ) : (
          <>
            <div style={{ padding: '14px 22px', borderBottom: `1px solid ${t.border}` }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 6, fontFamily: t.mono, fontSize: 12, color: t.fg2 }}>
                <span>{lines.length} lines</span><span>{progress}%</span>
              </div>
              <Bar pct={progress} color={done === false ? t.danger : t.accent} bg={t.surfaceAlt} h={8} />
            </div>
            <div style={{ flex: 1, overflow: 'auto', padding: 16, fontFamily: t.mono, fontSize: 11, lineHeight: 1.6, background: t.bg, maxHeight: 280 }}>
              {lines.map((l, i) => (
                <div key={i} style={{ color: l.includes('✓') ? t.success : l.includes('▸') ? t.fg2 : t.fg1, padding: '1px 0' }}>{l}</div>
              ))}
            </div>
          </>
        )}

        <div style={{ padding: '12px 22px', borderTop: `1px solid ${t.border}`, display: 'flex', justifyContent: 'space-between', alignItems: 'center', background: t.surfaceAlt }}>
          <span style={{ fontSize: 11, color: t.fg3, fontFamily: t.mono }}>
            {submitted == null ? '↵ pull' : 'Esc to background — pull continues'}
          </span>
          <div style={{ display: 'flex', gap: 8 }}>
            <button onClick={onClose} style={pillBtn(t)}>{submitted == null ? 'Cancel' : 'Background'}</button>
            {submitted == null && (
              <button
                onClick={startPull}
                disabled={!input.trim()}
                style={{
                  padding: '6px 14px',
                  background: t.fg1, color: t.bg, border: 'none', borderRadius: 6,
                  fontSize: 12, fontWeight: 500,
                  cursor: input.trim() ? 'pointer' : 'default',
                  opacity: input.trim() ? 1 : 0.5,
                }}
              >Pull</button>
            )}
          </div>
        </div>
      </div>
    </Backdrop>
  );
}
