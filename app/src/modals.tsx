// Modals: Detail / Pull / Trivy / Update / Doctor / Settings.

import { useEffect, useState, type ReactNode } from 'react';
import type { ThemeTokens } from './theme';
import type { Container, Severity, TrivyResult, Update, DoctorCheck, Runtime } from './types';
import { Icon, Bar, iconBtn, pillBtn } from './components';
import { api } from './api';
import { withToast } from './toast';

function Backdrop({ onClose, children }: { onClose: () => void; children: ReactNode }) {
  return (
    <div onClick={onClose} style={{ position: 'absolute', inset: 0, background: 'rgba(0,0,0,0.45)', display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 100, backdropFilter: 'blur(2px)' }}>
      <div onClick={e => e.stopPropagation()}>{children}</div>
    </div>
  );
}

export function DetailModal({ item, t, onClose }: { item: Container; t: ThemeTokens; onClose: () => void }) {
  const [json, setJson] = useState<string>('Loading…');
  useEffect(() => { api.inspectContainer(item.id).then(setJson); }, [item.id]);
  const c = item;
  return (
    <Backdrop onClose={onClose}>
      <div style={{ width: 720, maxHeight: '85vh', background: t.surface, border: `1px solid ${t.border}`, borderRadius: 12, overflow: 'hidden', display: 'flex', flexDirection: 'column', boxShadow: '0 24px 80px rgba(0,0,0,0.4)' }}>
        <div style={{ padding: '18px 22px', borderBottom: `1px solid ${t.border}`, display: 'flex', alignItems: 'flex-start', gap: 12 }}>
          <Icon name="box" size={20} color={t.accent} />
          <div style={{ flex: 1 }}>
            <div style={{ fontSize: 18, fontWeight: 600, color: t.fg1 }}>{c.name}</div>
            <div style={{ fontSize: 12, color: t.fg3, fontFamily: t.mono, marginTop: 4 }}>{c.id} · {c.image}</div>
          </div>
          <button onClick={onClose} style={iconBtn()}><Icon name="x" size={16} color={t.fg2} /></button>
        </div>
        <div style={{ padding: '14px 22px', display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: 14, borderBottom: `1px solid ${t.border}` }}>
          {[
            { l: 'Status', v: c.status, color: c.status === 'running' ? t.success : t.fg3 },
            { l: 'Uptime', v: c.uptime, color: undefined as string | undefined },
            { l: 'CPU', v: `${c.cpu.toFixed(1)}%`, color: c.cpu > 85 ? t.danger : c.cpu > 60 ? t.warning : t.fg1 },
            { l: 'Memory', v: `${c.mem.used.toFixed(1)} / ${c.mem.limit.toFixed(0)} GiB`, color: undefined as string | undefined },
          ].map((m, i) => (
            <div key={i}>
              <div style={{ fontSize: 10, color: t.fg3, fontWeight: 600, letterSpacing: '0.08em', textTransform: 'uppercase' }}>{m.l}</div>
              <div style={{ marginTop: 4, fontSize: 16, fontWeight: 500, color: m.color || t.fg1, textTransform: 'capitalize' }}>{m.v}</div>
            </div>
          ))}
        </div>
        <div style={{ padding: '14px 22px', borderBottom: `1px solid ${t.border}` }}>
          <div style={{ fontSize: 10, color: t.fg3, fontWeight: 600, letterSpacing: '0.08em', textTransform: 'uppercase', marginBottom: 6 }}>Command</div>
          <div style={{ fontFamily: t.mono, fontSize: 12, color: t.fg2, padding: '8px 10px', background: t.bg, border: `1px solid ${t.border}`, borderRadius: 6 }}>{c.cmd?.length ? c.cmd.join(' ') : '—'}</div>
        </div>
        <div style={{ flex: 1, overflow: 'auto', padding: '14px 22px', minHeight: 200 }}>
          <div style={{ fontSize: 10, color: t.fg3, fontWeight: 600, letterSpacing: '0.08em', textTransform: 'uppercase', marginBottom: 8 }}>Inspect</div>
          <pre style={{ margin: 0, fontFamily: t.mono, fontSize: 11, lineHeight: 1.6, color: t.fg2, whiteSpace: 'pre-wrap', wordBreak: 'break-all' }}>{json}</pre>
        </div>
        <div style={{ padding: '12px 22px', borderTop: `1px solid ${t.border}`, display: 'flex', gap: 8, justifyContent: 'flex-end', background: t.surfaceAlt }}>
          <button style={pillBtn(t)} onClick={() => withToast(`restart ${c.name}`, api.restartContainer(c.id)).catch(() => {})}>Restart</button>
          <button style={pillBtn(t)} onClick={() => withToast(`exec ${c.name}`, api.execContainer(c.id)).catch(() => {})}>Exec /bin/sh</button>
          {c.status === 'running' && (
            <button style={pillBtn(t, t.danger)}
                    onClick={() => {
                      if (!confirm(`Force-kill ${c.name}?\nThis sends SIGKILL — the container will not get a chance to clean up.`)) return;
                      withToast(`kill ${c.name}`, api.killContainer(c.id)).then(onClose).catch(() => {});
                    }}>Kill</button>
          )}
          {c.status === 'running'
            ? <button style={pillBtn(t, t.danger)} onClick={() => withToast(`stop ${c.name}`, api.stopContainer(c.id)).then(onClose).catch(() => {})}>Stop</button>
            : <button style={pillBtn(t, t.success)} onClick={() => withToast(`start ${c.name}`, api.startContainer(c.id)).then(onClose).catch(() => {})}>Start</button>}
        </div>
      </div>
    </Backdrop>
  );
}

export function PullModal({ t, reference = 'mlcommons/inference:llama2-70b', onClose }: {
  t: ThemeTokens; reference?: string; onClose: () => void;
}) {
  const [progress, setProgress] = useState(0);
  const [lines, setLines] = useState<string[]>([]);
  const [done, setDone] = useState<boolean | null>(null);

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
    api.startPull(reference).catch(e => console.warn('start_pull failed', e));
    return () => { cancelled = true; unlistenLine?.(); unlistenDone?.(); };
  }, [reference]);

  return (
    <Backdrop onClose={onClose}>
      <div style={{ width: 640, background: t.surface, border: `1px solid ${t.border}`, borderRadius: 12, overflow: 'hidden', display: 'flex', flexDirection: 'column', boxShadow: '0 24px 80px rgba(0,0,0,0.4)' }}>
        <div style={{ padding: '16px 22px', borderBottom: `1px solid ${t.border}`, display: 'flex', alignItems: 'center', gap: 12 }}>
          <Icon name="download" size={18} color={t.accent} />
          <div style={{ flex: 1 }}>
            <div style={{ fontSize: 15, fontWeight: 600, color: t.fg1 }}>{done === true ? 'Pulled' : done === false ? 'Pull failed' : 'Pulling image'}</div>
            <div style={{ fontSize: 12, color: t.fg3, fontFamily: t.mono, marginTop: 2 }}>{reference}</div>
          </div>
          <button onClick={onClose} style={iconBtn()}><Icon name="x" size={16} color={t.fg2} /></button>
        </div>
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
        <div style={{ padding: '12px 22px', borderTop: `1px solid ${t.border}`, display: 'flex', justifyContent: 'space-between', alignItems: 'center', background: t.surfaceAlt }}>
          <span style={{ fontSize: 11, color: t.fg3, fontFamily: t.mono }}>Esc to background — pull continues</span>
          <button onClick={onClose} style={pillBtn(t)}>Background</button>
        </div>
      </div>
    </Backdrop>
  );
}

export function TrivyModal({ t, image, onClose }: { t: ThemeTokens; image?: string; onClose: () => void }) {
  const [data, setData] = useState<TrivyResult | null>(null);
  const [filter, setFilter] = useState<Severity | null>(null);
  const [search, setSearch] = useState('');
  useEffect(() => { api.scanImage(image || '').then(setData); }, [image]);
  if (!data) return null;
  const sevColor: Record<Severity, string> = { CRITICAL: t.danger, HIGH: '#E5704A', MEDIUM: t.warning, LOW: t.fg3 };
  const findings = data.findings.filter(f =>
    (!filter || f.sev === filter) &&
    (!search || (f.cve + f.pkg + f.title).toLowerCase().includes(search.toLowerCase()))
  );
  return (
    <Backdrop onClose={onClose}>
      <div style={{ width: 800, maxHeight: '85vh', background: t.surface, border: `1px solid ${t.border}`, borderRadius: 12, overflow: 'hidden', display: 'flex', flexDirection: 'column' }}>
        <div style={{ padding: '16px 22px', borderBottom: `1px solid ${t.border}`, display: 'flex', alignItems: 'center', gap: 12 }}>
          <Icon name="shield" size={18} color={t.warning} />
          <div style={{ flex: 1 }}>
            <div style={{ fontSize: 15, fontWeight: 600, color: t.fg1 }}>Trivy scan</div>
            <div style={{ fontSize: 12, color: t.fg3, fontFamily: t.mono }}>{data.image}</div>
          </div>
          <button onClick={onClose} style={iconBtn()}><Icon name="x" size={16} color={t.fg2} /></button>
        </div>
        <div style={{ padding: '14px 22px', display: 'flex', gap: 8, borderBottom: `1px solid ${t.border}` }}>
          {(Object.entries(data.counts) as [Severity, number][]).map(([sev, n]) => (
            <button key={sev} onClick={() => setFilter(filter === sev ? null : sev)} style={{
              padding: '6px 12px', background: filter === sev ? sevColor[sev] : t.surfaceAlt, color: filter === sev ? '#fff' : sevColor[sev],
              border: `1px solid ${filter === sev ? sevColor[sev] : t.border}`, borderRadius: 6, fontSize: 11, fontWeight: 600, fontFamily: t.mono, cursor: 'pointer', letterSpacing: '0.05em',
            }}>{sev} {n}</button>
          ))}
          <div style={{ flex: 1 }} />
          <input placeholder="Search CVE / package…" value={search} onChange={e => setSearch(e.target.value)}
            style={{ padding: '6px 10px', background: t.surfaceAlt, border: `1px solid ${t.border}`, borderRadius: 6, color: t.fg1, fontSize: 12, fontFamily: t.mono, width: 200, outline: 'none' }} />
        </div>
        <div style={{ flex: 1, overflow: 'auto' }}>
          {findings.map((f, i) => (
            <div key={i} style={{ padding: '12px 22px', borderBottom: `1px solid ${t.border}`, display: 'grid', gridTemplateColumns: '80px 140px 1fr 110px', gap: 12, alignItems: 'center' }}>
              <span style={{ padding: '2px 8px', borderRadius: 999, background: sevColor[f.sev], color: '#fff', fontSize: 10, fontWeight: 700, fontFamily: t.mono, textAlign: 'center', letterSpacing: '0.04em' }}>{f.sev}</span>
              <span style={{ fontFamily: t.mono, fontSize: 12, color: t.fg1 }}>{f.cve}</span>
              <div>
                <div style={{ fontSize: 13, color: t.fg1 }}>{f.title}</div>
                <div style={{ fontSize: 11, color: t.fg3, fontFamily: t.mono, marginTop: 2 }}>{f.pkg} · {f.installed} → {f.fixed}</div>
              </div>
              <button style={pillBtn(t)}>Upgrade</button>
            </div>
          ))}
        </div>
      </div>
    </Backdrop>
  );
}

export function UpdateModal({ t, onClose }: { t: ThemeTokens; onClose: () => void }) {
  const [items, setItems] = useState<Update[]>([]);
  const [idx, setIdx] = useState(0);
  useEffect(() => { api.listUpdates().then(setItems); }, []);
  if (!items.length) return null;
  const u = items[idx];
  return (
    <Backdrop onClose={onClose}>
      <div style={{ width: 640, maxHeight: '85vh', background: t.surface, border: `1px solid ${t.border}`, borderRadius: 12, overflow: 'hidden', display: 'flex', flexDirection: 'column' }}>
        <div style={{ padding: '16px 22px', borderBottom: `1px solid ${t.border}`, display: 'flex', alignItems: 'center', gap: 12 }}>
          <Icon name="download" size={18} color={t.warning} />
          <div style={{ flex: 1 }}>
            <div style={{ fontSize: 15, fontWeight: 600, color: t.fg1 }}>Update available</div>
            <div style={{ fontSize: 12, color: t.fg3, fontFamily: t.mono }}>{u.component} {u.installed} → {u.latest} · {u.published}</div>
          </div>
          <div style={{ display: 'flex', gap: 4 }}>
            {items.map((_, i) => (
              <button key={i} onClick={() => setIdx(i)} style={{ width: 8, height: 8, borderRadius: '50%', background: i === idx ? t.accent : t.border, border: 'none', cursor: 'pointer' }} />
            ))}
          </div>
          <button onClick={onClose} style={iconBtn()}><Icon name="x" size={16} color={t.fg2} /></button>
        </div>
        <div style={{ flex: 1, overflow: 'auto', padding: '16px 22px', maxHeight: 360 }}>
          <pre style={{ margin: 0, fontFamily: t.mono, fontSize: 12, lineHeight: 1.6, color: t.fg2, whiteSpace: 'pre-wrap' }}>{u.notes}</pre>
        </div>
        <UpdateActions t={t} onClose={onClose} />
      </div>
    </Backdrop>
  );
}

// Self-update flow via tauri-plugin-updater. The check() call hits the
// configured manifest endpoint; if a newer signed release exists, the
// download → install → relaunch sequence is triggered. If the endpoint
// isn't reachable (no manifest hosted yet, offline, etc.) we surface the
// error in-modal rather than via a toast — the user is here specifically
// for this action.
function UpdateActions({ t, onClose }: { t: ThemeTokens; onClose: () => void }) {
  const [phase, setPhase] = useState<'idle' | 'checking' | 'downloading' | 'ready' | 'error' | 'none'>('idle');
  const [progress, setProgress] = useState<{ downloaded: number; total: number } | null>(null);
  const [errMsg, setErrMsg] = useState<string>('');
  const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

  const install = async () => {
    if (!inTauri) {
      setPhase('error');
      setErrMsg('not running in Tauri (browser-dev mode)');
      return;
    }
    setPhase('checking');
    try {
      const { check } = await import('@tauri-apps/plugin-updater');
      const { relaunch } = await import('@tauri-apps/plugin-process');
      const update = await check();
      if (!update) { setPhase('none'); return; }
      setPhase('downloading');
      let downloaded = 0;
      let total = 0;
      await update.downloadAndInstall(event => {
        if (event.event === 'Started') { total = event.data.contentLength ?? 0; }
        else if (event.event === 'Progress') {
          downloaded += event.data.chunkLength;
          setProgress({ downloaded, total });
        }
        else if (event.event === 'Finished') { setPhase('ready'); }
      });
      // The relaunch typically happens before this resolves on macOS.
      await relaunch();
    } catch (e: any) {
      setPhase('error');
      setErrMsg(typeof e === 'string' ? e : (e?.message ?? String(e)));
    }
  };

  const status = phase === 'idle' ? null
    : phase === 'checking'    ? 'Checking for updates…'
    : phase === 'downloading' ? `Downloading ${progress ? Math.round(progress.downloaded / 1024) + ' KiB' : ''}${progress?.total ? ' / ' + Math.round(progress.total / 1024) + ' KiB' : ''}`
    : phase === 'ready'       ? 'Installed — relaunching…'
    : phase === 'none'        ? 'Already on the latest version.'
    : `Error: ${errMsg}`;

  return (
    <div style={{ padding: '12px 22px', borderTop: `1px solid ${t.border}`, display: 'flex', alignItems: 'center', gap: 8, background: t.surfaceAlt }}>
      {status && (
        <span style={{ fontSize: 11, color: phase === 'error' ? t.danger : t.fg3, fontFamily: t.mono }}>{status}</span>
      )}
      <div style={{ flex: 1 }} />
      <button style={{ ...pillBtn(t), padding: '6px 14px' }} onClick={onClose}>Later</button>
      <button onClick={install}
              disabled={phase === 'checking' || phase === 'downloading'}
              style={{ padding: '6px 14px', background: t.fg1, color: t.bg, border: 'none', borderRadius: 6, fontSize: 12, fontWeight: 500, cursor: phase === 'idle' || phase === 'error' || phase === 'none' ? 'pointer' : 'wait', opacity: phase === 'checking' || phase === 'downloading' ? 0.6 : 1 }}>
        {phase === 'downloading' ? 'Installing…' : phase === 'ready' ? 'Done' : 'Install'}
      </button>
    </div>
  );
}

export function DoctorModal({ t, onClose }: { t: ThemeTokens; onClose: () => void }) {
  const [checks, setChecks] = useState<DoctorCheck[]>([]);
  useEffect(() => { api.doctor().then(setChecks); }, []);
  const passed = checks.filter(c => c.ok).length;
  const warns = checks.filter(c => !c.ok && c.warn).length;
  const fails = checks.filter(c => !c.ok && !c.warn).length;
  return (
    <Backdrop onClose={onClose}>
      <div style={{ width: 600, background: t.surface, border: `1px solid ${t.border}`, borderRadius: 12, overflow: 'hidden' }}>
        <div style={{ padding: '16px 22px', borderBottom: `1px solid ${t.border}`, display: 'flex', alignItems: 'center', gap: 12 }}>
          <Icon name="heart" size={18} color={t.success} />
          <div style={{ flex: 1, fontSize: 15, fontWeight: 600, color: t.fg1 }}>cgui doctor</div>
          <button onClick={onClose} style={iconBtn()}><Icon name="x" size={16} color={t.fg2} /></button>
        </div>
        <div style={{ padding: '14px 22px', fontFamily: t.mono, fontSize: 12 }}>
          {checks.map((d, i) => (
            <div key={i} style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '6px 0', color: d.ok ? t.fg1 : d.warn ? t.warning : t.danger }}>
              <span style={{ width: 16, color: d.ok ? t.success : t.warning, fontWeight: 700 }}>{d.ok ? '✓' : '!'}</span>
              <span>{d.text}</span>
            </div>
          ))}
        </div>
        <div style={{ padding: '12px 22px', borderTop: `1px solid ${t.border}`, background: t.surfaceAlt, fontFamily: t.mono, fontSize: 11, color: t.fg3 }}>
          == {passed} passed · {warns} warnings · {fails} failures ==
        </div>
      </div>
    </Backdrop>
  );
}

// Generic JSON inspect modal — shared by volume + network.
export function JsonInspectModal({ t, title, subtitle, fetcher, onClose }: {
  t: ThemeTokens; title: string; subtitle: string;
  fetcher: () => Promise<string>; onClose: () => void;
}) {
  const [json, setJson] = useState<string>('Loading…');
  useEffect(() => {
    let cancelled = false;
    fetcher().then(s => { if (!cancelled) setJson(s); }).catch(e => { if (!cancelled) setJson(`error: ${e}`); });
    return () => { cancelled = true; };
  }, [fetcher]);
  return (
    <Backdrop onClose={onClose}>
      <div style={{ width: 720, maxHeight: '85vh', background: t.surface, border: `1px solid ${t.border}`, borderRadius: 12, overflow: 'hidden', display: 'flex', flexDirection: 'column', boxShadow: '0 24px 80px rgba(0,0,0,0.4)' }}>
        <div style={{ padding: '18px 22px', borderBottom: `1px solid ${t.border}`, display: 'flex', alignItems: 'flex-start', gap: 12 }}>
          <Icon name="info" size={20} color={t.accent} />
          <div style={{ flex: 1 }}>
            <div style={{ fontSize: 18, fontWeight: 600, color: t.fg1 }}>{title}</div>
            <div style={{ fontSize: 12, color: t.fg3, fontFamily: t.mono, marginTop: 4 }}>{subtitle}</div>
          </div>
          <button onClick={onClose} style={iconBtn()}><Icon name="x" size={16} color={t.fg2} /></button>
        </div>
        <div style={{ flex: 1, overflow: 'auto', padding: '14px 22px', minHeight: 200 }}>
          <pre style={{ margin: 0, fontFamily: t.mono, fontSize: 11, lineHeight: 1.6, color: t.fg2, whiteSpace: 'pre-wrap', wordBreak: 'break-all' }}>{json}</pre>
        </div>
        <div style={{ padding: '12px 22px', borderTop: `1px solid ${t.border}`, display: 'flex', gap: 8, justifyContent: 'flex-end', background: t.surfaceAlt }}>
          <button style={pillBtn(t)} onClick={() => navigator.clipboard?.writeText(json)}>Copy</button>
          <button style={pillBtn(t)} onClick={onClose}>Close</button>
        </div>
      </div>
    </Backdrop>
  );
}

export function RunImageModal({ t, image, onLaunched, onClose }: {
  t: ThemeTokens; image: string;
  onLaunched: (id: string) => void; onClose: () => void;
}) {
  const [name, setName] = useState('');
  const [ports, setPorts] = useState('');
  const [env, setEnv] = useState('');
  const [command, setCommand] = useState('');
  const [busy, setBusy] = useState(false);

  const submit = () => {
    setBusy(true);
    api.runImage({
      image,
      name: name.trim() || undefined,
      ports: ports.split('\n').map(l => l.trim()).filter(Boolean),
      env: env.split('\n').map(l => l.trim()).filter(Boolean),
      command: command.trim() || undefined,
    }).then(id => { onLaunched(id); onClose(); })
      .catch(() => { setBusy(false); /* toast via withToast wrapper at call-site */ });
  };

  const inputStyle = {
    width: '100%', padding: '8px 10px', background: t.bg, border: `1px solid ${t.border}`,
    borderRadius: 6, color: t.fg1, fontSize: 12, fontFamily: t.mono, outline: 'none',
    boxSizing: 'border-box' as const,
  };
  const labelStyle = { fontSize: 11, fontWeight: 600, letterSpacing: '0.08em', textTransform: 'uppercase' as const, color: t.fg3, marginBottom: 6, display: 'block' };

  return (
    <Backdrop onClose={onClose}>
      <div style={{ width: 560, background: t.surface, border: `1px solid ${t.border}`, borderRadius: 12, overflow: 'hidden', display: 'flex', flexDirection: 'column', boxShadow: '0 24px 80px rgba(0,0,0,0.4)' }}>
        <div style={{ padding: '18px 22px', borderBottom: `1px solid ${t.border}`, display: 'flex', alignItems: 'flex-start', gap: 12 }}>
          <Icon name="play" size={20} color={t.success} />
          <div style={{ flex: 1 }}>
            <div style={{ fontSize: 18, fontWeight: 600, color: t.fg1 }}>Run image</div>
            <div style={{ fontSize: 12, color: t.fg3, fontFamily: t.mono, marginTop: 4 }}>{image}</div>
          </div>
          <button onClick={onClose} style={iconBtn()}><Icon name="x" size={16} color={t.fg2} /></button>
        </div>
        <div style={{ padding: '18px 22px', display: 'flex', flexDirection: 'column', gap: 14 }}>
          <div>
            <label style={labelStyle}>Name <span style={{ color: t.fg3, fontWeight: 400, textTransform: 'none', letterSpacing: 0 }}>(optional)</span></label>
            <input style={inputStyle} value={name} onChange={e => setName(e.target.value)} placeholder="myservice" />
          </div>
          <div>
            <label style={labelStyle}>Port mappings <span style={{ color: t.fg3, fontWeight: 400, textTransform: 'none', letterSpacing: 0 }}>(host:container, one per line)</span></label>
            <textarea style={{ ...inputStyle, minHeight: 60, resize: 'vertical' }} value={ports} onChange={e => setPorts(e.target.value)} placeholder="8080:80&#10;5432:5432" />
          </div>
          <div>
            <label style={labelStyle}>Environment <span style={{ color: t.fg3, fontWeight: 400, textTransform: 'none', letterSpacing: 0 }}>(KEY=value, one per line)</span></label>
            <textarea style={{ ...inputStyle, minHeight: 60, resize: 'vertical' }} value={env} onChange={e => setEnv(e.target.value)} placeholder="NODE_ENV=production&#10;DEBUG=true" />
          </div>
          <div>
            <label style={labelStyle}>Override command <span style={{ color: t.fg3, fontWeight: 400, textTransform: 'none', letterSpacing: 0 }}>(optional)</span></label>
            <input style={inputStyle} value={command} onChange={e => setCommand(e.target.value)} placeholder="sh -c 'echo hello'" />
          </div>
        </div>
        <div style={{ padding: '12px 22px', borderTop: `1px solid ${t.border}`, display: 'flex', gap: 8, justifyContent: 'flex-end', background: t.surfaceAlt }}>
          <button style={pillBtn(t)} onClick={onClose} disabled={busy}>Cancel</button>
          <button style={{ padding: '6px 14px', background: t.fg1, color: t.bg, border: 'none', borderRadius: 6, fontSize: 12, fontWeight: 500, cursor: busy ? 'wait' : 'pointer', opacity: busy ? 0.6 : 1 }}
                  onClick={submit} disabled={busy}>{busy ? 'Starting…' : 'Run'}</button>
        </div>
      </div>
    </Backdrop>
  );
}

export function SettingsModal({ t, onClose, runtime, setRuntime }: {
  t: ThemeTokens; onClose: () => void; runtime: Runtime; setRuntime: (r: Runtime) => void;
}) {
  const runtimes: { key: Runtime; path: string }[] = [
    { key: 'container', path: '/usr/local/bin/container' },
    { key: 'docker', path: '/usr/local/bin/docker' },
    { key: 'podman', path: '/opt/homebrew/bin/podman' },
  ];
  return (
    <Backdrop onClose={onClose}>
      <div style={{ width: 640, background: t.surface, border: `1px solid ${t.border}`, borderRadius: 12, overflow: 'hidden', maxHeight: '85vh', display: 'flex', flexDirection: 'column' }}>
        <div style={{ padding: '16px 22px', borderBottom: `1px solid ${t.border}`, display: 'flex', alignItems: 'center', gap: 12 }}>
          <Icon name="cog" size={18} color={t.fg2} />
          <div style={{ flex: 1, fontSize: 15, fontWeight: 600, color: t.fg1 }}>Settings</div>
          <button onClick={onClose} style={iconBtn()}><Icon name="x" size={16} color={t.fg2} /></button>
        </div>
        <div style={{ flex: 1, overflow: 'auto', padding: '18px 22px' }}>
          <div style={{ fontSize: 11, fontWeight: 600, letterSpacing: '0.08em', textTransform: 'uppercase', color: t.fg3, marginBottom: 10 }}>Runtime profile</div>
          {runtimes.map(({ key, path }) => (
            <label key={key} style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '10px 12px', borderRadius: 6, background: key === runtime ? t.selected : 'transparent', border: `1px solid ${key === runtime ? t.accent : t.border}`, marginBottom: 6, cursor: 'pointer' }}>
              <input type="radio" checked={key === runtime} onChange={() => setRuntime(key)} style={{ accentColor: t.accent }} />
              <div style={{ flex: 1 }}>
                <div style={{ fontSize: 13, color: t.fg1, fontWeight: 500, fontFamily: t.mono }}>{key}</div>
                <div style={{ fontSize: 11, color: t.fg3, fontFamily: t.mono }}>{path}</div>
              </div>
              {key === runtime && <span style={{ fontSize: 10, color: t.success, fontFamily: t.mono }}>● ACTIVE</span>}
            </label>
          ))}
          <div style={{ fontSize: 11, fontWeight: 600, letterSpacing: '0.08em', textTransform: 'uppercase', color: t.fg3, margin: '20px 0 10px' }}>Resource alerts</div>
          {[
            { l: 'CPU warn', v: '60%' }, { l: 'CPU alert', v: '85%' },
            { l: 'Memory warn', v: '70%' }, { l: 'Memory alert', v: '90%' },
          ].map((row, i) => (
            <div key={i} style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '8px 0', borderBottom: `1px solid ${t.border}` }}>
              <span style={{ fontSize: 13, color: t.fg2 }}>{row.l}</span>
              <span style={{ fontSize: 12, color: t.fg1, fontFamily: t.mono, padding: '4px 10px', background: t.surfaceAlt, border: `1px solid ${t.border}`, borderRadius: 4 }}>{row.v}</span>
            </div>
          ))}
          <div style={{ fontSize: 11, fontWeight: 600, letterSpacing: '0.08em', textTransform: 'uppercase', color: t.fg3, margin: '20px 0 10px' }}>State</div>
          <div style={{ fontFamily: t.mono, fontSize: 11, color: t.fg3 }}>~/.config/cgui/state.json — auto-saved</div>
        </div>
      </div>
    </Backdrop>
  );
}
