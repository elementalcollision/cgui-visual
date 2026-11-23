import { useEffect, useRef, useState } from 'react';
import { api } from '../api';
import { Icon, iconBtn, pillBtn } from '../components';
import { looksLikeServicesDown } from '../loadState';
import type { ThemeTokens } from '../theme';
import { withToast } from '../toast';
import { Backdrop } from './shared';

// Dockerfile build (phase 3). Same idle → streaming state machine as
// PullModal; the build runs in the builder VM (started on demand by
// the CLI) and streams plain-progress lines over build:tick.
export function BuildModal({ t, onClose }: { t: ThemeTokens; onClose: () => void }) {
  const [context, setContext] = useState('');
  const [dockerfile, setDockerfile] = useState('');
  const [tag, setTag] = useState('');
  const [buildArgs, setBuildArgs] = useState('');
  const [target, setTarget] = useState('');
  const [noCache, setNoCache] = useState(false);
  const [submitted, setSubmitted] = useState(false);
  const [lines, setLines] = useState<string[]>([]);
  const [done, setDone] = useState<boolean | null>(null);
  const logRef = useRef<HTMLDivElement>(null);
  const [starting, setStarting] = useState(false);
  // On 1.6+ `container build` is a CLI plugin and fails with "Plugins are
  // unavailable ..." while the services are stopped.
  const servicesDown = done === false && lines.some(looksLikeServicesDown);
  const startServices = () => {
    setStarting(true);
    withToast('start services', api.systemStart())
      .then(() => { setSubmitted(false); })
      .catch(() => {})
      .finally(() => setStarting(false));
  };

  useEffect(() => {
    let cancelled = false;
    let unLine: (() => void) | null = null;
    let unDone: (() => void) | null = null;
    api.onBuildLine(line => {
      if (cancelled) return;
      setLines(L => [...L, line]);
    }).then(fn => { if (cancelled) fn(); else unLine = fn; });
    api.onBuildDone(ok => { if (!cancelled) setDone(ok); })
      .then(fn => { if (cancelled) fn(); else unDone = fn; });
    return () => { cancelled = true; unLine?.(); unDone?.(); };
  }, []);

  // Follow the tail as lines stream in.
  useEffect(() => {
    logRef.current?.scrollTo?.({ top: logRef.current.scrollHeight });
  }, [lines]);

  const pickContext = async () => {
    const { open } = await import('@tauri-apps/plugin-dialog');
    const dir = await open({ directory: true, multiple: false, title: 'Build context folder' });
    if (typeof dir === 'string') setContext(dir);
  };
  const pickDockerfile = async () => {
    const { open } = await import('@tauri-apps/plugin-dialog');
    const f = await open({ multiple: false, title: 'Dockerfile' });
    if (typeof f === 'string') setDockerfile(f);
  };

  const canBuild = context.trim() !== '' && tag.trim() !== '';
  const startBuild = () => {
    if (!canBuild) return;
    setSubmitted(true);
    setDone(null);
    setLines([]);
    api.startBuild({
      context: context.trim(),
      tag: tag.trim(),
      dockerfile: dockerfile.trim() || undefined,
      buildArgs: buildArgs.split('\n').map(s => s.trim()).filter(Boolean),
      target: target.trim() || undefined,
      noCache,
    }).catch(e => { setDone(false); setLines(L => [...L, String(e)]); });
  };

  const headerTitle = !submitted ? 'Build image'
    : done === true  ? 'Build succeeded'
    : done === false ? 'Build failed'
    :                  'Building…';

  const field = (label: string, value: string, set: (s: string) => void, placeholder: string, picker?: () => void) => (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
      <label style={{ fontSize: 11, fontWeight: 600, letterSpacing: '0.08em', textTransform: 'uppercase', color: t.fg3 }}>{label}</label>
      <div style={{ display: 'flex', gap: 8 }}>
        <input value={value} onChange={e => set(e.target.value)} placeholder={placeholder} spellCheck={false}
          style={{ flex: 1, padding: '8px 12px', background: t.bg, color: t.fg1, border: `1px solid ${t.border}`, borderRadius: 6, fontFamily: t.mono, fontSize: 12, outline: 'none' }} />
        {picker && <button style={pillBtn(t)} onClick={picker}>Browse…</button>}
      </div>
    </div>
  );

  return (
    <Backdrop title={headerTitle} onClose={onClose}>
      <div style={{ width: 680, background: t.surface, border: `1px solid ${t.border}`, borderRadius: 12, overflow: 'hidden', display: 'flex', flexDirection: 'column', boxShadow: '0 24px 80px rgba(0,0,0,0.4)' }}>
        <div style={{ padding: '16px 22px', borderBottom: `1px solid ${t.border}`, display: 'flex', alignItems: 'center', gap: 12 }}>
          <Icon name="layers" size={18} color={done === false ? t.danger : t.accent} />
          <div style={{ flex: 1, minWidth: 0 }}>
            <div style={{ fontSize: 15, fontWeight: 600, color: t.fg1 }}>{headerTitle}</div>
            {submitted && tag && (
              <div style={{ fontSize: 12, color: t.fg3, fontFamily: t.mono, marginTop: 2 }}>{tag} · {context}</div>
            )}
          </div>
          <button onClick={onClose} style={iconBtn()} aria-label="Close"><Icon name="x" size={16} color={t.fg2} /></button>
        </div>

        {!submitted ? (
          <div style={{ padding: '18px 22px', display: 'flex', flexDirection: 'column', gap: 14, maxHeight: '60vh', overflow: 'auto' }}>
            {field('Build context', context, setContext, '/path/to/project', pickContext)}
            {field('Tag', tag, setTag, 'myapp:latest')}
            {field('Dockerfile (optional)', dockerfile, setDockerfile, 'defaults to <context>/Dockerfile', pickDockerfile)}
            {field('Target stage (optional)', target, setTarget, 'multi-stage build target')}
            <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
              <label style={{ fontSize: 11, fontWeight: 600, letterSpacing: '0.08em', textTransform: 'uppercase', color: t.fg3 }}>Build args (one KEY=value per line)</label>
              <textarea value={buildArgs} onChange={e => setBuildArgs(e.target.value)} rows={3} spellCheck={false}
                placeholder={'VERSION=1.2.3\nGIT_SHA=abc123'}
                style={{ padding: '8px 12px', background: t.bg, color: t.fg1, border: `1px solid ${t.border}`, borderRadius: 6, fontFamily: t.mono, fontSize: 12, outline: 'none', resize: 'vertical' }} />
            </div>
            <label style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 13, color: t.fg1, cursor: 'pointer' }}>
              <input type="checkbox" checked={noCache} onChange={e => setNoCache(e.target.checked)} style={{ accentColor: t.accent }} />
              No cache
            </label>
            <div style={{ fontSize: 11, color: t.fg3, lineHeight: 1.5 }}>
              Runs <code style={{ fontFamily: t.mono, color: t.fg2 }}>container build</code> in the builder VM.
              The CLI starts the builder automatically on first build (Settings → Builder shows its state).
            </div>
          </div>
        ) : (
          <div ref={logRef} style={{ flex: 1, overflow: 'auto', padding: 16, fontFamily: t.mono, fontSize: 11, lineHeight: 1.6, background: t.bg, maxHeight: 360, minHeight: 200 }}>
            {lines.map((l, i) => (
              <div key={i} style={{ color: /error|fail/i.test(l) ? t.danger : t.fg1, padding: '1px 0', whiteSpace: 'pre-wrap', wordBreak: 'break-all' }}>{l}</div>
            ))}
            {done === null && <div style={{ color: t.fg3 }}>▍</div>}
            {servicesDown && (
              <div role="alert" style={{ marginTop: 10, fontFamily: 'inherit', color: t.warning }}>
                Container services aren't running — builds need them.
              </div>
            )}
          </div>
        )}

        <div style={{ padding: '12px 22px', borderTop: `1px solid ${t.border}`, display: 'flex', justifyContent: 'space-between', alignItems: 'center', background: t.surfaceAlt }}>
          <span style={{ fontSize: 11, color: t.fg3, fontFamily: t.mono }}>
            {!submitted ? `${lines.length ? lines.length + ' lines from last run · ' : ''}context + tag required`
              : done === null ? 'Esc to background — build continues'
              : done ? 'Image is now in the Images tab' : `${lines.length} lines`}
          </span>
          <div style={{ display: 'flex', gap: 8 }}>
            {servicesDown && (
              <button style={{ ...pillBtn(t, t.success), opacity: starting ? 0.5 : 1 }} disabled={starting} onClick={startServices}>
                <Icon name="play" size={12} color={t.success} />Start services
              </button>
            )}
            {submitted && done !== null && (
              <button style={pillBtn(t)} onClick={() => { setSubmitted(false); }}>Edit & rebuild</button>
            )}
            <button onClick={onClose} style={pillBtn(t)}>{!submitted || done !== null ? 'Close' : 'Background'}</button>
            {!submitted && (
              <button onClick={startBuild} disabled={!canBuild}
                style={{ padding: '6px 14px', background: t.fg1, color: t.bg, border: 'none', borderRadius: 6, fontSize: 12, fontWeight: 500, cursor: canBuild ? 'pointer' : 'default', opacity: canBuild ? 1 : 0.5 }}>
                Build
              </button>
            )}
          </div>
        </div>
      </div>
    </Backdrop>
  );
}
