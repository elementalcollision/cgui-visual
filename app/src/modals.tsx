// Modals: Detail / Pull / Trivy / Update / Doctor / Settings.

import React, { Fragment, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import type { ThemeTokens } from './theme';
import type { Container, Image, Severity, Stack, Tab, TrivyFinding, TrivyResult, Update, DoctorCheck, DoctorFix, Runtime } from './types';
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

// Tabs available in the inspect drawer. Each maps to a different
// projection of the inspect JSON; "raw" always falls back to the verbatim
// blob so users can still grep odd fields the parser hasn't extracted.
type InspectTab = 'env' | 'mounts' | 'network' | 'ports' | 'health' | 'raw';

interface ParsedInspect {
  env: { key: string; value: string }[];
  mounts: { source: string; destination: string; type: string; readOnly?: boolean }[];
  network: { name: string; ip: string; mac?: string }[];
  ports: { container: string; host: string }[];
  health?: { status: string; failingStreak?: number; log: { exitCode: number; output: string }[] };
}

// Permissive inspect-JSON parser. Apple's `container inspect` output isn't
// 100% docker-compatible, so we look at both Docker's PascalCase keys and
// Apple's camelCase variants. Missing fields are silently skipped — the
// Raw tab is the safety net.
export function parseInspect(text: string): ParsedInspect | null {
  let root: any;
  try {
    const v = JSON.parse(text);
    // `container inspect` returns an array; `docker inspect` returns either.
    root = Array.isArray(v) ? v[0] : v;
  } catch {
    return null;
  }
  if (!root || typeof root !== 'object') return null;

  const config = root.Config ?? root.config ?? {};
  const ns = root.NetworkSettings ?? root.networkSettings ?? {};
  const hostConfig = root.HostConfig ?? root.hostConfig ?? {};
  const stateBlock = root.State ?? root.state ?? {};

  // ── Env: ["KEY=value", ...] split once on first '='. Keys without '='
  //         become bare keys with empty value rather than getting dropped.
  const envList: string[] = config.Env ?? config.env ?? [];
  const env = envList.map((line: string) => {
    const i = line.indexOf('=');
    return i === -1 ? { key: line, value: '' } : { key: line.slice(0, i), value: line.slice(i + 1) };
  });

  // ── Mounts: docker shape is {Source,Destination,Type,RW}; apple variant
  //         exposes camelCase. Read-only is RW=false (docker) or
  //         readOnly=true (apple).
  const rawMounts: any[] = root.Mounts ?? root.mounts ?? [];
  const mounts = rawMounts.map((m: any) => ({
    source: m.Source ?? m.source ?? '',
    destination: m.Destination ?? m.destination ?? '',
    type: m.Type ?? m.type ?? '',
    readOnly: typeof m.RW === 'boolean' ? !m.RW : (m.readOnly === true ? true : undefined),
  }));

  // ── Network: docker NetworkSettings.Networks is { netName: { IPAddress } };
  //         older payloads have just IPAddress at top level.
  const networks: { name: string; ip: string; mac?: string }[] = [];
  const dockerNets = ns.Networks ?? ns.networks;
  if (dockerNets && typeof dockerNets === 'object') {
    for (const [name, info] of Object.entries(dockerNets as Record<string, any>)) {
      networks.push({
        name,
        ip: info?.IPAddress ?? info?.ipAddress ?? '',
        mac: info?.MacAddress ?? info?.macAddress,
      });
    }
  } else if (ns.IPAddress || ns.ipAddress) {
    networks.push({
      name: 'default',
      ip: ns.IPAddress ?? ns.ipAddress ?? '',
      mac: ns.MacAddress ?? ns.macAddress,
    });
  }

  // ── Ports: docker shape is { "8080/tcp": [{ HostPort: "8080" }, …] }.
  //         A port with a null mapping is "exposed but not published".
  const ports: { container: string; host: string }[] = [];
  const portMap = ns.Ports ?? ns.ports ?? config.ExposedPorts ?? config.exposedPorts ?? {};
  for (const [containerPort, bindings] of Object.entries(portMap as Record<string, any>)) {
    if (Array.isArray(bindings) && bindings.length > 0) {
      for (const b of bindings) {
        const hp = b?.HostPort ?? b?.hostPort ?? '';
        const hi = b?.HostIp ?? b?.hostIp ?? '';
        ports.push({ container: containerPort, host: hi ? `${hi}:${hp}` : hp });
      }
    } else {
      ports.push({ container: containerPort, host: '— (exposed)' });
    }
  }

  // ── Health: only present when the image declared a HEALTHCHECK. The
  //         log entries are kept short (last 3) since the modal is small.
  const healthBlock = stateBlock.Health ?? stateBlock.health;
  let health: ParsedInspect['health'];
  if (healthBlock && typeof healthBlock === 'object') {
    const log = (healthBlock.Log ?? healthBlock.log ?? []).slice(-3).map((entry: any) => ({
      exitCode: entry?.ExitCode ?? entry?.exitCode ?? 0,
      output: entry?.Output ?? entry?.output ?? '',
    }));
    health = {
      status: healthBlock.Status ?? healthBlock.status ?? 'unknown',
      failingStreak: healthBlock.FailingStreak ?? healthBlock.failingStreak,
      log,
    };
  }

  // Suppress unused-warning when restart policy parsing is added later;
  // we keep the read here so the dependency is documented.
  void hostConfig;

  return { env, mounts, network: networks, ports, health };
}

// Compact tab-strip for the inspect modal. Disabled tabs (no data for
// that projection) render dim and stay un-clickable.
function InspectTabs({ t, tab, setTab, parsed }: {
  t: ThemeTokens; tab: InspectTab; setTab: (v: InspectTab) => void; parsed: ParsedInspect | null;
}) {
  const tabs: { id: InspectTab; label: string; count: number; available: boolean }[] = [
    { id: 'env',     label: 'Env',     count: parsed?.env.length ?? 0,     available: !!parsed?.env.length },
    { id: 'mounts',  label: 'Mounts',  count: parsed?.mounts.length ?? 0,  available: !!parsed?.mounts.length },
    { id: 'network', label: 'Network', count: parsed?.network.length ?? 0, available: !!parsed?.network.length },
    { id: 'ports',   label: 'Ports',   count: parsed?.ports.length ?? 0,   available: !!parsed?.ports.length },
    { id: 'health',  label: 'Health',  count: parsed?.health ? 1 : 0,      available: !!parsed?.health },
    { id: 'raw',     label: 'Raw',     count: 0,                            available: true },
  ];
  return (
    <div style={{ display: 'flex', gap: 4, padding: '8px 16px 0', borderBottom: `1px solid ${t.border}` }}>
      {tabs.map(item => {
        const active = tab === item.id;
        const dim = !item.available && item.id !== 'raw';
        return (
          <button
            key={item.id}
            disabled={dim}
            onClick={() => setTab(item.id)}
            style={{
              padding: '8px 12px',
              fontSize: 12, fontWeight: 500, fontFamily: t.mono,
              color: active ? t.fg1 : (dim ? t.fg3 : t.fg2),
              background: 'transparent',
              border: 'none',
              borderBottom: `2px solid ${active ? t.accent : 'transparent'}`,
              cursor: dim ? 'default' : 'pointer',
              opacity: dim ? 0.5 : 1,
              marginBottom: -1,
            }}
          >
            {item.label}{item.count > 0 && <span style={{ marginLeft: 6, color: t.fg3, fontSize: 10 }}>{item.count}</span>}
          </button>
        );
      })}
    </div>
  );
}

function InspectPanel({ t, tab, parsed, json }: {
  t: ThemeTokens; tab: InspectTab; parsed: ParsedInspect | null; json: string;
}) {
  if (tab === 'raw' || !parsed) {
    return (
      <pre style={{ margin: 0, fontFamily: t.mono, fontSize: 11, lineHeight: 1.6, color: t.fg2, whiteSpace: 'pre-wrap', wordBreak: 'break-all' }}>{json}</pre>
    );
  }
  if (tab === 'env') {
    return (
      <KvList
        t={t}
        empty="No environment variables."
        rows={parsed.env.map(e => ({ k: e.key, v: e.value }))}
      />
    );
  }
  if (tab === 'mounts') {
    if (!parsed.mounts.length) return <Muted t={t}>No mounts.</Muted>;
    return (
      <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
        {parsed.mounts.map((m, i) => (
          <div key={i} style={{
            padding: '8px 12px', background: t.surfaceAlt, border: `1px solid ${t.border}`, borderRadius: 6,
            display: 'grid', gridTemplateColumns: '1fr auto', gap: 8, alignItems: 'baseline',
          }}>
            <div style={{ fontFamily: t.mono, fontSize: 12, color: t.fg2, wordBreak: 'break-all' }}>
              <span style={{ color: t.fg1 }}>{m.source}</span>
              <span style={{ color: t.fg3, margin: '0 8px' }}>→</span>
              <span style={{ color: t.fg1 }}>{m.destination}</span>
            </div>
            <span style={{ fontFamily: t.mono, fontSize: 10, color: t.fg3, padding: '2px 8px', background: t.bg, borderRadius: 4 }}>
              {m.type}{m.readOnly ? ' · ro' : ''}
            </span>
          </div>
        ))}
      </div>
    );
  }
  if (tab === 'network') {
    if (!parsed.network.length) return <Muted t={t}>No network attachments.</Muted>;
    return (
      <KvList
        t={t}
        empty="No networks."
        rows={parsed.network.flatMap(n => {
          const out: { k: string; v: string }[] = [{ k: n.name, v: n.ip || '—' }];
          if (n.mac) out.push({ k: `${n.name} (mac)`, v: n.mac });
          return out;
        })}
      />
    );
  }
  if (tab === 'ports') {
    if (!parsed.ports.length) return <Muted t={t}>No ports exposed.</Muted>;
    return (
      <KvList
        t={t}
        empty="No ports."
        rows={parsed.ports.map(p => ({ k: p.container, v: p.host }))}
      />
    );
  }
  if (tab === 'health') {
    const h = parsed.health;
    if (!h) return <Muted t={t}>No HEALTHCHECK configured.</Muted>;
    const statusColor = h.status === 'healthy' ? t.success : h.status === 'unhealthy' ? t.danger : t.warning;
    return (
      <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
        <div style={{ fontFamily: t.mono, fontSize: 12, color: t.fg2 }}>
          status <span style={{ color: statusColor, fontWeight: 600 }}>{h.status}</span>
          {typeof h.failingStreak === 'number' && (
            <span style={{ color: t.fg3, marginLeft: 12 }}>failing streak: {h.failingStreak}</span>
          )}
        </div>
        {h.log.length > 0 && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
            {h.log.map((entry, i) => (
              <div key={i} style={{
                padding: '8px 10px', background: t.bg, border: `1px solid ${t.border}`, borderRadius: 6,
                fontFamily: t.mono, fontSize: 11, color: entry.exitCode === 0 ? t.fg2 : t.danger, whiteSpace: 'pre-wrap',
              }}>
                <span style={{ color: t.fg3 }}>exit {entry.exitCode}</span>{entry.output ? ` · ${entry.output}` : ''}
              </div>
            ))}
          </div>
        )}
      </div>
    );
  }
  return null;
}

function KvList({ t, rows, empty }: { t: ThemeTokens; rows: { k: string; v: string }[]; empty: string }) {
  if (!rows.length) return <Muted t={t}>{empty}</Muted>;
  return (
    <div style={{ display: 'grid', gridTemplateColumns: 'auto 1fr', gap: '6px 14px', fontFamily: t.mono, fontSize: 12, alignItems: 'baseline' }}>
      {rows.map((r, i) => (
        <Fragment key={i}>
          <div style={{ color: t.fg3, whiteSpace: 'nowrap' }}>{r.k}</div>
          <div style={{ color: t.fg1, wordBreak: 'break-all' }}>{r.v || <span style={{ color: t.fg3 }}>—</span>}</div>
        </Fragment>
      ))}
    </div>
  );
}

function Muted({ t, children }: { t: ThemeTokens; children: ReactNode }) {
  return <div style={{ fontSize: 12, color: t.fg3, fontStyle: 'italic' }}>{children}</div>;
}

export function DetailModal({ item, t, onClose }: { item: Container; t: ThemeTokens; onClose: () => void }) {
  const [json, setJson] = useState<string>('Loading…');
  const [tab, setTab] = useState<InspectTab>('env');
  useEffect(() => { api.inspectContainer(item.id).then(setJson); }, [item.id]);
  const c = item;
  // Parse once per JSON change. Errors are tolerated — the Raw tab still
  // shows whatever came back, even when it isn't valid JSON.
  const parsed = useMemo(() => parseInspect(json), [json]);
  return (
    <Backdrop onClose={onClose}>
      <div style={{ width: 760, maxHeight: '85vh', background: t.surface, border: `1px solid ${t.border}`, borderRadius: 12, overflow: 'hidden', display: 'flex', flexDirection: 'column', boxShadow: '0 24px 80px rgba(0,0,0,0.4)' }}>
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
        <InspectTabs t={t} tab={tab} setTab={setTab} parsed={parsed} />
        <div style={{ flex: 1, overflow: 'auto', padding: '14px 22px', minHeight: 200 }}>
          <InspectPanel t={t} tab={tab} parsed={parsed} json={json} />
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

// Synthesise a vendor-neutral lookup URL when trivy doesn't report any
// References. NVD has the broadest CVE coverage and supports a stable
// path scheme by id, so it makes a sane default link target.
function nvdUrl(cve: string): string {
  return `https://nvd.nist.gov/vuln/detail/${encodeURIComponent(cve)}`;
}

export function TrivyModal({ t, image, onClose }: { t: ThemeTokens; image?: string; onClose: () => void }) {
  const [data, setData] = useState<TrivyResult | null>(null);
  const [filter, setFilter] = useState<Severity | null>(null);
  const [search, setSearch] = useState('');
  const [selected, setSelected] = useState<TrivyFinding | null>(null);
  useEffect(() => { api.scanImage(image || '').then(setData); }, [image]);
  if (!data) return null;
  const sevColor: Record<Severity, string> = { CRITICAL: t.danger, HIGH: '#E5704A', MEDIUM: t.warning, LOW: t.fg3 };
  const findings = data.findings.filter(f =>
    (!filter || f.sev === filter) &&
    (!search || (f.cve + f.pkg + f.title).toLowerCase().includes(search.toLowerCase()))
  );
  return (
    <Backdrop onClose={onClose}>
      <div style={{ width: 1000, maxHeight: '85vh', background: t.surface, border: `1px solid ${t.border}`, borderRadius: 12, overflow: 'hidden', display: 'flex', flexDirection: 'column' }}>
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
        {/* Findings table + slide-in drawer. The drawer is a simple flex
            sibling rather than a separate overlay so it cooperates with the
            modal's max-height + scrollbars. */}
        <div style={{ flex: 1, display: 'flex', minHeight: 0 }}>
          <div style={{ flex: 1, overflow: 'auto', borderRight: selected ? `1px solid ${t.border}` : 'none' }}>
            {findings.map((f, i) => {
              const active = selected?.cve === f.cve && selected?.pkg === f.pkg;
              return (
                <button
                  key={`${f.cve}-${f.pkg}-${i}`}
                  onClick={() => setSelected(active ? null : f)}
                  style={{
                    width: '100%', textAlign: 'left',
                    padding: '12px 22px', borderBottom: `1px solid ${t.border}`,
                    background: active ? t.selected : 'transparent',
                    border: 'none', borderLeft: `3px solid ${active ? sevColor[f.sev] : 'transparent'}`,
                    display: 'grid', gridTemplateColumns: '80px 140px 1fr 60px', gap: 12, alignItems: 'center',
                    cursor: 'pointer', color: 'inherit',
                  }}
                >
                  <span style={{ padding: '2px 8px', borderRadius: 999, background: sevColor[f.sev], color: '#fff', fontSize: 10, fontWeight: 700, fontFamily: t.mono, textAlign: 'center', letterSpacing: '0.04em' }}>{f.sev}</span>
                  <span style={{ fontFamily: t.mono, fontSize: 12, color: t.fg1 }}>{f.cve}</span>
                  <div>
                    <div style={{ fontSize: 13, color: t.fg1 }}>{f.title || '(no title)'}</div>
                    <div style={{ fontSize: 11, color: t.fg3, fontFamily: t.mono, marginTop: 2 }}>{f.pkg} · {f.installed} → {f.fixed || '—'}</div>
                  </div>
                  <span style={{ fontFamily: t.mono, fontSize: 12, color: t.fg2, fontVariantNumeric: 'tabular-nums', textAlign: 'right' }}>
                    {typeof f.cvss === 'number' ? f.cvss.toFixed(1) : '—'}
                  </span>
                </button>
              );
            })}
          </div>
          {selected && (
            <CveDrawer t={t} f={selected} sevColor={sevColor[selected.sev]} onClose={() => setSelected(null)} />
          )}
        </div>
      </div>
    </Backdrop>
  );
}

function CveDrawer({ t, f, sevColor, onClose }: {
  t: ThemeTokens; f: TrivyFinding; sevColor: string; onClose: () => void;
}) {
  const refs = (f.refs && f.refs.length > 0) ? f.refs : [nvdUrl(f.cve)];
  return (
    <div style={{
      width: 380, flex: '0 0 380px', overflow: 'auto',
      background: t.surface, padding: '16px 18px',
      display: 'flex', flexDirection: 'column', gap: 14,
    }}>
      <div style={{ display: 'flex', alignItems: 'flex-start', gap: 10 }}>
        <span style={{ padding: '3px 10px', borderRadius: 999, background: sevColor, color: '#fff', fontSize: 10, fontWeight: 700, fontFamily: t.mono, letterSpacing: '0.04em' }}>{f.sev}</span>
        <div style={{ flex: 1, minWidth: 0 }}>
          <div style={{ fontFamily: t.mono, fontSize: 13, fontWeight: 600, color: t.fg1, wordBreak: 'break-all' }}>{f.cve}</div>
          {typeof f.cvss === 'number' && (
            <div style={{ fontSize: 11, color: t.fg3, fontFamily: t.mono, marginTop: 2 }}>CVSS v3 · <span style={{ color: t.fg2 }}>{f.cvss.toFixed(1)}</span></div>
          )}
        </div>
        <button onClick={onClose} style={iconBtn()}><Icon name="x" size={14} color={t.fg2} /></button>
      </div>
      {f.title && (
        <div style={{ fontSize: 13, color: t.fg1, lineHeight: 1.5 }}>{f.title}</div>
      )}
      <div style={{
        fontSize: 11, color: t.fg3, fontFamily: t.mono,
        background: t.surfaceAlt, border: `1px solid ${t.border}`, borderRadius: 6,
        padding: '8px 10px',
      }}>
        <div><span style={{ color: t.fg2 }}>package</span> {f.pkg}</div>
        <div><span style={{ color: t.fg2 }}>installed</span> {f.installed}</div>
        <div><span style={{ color: t.fg2 }}>fixed in</span> {f.fixed || '— (no fix yet)'}</div>
      </div>
      {f.description && (
        <div>
          <div style={{ fontSize: 10, color: t.fg3, fontWeight: 600, letterSpacing: '0.08em', textTransform: 'uppercase', marginBottom: 6 }}>Description</div>
          <div style={{ fontSize: 12, color: t.fg2, lineHeight: 1.55, whiteSpace: 'pre-wrap' }}>{f.description}</div>
        </div>
      )}
      <div>
        <div style={{ fontSize: 10, color: t.fg3, fontWeight: 600, letterSpacing: '0.08em', textTransform: 'uppercase', marginBottom: 6 }}>References</div>
        <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
          {refs.map((u, i) => (
            <a
              key={i}
              href={u}
              target="_blank"
              rel="noreferrer noopener"
              style={{ fontFamily: t.mono, fontSize: 11, color: t.accent, wordBreak: 'break-all', textDecoration: 'none' }}
            >{u}</a>
          ))}
        </div>
      </div>
    </div>
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

// Fire a DoctorFix. URL fixes go through the opener plugin (or a fallback
// window.open in browser-dev mode). Copy fixes write to the clipboard and
// flash the button label so the user sees confirmation.
async function runDoctorFix(fix: DoctorFix): Promise<'opened' | 'copied'> {
  if (fix.kind === 'url') {
    try {
      const { openUrl } = await import('@tauri-apps/plugin-opener');
      await openUrl(fix.url);
    } catch {
      window.open(fix.url, '_blank', 'noopener,noreferrer');
    }
    return 'opened';
  }
  await navigator.clipboard?.writeText(fix.command);
  return 'copied';
}

function DoctorRow({ t, d }: { t: ThemeTokens; d: DoctorCheck }) {
  const [flash, setFlash] = useState<'opened' | 'copied' | null>(null);
  useEffect(() => {
    if (!flash) return;
    const id = window.setTimeout(() => setFlash(null), 1500);
    return () => window.clearTimeout(id);
  }, [flash]);
  const onClick = async () => {
    if (!d.fix) return;
    try {
      const verb = await runDoctorFix(d.fix);
      setFlash(verb);
    } catch (e) {
      // Fallback toast via the lazy import to avoid a circular dep.
      const { toast } = await import('./toast');
      toast(`Doctor fix failed: ${typeof e === 'string' ? e : (e as Error)?.message ?? 'unknown'}`);
    }
  };
  const label = flash === 'copied' ? '✓ Copied'
              : flash === 'opened' ? '✓ Opened'
              : d.fix?.label;
  return (
    <div style={{
      display: 'flex', alignItems: 'center', gap: 10, padding: '6px 0',
      color: d.ok ? t.fg1 : d.warn ? t.warning : t.danger,
    }}>
      <span style={{ width: 16, color: d.ok ? t.success : t.warning, fontWeight: 700 }}>{d.ok ? '✓' : '!'}</span>
      <span style={{ flex: 1 }}>{d.text}</span>
      {d.fix && (
        <button
          onClick={onClick}
          style={{
            padding: '4px 10px',
            background: t.surfaceAlt, color: t.fg2,
            border: `1px solid ${t.border}`, borderRadius: 4,
            fontSize: 11, fontFamily: t.mono, cursor: 'pointer',
            whiteSpace: 'nowrap',
          }}
          title={d.fix.kind === 'url' ? d.fix.url : d.fix.command}
        >{label}</button>
      )}
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
          {checks.map((d, i) => <DoctorRow key={i} t={t} d={d} />)}
        </div>
        <div style={{ padding: '12px 22px', borderTop: `1px solid ${t.border}`, background: t.surfaceAlt, fontFamily: t.mono, fontSize: 11, color: t.fg3 }}>
          == {passed} passed · {warns} warnings · {fails} failures ==
        </div>
      </div>
    </Backdrop>
  );
}

// ─── Image layer inspector (A7) ───────────────────────────────────────
//
// Apple's `container image inspect` returns the OCI image config, which
// includes a `history[]` array describing each build step (Dockerfile
// instruction → resulting layer). We surface that as a "Layers" view
// alongside the raw JSON. Sizes aren't in the config — they live in the
// manifest — so the layer rows show command + age + empty-layer flag and
// the diff_id when available, but `size` stays optional.

export interface ImageLayer {
  /** Wall-clock timestamp the layer was created. ISO-8601 from the OCI
   *  config; empty when the build step is missing it. */
  created: string;
  /** The build command (Dockerfile RUN/COPY/CMD/etc.) that produced this
   *  layer. Often prefixed with `/bin/sh -c #(nop)` for non-RUN steps. */
  createdBy: string;
  /** Optional comment shown next to the command. */
  comment?: string;
  /** OCI calls a step "empty" when no filesystem diff was produced
   *  (typical of CMD / ENV / LABEL). We render those dim. */
  emptyLayer?: boolean;
  /** sha256 diff id from rootfs.diff_ids. Aligned to non-empty layers in
   *  history order; undefined when the alignment is ambiguous. */
  diffId?: string;
}

export interface ParsedImage {
  layers: ImageLayer[];
  digest?: string;
  os?: string;
  architecture?: string;
}

// Pull a layer summary out of an `image inspect` JSON payload. Tolerant
// to docker (PascalCase) vs apple/OCI (camelCase) and to the top-level
// array form `[{ ... }]`. Returns null on un-parseable input so the
// caller can fall back to a raw-JSON view.
export function parseImageInspect(text: string): ParsedImage | null {
  let root: any;
  try {
    const v = JSON.parse(text);
    root = Array.isArray(v) ? v[0] : v;
  } catch {
    return null;
  }
  if (!root || typeof root !== 'object') return null;

  const history: any[] = root.history ?? root.History ?? [];
  const diffIds: string[] = root?.rootfs?.diff_ids ?? root?.RootFS?.DiffIDs ?? [];

  // Walk history; for every non-empty step, claim the next diff id off
  // the queue. Empty steps don't consume a diff id (per OCI spec).
  let dq = 0;
  const layers: ImageLayer[] = history.map((h: any) => {
    const empty = h.empty_layer === true || h.emptyLayer === true;
    const diffId = empty ? undefined : diffIds[dq];
    if (!empty) dq++;
    return {
      created: h.created ?? h.Created ?? '',
      createdBy: h.created_by ?? h.createdBy ?? h.CreatedBy ?? '',
      comment: h.comment ?? h.Comment ?? undefined,
      emptyLayer: empty || undefined,
      diffId,
    };
  });

  return {
    layers,
    digest: root.config?.digest ?? root.Id ?? root.id,
    os: root.os ?? root.Os,
    architecture: root.architecture ?? root.Architecture,
  };
}

// Squash leading `/bin/sh -c #(nop) ` so the visible command focuses on
// the Dockerfile instruction. Falls back to the full string when the
// prefix isn't present.
function prettyCmd(s: string): string {
  return s.replace(/^\/bin\/sh -c #\(nop\)\s*/, '').trim() || s;
}

// Best-effort relative "n ago" without pulling a date library. Returns
// "—" for missing / un-parseable timestamps so the column degrades.
function ago(iso: string): string {
  if (!iso) return '—';
  const ms = Date.parse(iso);
  if (Number.isNaN(ms)) return '—';
  const sec = Math.max(0, Math.floor((Date.now() - ms) / 1000));
  if (sec < 60) return `${sec}s ago`;
  const min = Math.floor(sec / 60);
  if (min < 60) return `${min}m ago`;
  const hr = Math.floor(min / 60);
  if (hr < 48) return `${hr}h ago`;
  const day = Math.floor(hr / 24);
  if (day < 30) return `${day}d ago`;
  const mo = Math.floor(day / 30);
  if (mo < 24) return `${mo}mo ago`;
  return `${Math.floor(day / 365)}y ago`;
}

export function ImageInspectModal({ t, reference, onClose }: {
  t: ThemeTokens; reference: string; onClose: () => void;
}) {
  const [json, setJson] = useState<string>('Loading…');
  const [tab, setTab] = useState<'layers' | 'raw'>('layers');
  useEffect(() => {
    let cancelled = false;
    api.inspectImage(reference)
      .then(s => { if (!cancelled) setJson(s); })
      .catch(e => { if (!cancelled) setJson(`error: ${e}`); });
    return () => { cancelled = true; };
  }, [reference]);

  const parsed = useMemo(() => parseImageInspect(json), [json]);
  const layers = parsed?.layers ?? [];
  const realLayers = layers.filter(l => !l.emptyLayer).length;

  return (
    <Backdrop onClose={onClose}>
      <div style={{ width: 820, maxHeight: '85vh', background: t.surface, border: `1px solid ${t.border}`, borderRadius: 12, overflow: 'hidden', display: 'flex', flexDirection: 'column', boxShadow: '0 24px 80px rgba(0,0,0,0.4)' }}>
        <div style={{ padding: '18px 22px', borderBottom: `1px solid ${t.border}`, display: 'flex', alignItems: 'flex-start', gap: 12 }}>
          <Icon name="image" size={20} color={t.accent} />
          <div style={{ flex: 1, minWidth: 0 }}>
            <div style={{ fontSize: 16, fontWeight: 600, color: t.fg1, fontFamily: t.mono, wordBreak: 'break-all' }}>{reference}</div>
            <div style={{ fontSize: 12, color: t.fg3, fontFamily: t.mono, marginTop: 4 }}>
              container image inspect
              {parsed && (
                <span style={{ marginLeft: 8 }}>
                  · {realLayers} layer{realLayers === 1 ? '' : 's'}
                  {parsed.architecture && ` · ${parsed.os ?? 'linux'}/${parsed.architecture}`}
                </span>
              )}
            </div>
          </div>
          <button onClick={onClose} style={iconBtn()}><Icon name="x" size={16} color={t.fg2} /></button>
        </div>
        <div style={{ display: 'flex', gap: 4, padding: '8px 16px 0', borderBottom: `1px solid ${t.border}` }}>
          {([
            { id: 'layers' as const, label: 'Layers', count: layers.length, available: layers.length > 0 },
            { id: 'raw'    as const, label: 'Raw',    count: 0,             available: true },
          ]).map(item => {
            const active = tab === item.id;
            const dim = !item.available && item.id !== 'raw';
            return (
              <button
                key={item.id}
                disabled={dim}
                onClick={() => setTab(item.id)}
                style={{
                  padding: '8px 12px',
                  fontSize: 12, fontWeight: 500, fontFamily: t.mono,
                  color: active ? t.fg1 : (dim ? t.fg3 : t.fg2),
                  background: 'transparent', border: 'none',
                  borderBottom: `2px solid ${active ? t.accent : 'transparent'}`,
                  cursor: dim ? 'default' : 'pointer',
                  opacity: dim ? 0.5 : 1, marginBottom: -1,
                }}
              >
                {item.label}{item.count > 0 && <span style={{ marginLeft: 6, color: t.fg3, fontSize: 10 }}>{item.count}</span>}
              </button>
            );
          })}
        </div>
        <div style={{ flex: 1, overflow: 'auto', padding: '12px 22px', minHeight: 240 }}>
          {tab === 'raw' || !parsed ? (
            <pre style={{ margin: 0, fontFamily: t.mono, fontSize: 11, lineHeight: 1.6, color: t.fg2, whiteSpace: 'pre-wrap', wordBreak: 'break-all' }}>{json}</pre>
          ) : layers.length === 0 ? (
            <div style={{ fontSize: 12, color: t.fg3, fontStyle: 'italic' }}>No layer history reported by the runtime.</div>
          ) : (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
              {layers.map((l, i) => (
                <div key={i} style={{
                  padding: '8px 10px',
                  background: l.emptyLayer ? 'transparent' : t.surfaceAlt,
                  border: `1px solid ${t.border}`,
                  borderRadius: 6,
                  opacity: l.emptyLayer ? 0.65 : 1,
                  display: 'grid', gridTemplateColumns: '40px 1fr auto', gap: 10, alignItems: 'baseline',
                }}>
                  <span style={{ fontFamily: t.mono, fontSize: 10, color: t.fg3, fontVariantNumeric: 'tabular-nums' }}>
                    #{i + 1}
                  </span>
                  <div style={{ minWidth: 0 }}>
                    <div style={{ fontFamily: t.mono, fontSize: 12, color: t.fg1, wordBreak: 'break-all', whiteSpace: 'pre-wrap' }}>
                      {prettyCmd(l.createdBy) || <span style={{ color: t.fg3 }}>(no command)</span>}
                    </div>
                    {(l.comment || l.diffId) && (
                      <div style={{ fontFamily: t.mono, fontSize: 10, color: t.fg3, marginTop: 4, wordBreak: 'break-all' }}>
                        {l.comment && <span>{l.comment}{l.diffId ? ' · ' : ''}</span>}
                        {l.diffId && <span>{l.diffId}</span>}
                      </div>
                    )}
                  </div>
                  <div style={{ textAlign: 'right' }}>
                    <div style={{ fontFamily: t.mono, fontSize: 10, color: t.fg3, whiteSpace: 'nowrap' }}>{ago(l.created)}</div>
                    {l.emptyLayer && (
                      <div style={{ fontFamily: t.mono, fontSize: 9, color: t.fg3, marginTop: 2, letterSpacing: '0.04em', textTransform: 'uppercase' }}>empty</div>
                    )}
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
        <div style={{ padding: '12px 22px', borderTop: `1px solid ${t.border}`, display: 'flex', gap: 8, justifyContent: 'flex-end', background: t.surfaceAlt }}>
          <button style={pillBtn(t)} onClick={() => navigator.clipboard?.writeText(json)}>Copy JSON</button>
          <button style={pillBtn(t)} onClick={onClose}>Close</button>
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

// Shown on first launch when Apple's `container` CLI isn't on PATH.
// Primary CTA points users at Apple's official GitHub release page (their
// recommended install path); secondary path is the community-maintained
// Homebrew formula. The "Re-check" button avoids needing a relaunch
// after install — the polling task picks up the CLI on its next tick
// and the modal auto-closes via the `onAvailable` callback.
export function OnboardingModal({ t, onAvailable, onDismiss }: {
  t: ThemeTokens;
  onAvailable: () => void;
  onDismiss: () => void;
}) {
  const [checking, setChecking] = useState(false);
  const [copied, setCopied] = useState(false);

  const recheck = async () => {
    setChecking(true);
    try {
      const ok = await api.runtimeAvailable();
      if (ok) onAvailable();
    } finally {
      setChecking(false);
    }
  };

  const openApple = async () => {
    const url = 'https://github.com/apple/container/releases';
    if ('__TAURI_INTERNALS__' in window) {
      // tauri-plugin-opener: open URL in the system default browser.
      try {
        const { openUrl } = await import('@tauri-apps/plugin-opener');
        await openUrl(url);
        return;
      } catch (e) { console.warn('opener failed, falling back', e); }
    }
    window.open(url, '_blank', 'noopener,noreferrer');
  };

  const copyBrew = async () => {
    await navigator.clipboard?.writeText('brew install container');
    setCopied(true);
    setTimeout(() => setCopied(false), 2500);
  };

  return (
    <Backdrop onClose={onDismiss}>
      <div style={{ width: 560, background: t.surface, border: `1px solid ${t.border}`, borderRadius: 12, overflow: 'hidden', display: 'flex', flexDirection: 'column', boxShadow: '0 24px 80px rgba(0,0,0,0.4)' }}>
        <div style={{ padding: '20px 22px', borderBottom: `1px solid ${t.border}`, display: 'flex', alignItems: 'flex-start', gap: 12 }}>
          <Icon name="info" size={22} color={t.warning} />
          <div style={{ flex: 1 }}>
            <div style={{ fontSize: 17, fontWeight: 600, color: t.fg1 }}>Apple <code style={{ background: t.surfaceAlt, padding: '0 4px', borderRadius: 3, fontFamily: t.mono }}>container</code> CLI not detected</div>
            <div style={{ fontSize: 12, color: t.fg3, marginTop: 4 }}>cgui-visual is showing sample data until the runtime is installed.</div>
          </div>
        </div>

        <div style={{ padding: '18px 22px', display: 'flex', flexDirection: 'column', gap: 16 }}>
          <div>
            <div style={{ fontSize: 11, fontWeight: 600, letterSpacing: '0.08em', textTransform: 'uppercase', color: t.fg3, marginBottom: 8 }}>Recommended — Apple's signed installer</div>
            <button onClick={openApple} style={{
              display: 'flex', alignItems: 'center', gap: 10, width: '100%',
              padding: '12px 14px', background: t.fg1, color: t.bg, border: 'none',
              borderRadius: 8, fontSize: 13, fontFamily: 'inherit', fontWeight: 500,
              cursor: 'pointer', textAlign: 'left',
            }}>
              <Icon name="download" size={16} color={t.bg} />
              <div style={{ flex: 1 }}>
                <div>Download from apple/container releases</div>
                <div style={{ fontSize: 11, opacity: 0.7, marginTop: 2, fontFamily: t.mono }}>github.com/apple/container/releases</div>
              </div>
              <span style={{ fontSize: 14, opacity: 0.7 }}>↗</span>
            </button>
            <div style={{ fontSize: 11, color: t.fg3, marginTop: 8, lineHeight: 1.5 }}>
              Apple's signed <code style={{ fontFamily: t.mono }}>.pkg</code> is the canonical install. Includes update + uninstall scripts and verifies macOS 26+.
            </div>
          </div>

          <div style={{ height: 1, background: t.border }} />

          <div>
            <div style={{ fontSize: 11, fontWeight: 600, letterSpacing: '0.08em', textTransform: 'uppercase', color: t.fg3, marginBottom: 8 }}>Or, with Homebrew</div>
            <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
              <code style={{
                flex: 1, padding: '8px 12px', background: t.bg,
                border: `1px solid ${t.border}`, borderRadius: 6,
                fontFamily: t.mono, fontSize: 12, color: t.fg1,
              }}>brew install container</code>
              <button onClick={copyBrew} style={pillBtn(t)}>{copied ? '✓ Copied' : 'Copy'}</button>
            </div>
          </div>
        </div>

        <div style={{ padding: '12px 22px', borderTop: `1px solid ${t.border}`, display: 'flex', gap: 8, justifyContent: 'space-between', alignItems: 'center', background: t.surfaceAlt }}>
          <span style={{ fontSize: 11, color: t.fg3, fontFamily: t.mono }}>
            {checking ? 'Re-checking…' : 'Already installed?'}
          </span>
          <div style={{ display: 'flex', gap: 8 }}>
            <button onClick={onDismiss} style={pillBtn(t)}>Continue with sample data</button>
            <button onClick={recheck} disabled={checking}
                    style={{ padding: '6px 14px', background: t.accent, color: t.accentInk, border: 'none', borderRadius: 6, fontSize: 12, fontWeight: 500, cursor: checking ? 'wait' : 'pointer', opacity: checking ? 0.6 : 1 }}>
              Re-check
            </button>
          </div>
        </div>
      </div>
    </Backdrop>
  );
}

export function SettingsModal({
  t, onClose, runtime, setRuntime,
  dark, setDark,
  menubarMode, setMenubarMode,
  globalHotkey, setGlobalHotkey,
  notifyOnExit, setNotifyOnExit,
}: {
  t: ThemeTokens; onClose: () => void; runtime: Runtime; setRuntime: (r: Runtime) => void;
  dark: boolean; setDark: (b: boolean) => void;
  menubarMode: boolean; setMenubarMode: (b: boolean) => void;
  globalHotkey: string; setGlobalHotkey: (s: string) => void;
  notifyOnExit: boolean; setNotifyOnExit: (b: boolean) => void;
}) {
  const runtimes: { key: Runtime; path: string }[] = [
    { key: 'container', path: '/usr/local/bin/container' },
    { key: 'docker', path: '/usr/local/bin/docker' },
    { key: 'podman', path: '/opt/homebrew/bin/podman' },
  ];

  // Per-runtime availability probe (B8). undefined = still probing.
  // Probes run once on mount + whenever the modal re-opens.
  const [avail, setAvail] = useState<Record<Runtime, boolean | undefined>>({
    container: undefined, docker: undefined, podman: undefined,
  });
  useEffect(() => {
    let cancelled = false;
    (async () => {
      const results = await Promise.all(runtimes.map(async ({ key }) => {
        try { return [key, await api.probeRuntime(key)] as const; }
        catch { return [key, false] as const; }
      }));
      if (!cancelled) {
        setAvail(prev => {
          const next = { ...prev };
          for (const [k, v] of results) next[k] = v;
          return next;
        });
      }
    })();
    return () => { cancelled = true; };
    // runtimes is a constant array literal; eslint can't see that from a
    // const-in-render perspective so the list of deps stays empty.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

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
          {runtimes.map(({ key, path }) => {
            const ok = avail[key];
            const probing = ok === undefined;
            const installed = ok === true;
            return (
              <label key={key} style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '10px 12px', borderRadius: 6, background: key === runtime ? t.selected : 'transparent', border: `1px solid ${key === runtime ? t.accent : t.border}`, marginBottom: 6, cursor: 'pointer' }}>
                <input type="radio" checked={key === runtime} onChange={() => setRuntime(key)} style={{ accentColor: t.accent }} />
                <div style={{ flex: 1 }}>
                  <div style={{ fontSize: 13, color: t.fg1, fontWeight: 500, fontFamily: t.mono }}>{key}</div>
                  <div style={{ fontSize: 11, color: t.fg3, fontFamily: t.mono }}>{path}</div>
                </div>
                {/* Availability badge: probing / installed / missing */}
                <span style={{
                  fontSize: 10, fontFamily: t.mono, letterSpacing: '0.04em',
                  padding: '3px 8px', borderRadius: 999,
                  color: probing ? t.fg3 : (installed ? t.success : t.danger),
                  background: probing ? t.surfaceAlt
                    : (installed ? `${t.success}22` : `${t.danger}22`),
                  border: `1px solid ${probing ? t.border : (installed ? t.success : t.danger)}`,
                }}>
                  {probing ? '… probing' : (installed ? '● installed' : '○ missing')}
                </span>
                {key === runtime && <span style={{ fontSize: 10, color: t.success, fontFamily: t.mono }}>● ACTIVE</span>}
              </label>
            );
          })}
          <div style={{ fontSize: 11, fontWeight: 600, letterSpacing: '0.08em', textTransform: 'uppercase', color: t.fg3, margin: '20px 0 10px' }}>Appearance</div>
          <SettingsToggle t={t} label="Dark mode" hint="Switch the UI to a light theme when off." value={dark} onChange={setDark} />

          <div style={{ fontSize: 11, fontWeight: 600, letterSpacing: '0.08em', textTransform: 'uppercase', color: t.fg3, margin: '20px 0 10px' }}>Menu bar</div>
          <SettingsToggle
            t={t}
            label="Menu-bar mode"
            hint="Closing the main window keeps cgui running in the menu bar. The tray icon shows the running container count."
            value={menubarMode}
            onChange={setMenubarMode}
          />
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '10px 0', borderBottom: `1px solid ${t.border}`, gap: 12 }}>
            <div style={{ minWidth: 0 }}>
              <div style={{ fontSize: 13, color: t.fg1 }}>Global summon hotkey</div>
              <div style={{ fontSize: 11, color: t.fg3, marginTop: 2 }}>e.g. <span style={{ fontFamily: t.mono }}>CmdOrCtrl+Alt+Space</span>. Empty disables.</div>
            </div>
            <input
              value={globalHotkey}
              onChange={e => setGlobalHotkey(e.target.value)}
              placeholder="(disabled)"
              spellCheck={false}
              style={{
                width: 220,
                padding: '6px 10px',
                background: t.surfaceAlt,
                color: t.fg1,
                border: `1px solid ${t.border}`,
                borderRadius: 4,
                fontFamily: t.mono,
                fontSize: 12,
                outline: 'none',
              }}
            />
          </div>

          <div style={{ fontSize: 11, fontWeight: 600, letterSpacing: '0.08em', textTransform: 'uppercase', color: t.fg3, margin: '20px 0 10px' }}>Notifications</div>
          <SettingsToggle
            t={t}
            label="Notify on container exit"
            hint="Show a macOS notification when a running container exits. Non-zero exit codes are highlighted."
            value={notifyOnExit}
            onChange={setNotifyOnExit}
          />

          <div style={{ fontSize: 11, fontWeight: 600, letterSpacing: '0.08em', textTransform: 'uppercase', color: t.fg3, margin: '20px 0 10px' }}>State</div>
          <div style={{ fontFamily: t.mono, fontSize: 11, color: t.fg3 }}>~/.config/cgui-gui/state.json — auto-saved</div>
        </div>
      </div>
    </Backdrop>
  );
}

// Tiny labeled-toggle row used by SettingsModal. Pure CSS switch so the
// modal stays a leaf chunk with no extra deps.
function SettingsToggle({ t, label, hint, value, onChange }: {
  t: ThemeTokens; label: string; hint?: string;
  value: boolean; onChange: (v: boolean) => void;
}) {
  return (
    <div style={{
      display: 'flex', alignItems: 'center', justifyContent: 'space-between',
      padding: '10px 0', borderBottom: `1px solid ${t.border}`, gap: 12,
    }}>
      <div style={{ minWidth: 0 }}>
        <div style={{ fontSize: 13, color: t.fg1 }}>{label}</div>
        {hint && <div style={{ fontSize: 11, color: t.fg3, marginTop: 2 }}>{hint}</div>}
      </div>
      <button
        onClick={() => onChange(!value)}
        aria-pressed={value}
        style={{
          flex: '0 0 auto',
          width: 38, height: 22, borderRadius: 999,
          background: value ? t.accent : t.surfaceAlt,
          border: `1px solid ${value ? t.accent : t.border}`,
          position: 'relative', cursor: 'pointer', transition: 'background 120ms ease',
        }}
      >
        <span style={{
          position: 'absolute', top: 1, left: value ? 17 : 1,
          width: 18, height: 18, borderRadius: '50%',
          background: '#fff',
          transition: 'left 120ms ease',
        }} />
      </button>
    </div>
  );
}

// ─── Command palette ──────────────────────────────────────────────────
//
// Cmd-K opens a floating fuzzy-search box with three result classes:
//
//   • Tab    — switch the main view (containers / images / volumes / …)
//   • Action — global affordances (open Settings, Doctor, Pull, ...)
//   • Entity — a specific Container / Image / Stack to inspect
//
// Selecting an entity routes through `onPick` which dispatches the
// right modal/tab change in App.tsx. Match scoring is a tiny subsequence
// fuzzy match — fast enough for thousands of rows and zero deps.

export type CommandKind = 'tab' | 'action' | 'container' | 'image' | 'stack';
export type CommandResult =
  | { kind: 'tab'; tab: Tab; label: string }
  | { kind: 'action'; id: string; label: string }
  | { kind: 'container'; container: Container; label: string }
  | { kind: 'image'; image: Image; label: string }
  | { kind: 'stack'; stack: Stack; label: string };

// Lightweight subsequence scorer: returns Infinity on miss, a smaller
// number for a tighter match. Letters in `q` must appear in `text` in
// order; matches at word starts cost less than mid-word matches.
export function fuzzyScore(text: string, q: string): number {
  if (!q) return 0;
  const lt = text.toLowerCase();
  const lq = q.toLowerCase();
  let i = 0, j = 0, score = 0, lastMatch = -1;
  while (i < lt.length && j < lq.length) {
    if (lt[i] === lq[j]) {
      // Word-start bonus: matches at index 0 or just after non-alnum
      // are cheap. Consecutive matches stay cheap; gaps add cost.
      const wordStart = i === 0 || !/[a-z0-9]/i.test(lt[i - 1]);
      score += wordStart ? 0 : (lastMatch === i - 1 ? 1 : 4);
      lastMatch = i;
      j++;
    }
    i++;
  }
  return j === lq.length ? score : Infinity;
}

export function CommandPaletteModal({
  t, onClose,
  containers, images, stacks,
  onTab, onAction, onContainer, onImage, onStack,
}: {
  t: ThemeTokens; onClose: () => void;
  containers: Container[]; images: Image[]; stacks: Stack[];
  onTab: (tab: Tab) => void;
  onAction: (id: string) => void;
  onContainer: (c: Container) => void;
  onImage: (img: Image) => void;
  onStack: (s: Stack) => void;
}) {
  const [q, setQ] = useState('');
  const [active, setActive] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  useEffect(() => { inputRef.current?.focus(); }, []);

  // Build the universe of command targets, then filter + score against q.
  // Limit to top 12 results so navigation stays one-screen.
  const all: CommandResult[] = useMemo(() => {
    const tabs: CommandResult[] = [
      { kind: 'tab', tab: 'containers', label: 'Tab: Containers' },
      { kind: 'tab', tab: 'images', label: 'Tab: Images' },
      { kind: 'tab', tab: 'volumes', label: 'Tab: Volumes' },
      { kind: 'tab', tab: 'networks', label: 'Tab: Networks' },
      { kind: 'tab', tab: 'stacks', label: 'Tab: Stacks' },
      { kind: 'tab', tab: 'logs', label: 'Tab: Logs' },
    ];
    const actions: CommandResult[] = [
      { kind: 'action', id: 'settings', label: 'Open Settings…' },
      { kind: 'action', id: 'doctor', label: 'Run Doctor checks' },
      { kind: 'action', id: 'pull', label: 'Pull image…' },
    ];
    const cs: CommandResult[] = containers.map(c => ({
      kind: 'container', container: c, label: `${c.name} · ${c.image}`,
    }));
    const ims: CommandResult[] = images.map(i => ({
      kind: 'image', image: i, label: `${i.ref}`,
    }));
    const ss: CommandResult[] = stacks.map(s => ({
      kind: 'stack', stack: s, label: `${s.name} · ${s.services.length} svc`,
    }));
    return [...tabs, ...actions, ...cs, ...ims, ...ss];
  }, [containers, images, stacks]);

  const ranked = useMemo(() => {
    const trimmed = q.trim();
    const scored = all
      .map(r => ({ r, score: fuzzyScore(r.label, trimmed) }))
      .filter(x => x.score !== Infinity);
    scored.sort((a, b) => a.score - b.score);
    return scored.slice(0, 12).map(x => x.r);
  }, [all, q]);

  // Keep the active index in range when results shrink under the cursor.
  useEffect(() => {
    if (active >= ranked.length) setActive(0);
  }, [ranked.length, active]);

  const dispatch = (r: CommandResult) => {
    onClose();
    if (r.kind === 'tab') onTab(r.tab);
    else if (r.kind === 'action') onAction(r.id);
    else if (r.kind === 'container') onContainer(r.container);
    else if (r.kind === 'image') onImage(r.image);
    else if (r.kind === 'stack') onStack(r.stack);
  };

  const onKey = (e: React.KeyboardEvent) => {
    if (e.key === 'ArrowDown') { e.preventDefault(); setActive(i => Math.min(ranked.length - 1, i + 1)); }
    else if (e.key === 'ArrowUp') { e.preventDefault(); setActive(i => Math.max(0, i - 1)); }
    else if (e.key === 'Enter') { e.preventDefault(); if (ranked[active]) dispatch(ranked[active]); }
  };

  const kindBadge: Record<CommandKind, { label: string; color: string }> = {
    tab:       { label: 'Tab',       color: t.fg3 },
    action:    { label: 'Action',    color: t.accent },
    container: { label: 'Container', color: t.success },
    image:     { label: 'Image',     color: t.warning },
    stack:     { label: 'Stack',     color: t.accent },
  };

  return (
    <Backdrop onClose={onClose}>
      <div style={{ width: 580, background: t.surface, border: `1px solid ${t.border}`, borderRadius: 12, overflow: 'hidden', boxShadow: '0 24px 80px rgba(0,0,0,0.45)' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '12px 16px', borderBottom: `1px solid ${t.border}` }}>
          <Icon name="info" size={16} color={t.fg3} />
          <input
            ref={inputRef}
            value={q}
            onChange={e => setQ(e.target.value)}
            onKeyDown={onKey}
            placeholder="Type a tab, action, container, image, or stack…"
            spellCheck={false}
            style={{
              flex: 1, background: 'transparent', border: 'none', outline: 'none',
              fontSize: 14, color: t.fg1, fontFamily: 'inherit',
            }}
          />
          <span style={{ fontSize: 10, color: t.fg3, fontFamily: t.mono, padding: '2px 6px', background: t.surfaceAlt, border: `1px solid ${t.border}`, borderRadius: 4 }}>esc</span>
        </div>
        <div style={{ maxHeight: 360, overflow: 'auto' }}>
          {ranked.length === 0 ? (
            <div style={{ padding: '20px 16px', fontSize: 12, color: t.fg3, fontStyle: 'italic' }}>No matches.</div>
          ) : ranked.map((r, i) => {
            const k = kindBadge[r.kind];
            const sel = i === active;
            return (
              <button
                key={`${r.kind}-${i}`}
                onMouseEnter={() => setActive(i)}
                onClick={() => dispatch(r)}
                style={{
                  width: '100%', textAlign: 'left',
                  display: 'flex', alignItems: 'center', gap: 10,
                  padding: '10px 16px',
                  background: sel ? t.selected : 'transparent',
                  border: 'none', borderLeft: `2px solid ${sel ? t.accent : 'transparent'}`,
                  cursor: 'pointer', color: 'inherit',
                }}
              >
                <span style={{
                  fontFamily: t.mono, fontSize: 10, fontWeight: 700,
                  color: k.color,
                  letterSpacing: '0.05em', textTransform: 'uppercase',
                  width: 64, flexShrink: 0,
                }}>{k.label}</span>
                <span style={{ flex: 1, fontSize: 13, color: t.fg1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{r.label}</span>
                {sel && <span style={{ fontSize: 10, color: t.fg3, fontFamily: t.mono }}>↵</span>}
              </button>
            );
          })}
        </div>
        <div style={{ padding: '8px 14px', borderTop: `1px solid ${t.border}`, background: t.surfaceAlt, fontSize: 10, fontFamily: t.mono, color: t.fg3, display: 'flex', gap: 12 }}>
          <span>↑↓ navigate</span><span>↵ open</span><span>esc close</span>
          <div style={{ flex: 1 }} />
          <span>{ranked.length} result{ranked.length === 1 ? '' : 's'}</span>
        </div>
      </div>
    </Backdrop>
  );
}
