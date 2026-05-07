// Tab views: Containers / Images / Volumes / Networks / Stacks / Logs.
// Workbench variation only.

import { Fragment, useEffect, useMemo, useRef, useState } from 'react';
import type { ThemeTokens } from './theme';
import type { Container, Image, Volume, Network, Stack } from './types';
import { Icon, Sparkline, Bar, StatusDot, BulkActionBar, SelectCheckbox, iconBtn, pillBtn, tableHeader, tableRow } from './components';
import { api } from './api';
import { withToast } from './toast';

// Aggregate CPU / Memory / Network I/O / Disk I/O across the live container
// list. CPU sparkline zip-sums per-container cpuHistory; the others render
// flat at the current total since the backend doesn't keep per-tick history
// for those (yet).
function buildKPIs(containers: Container[]) {
  const cpuTotal  = containers.reduce((s, c) => s + c.cpu, 0);
  const memTotal  = containers.reduce((s, c) => s + c.mem.used, 0);
  const netTotal  = containers.reduce((s, c) => s + (c.netIoBps ?? 0), 0) / (1024 * 1024);
  const diskTotal = containers.reduce((s, c) => s + (c.diskIoBps ?? 0), 0) / (1024 * 1024);
  const len = containers[0]?.cpuHistory?.length ?? 24;
  const cpuSpark = Array.from({ length: len }, (_, i) =>
    containers.reduce((s, c) => s + (c.cpuHistory?.[i] ?? 0), 0)
  );
  return [
    { l: 'CPU',         v: cpuTotal.toFixed(1),   u: '%',    spark: cpuSpark },
    { l: 'Memory',      v: memTotal.toFixed(1),   u: 'GiB',  spark: Array(len).fill(memTotal) },
    { l: 'Network I/O', v: netTotal.toFixed(2),   u: 'MB/s', spark: Array(len).fill(netTotal) },
    { l: 'Disk I/O',    v: diskTotal.toFixed(2),  u: 'MB/s', spark: Array(len).fill(diskTotal) },
  ];
}

const COLS_CONTAINERS = '24px 1fr 1.2fr 80px 110px 90px 110px 100px';

export function ContainersView({ t, search, selected, setSelected, onInspect, onLogs, containers }: {
  t: ThemeTokens; search: string;
  selected: string; setSelected: (id: string) => void;
  onInspect: (c: Container) => void; onLogs: (c: Container) => void;
  containers: Container[];
}) {
  const rows = useMemo(() => containers.filter(c =>
    !search ||
    c.name.toLowerCase().includes(search.toLowerCase()) ||
    c.image.toLowerCase().includes(search.toLowerCase()) ||
    c.id.toLowerCase().includes(search.toLowerCase())
  ), [containers, search]);

  const kpis = useMemo(() => buildKPIs(containers), [containers]);

  // Bulk selection state (A5). Stored as a Set of ids; pruned to live ids
  // on every container poll so a stopped+removed container can't be
  // targeted by an action it can no longer satisfy.
  const [picked, setPicked] = useState<Set<string>>(new Set());
  useEffect(() => {
    const live = new Set(containers.map(c => c.id));
    setPicked(prev => {
      let changed = false;
      const next = new Set<string>();
      for (const id of prev) {
        if (live.has(id)) next.add(id);
        else changed = true;
      }
      return changed ? next : prev;
    });
  }, [containers]);
  const pickedRows = useMemo(
    () => rows.filter(c => picked.has(c.id)),
    [rows, picked],
  );
  const allChecked = rows.length > 0 && rows.every(c => picked.has(c.id));
  const someChecked = picked.size > 0 && !allChecked;
  const toggleId = (id: string) => setPicked(prev => {
    const next = new Set(prev);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    return next;
  });
  const toggleAll = () => setPicked(prev => {
    if (rows.every(c => prev.has(c.id))) return new Set();
    return new Set(rows.map(c => c.id));
  });

  // Bulk action runner. Wraps the per-id call set in a single confirm,
  // settles in parallel, and toasts pass/fail counts.
  const bulkRun = async (verb: string, ids: string[], op: (id: string) => Promise<unknown>) => {
    if (!ids.length) return;
    if (!confirm(`${verb} ${ids.length} container${ids.length === 1 ? '' : 's'}?`)) return;
    const results = await Promise.allSettled(ids.map(op));
    const ok = results.filter(r => r.status === 'fulfilled').length;
    const fail = results.length - ok;
    if (fail === 0) {
      withToast(`${verb} ${ok}`, Promise.resolve()).catch(() => {});
    } else if (ok === 0) {
      withToast(`${verb} ${ids.length}`, Promise.reject(new Error('all calls failed'))).catch(() => {});
    } else {
      withToast(`${verb} ${ok}/${ids.length}`,
        Promise.reject(new Error(`${fail} failed`))).catch(() => {});
    }
    setPicked(new Set());
  };

  const grouped = useMemo(() => {
    const m = new Map<string, Container[]>();
    rows.forEach(c => {
      const k = c.stack || '__solo';
      if (!m.has(k)) m.set(k, []);
      m.get(k)!.push(c);
    });
    return [...m.entries()].map(([k, items]) => ({ stack: k === '__solo' ? null : k, items }));
  }, [rows]);

  return (
    <div style={{ flex: 1, overflow: 'auto', background: t.bg }}>
      <div style={{ padding: '16px 20px 8px', display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: 16 }}>
        {kpis.map((m, i) => {
          const max = Math.max(1, ...m.spark) * 1.1;
          return (
            <div key={i} style={{ background: t.surface, border: `1px solid ${t.border}`, borderRadius: 8, padding: '14px 16px' }}>
              <div style={{ fontSize: 11, color: t.fg3, fontWeight: 500, letterSpacing: '0.04em', textTransform: 'uppercase' }}>{m.l}</div>
              <div style={{ display: 'flex', alignItems: 'baseline', gap: 6, marginTop: 4 }}>
                <span style={{ fontSize: 22, fontWeight: 600, color: t.fg1, fontVariantNumeric: 'tabular-nums', letterSpacing: '-0.01em' }}>{m.v}</span>
                <span style={{ fontSize: 12, color: t.fg3, fontFamily: t.mono }}>{m.u}</span>
              </div>
              <div style={{ marginTop: 6 }}>
                <Sparkline data={m.spark} w={200} h={26} color={t.sparkline} max={max} />
              </div>
            </div>
          );
        })}
      </div>

      <div style={{ ...tableHeader(t, COLS_CONTAINERS) }}>
        <SelectCheckbox
          t={t}
          checked={allChecked}
          indeterminate={someChecked}
          onChange={toggleAll}
          title={allChecked ? 'Clear selection' : 'Select all visible'}
        />
        <span>Name</span><span>Image</span><span>Status</span>
        <span style={{ textAlign: 'right' }}>CPU</span>
        <span style={{ textAlign: 'right' }}>Memory</span>
        <span>Ports</span>
        <span style={{ textAlign: 'right' }}>Uptime</span>
      </div>

      <BulkActionBar t={t} count={pickedRows.length} onClear={() => setPicked(new Set())}>
        <button style={pillBtn(t, t.fg2)}
                disabled={!pickedRows.some(c => c.status === 'running')}
                onClick={() => bulkRun('Stop', pickedRows.filter(c => c.status === 'running').map(c => c.id), api.stopContainer)}>
          <Icon name="stop" size={11} color={t.fg2} />Stop
        </button>
        <button style={pillBtn(t, t.success)}
                disabled={!pickedRows.some(c => c.status !== 'running')}
                onClick={() => bulkRun('Start', pickedRows.filter(c => c.status !== 'running').map(c => c.id), api.startContainer)}>
          <Icon name="play" size={11} color={t.success} />Start
        </button>
        <button style={pillBtn(t)}
                onClick={() => bulkRun('Restart', pickedRows.map(c => c.id), api.restartContainer)}>
          <Icon name="play" size={11} color={t.fg2} />Restart
        </button>
        <button style={pillBtn(t, t.danger)}
                onClick={() => bulkRun('Delete', pickedRows.map(c => c.id), api.deleteContainer)}>
          <Icon name="trash" size={11} color={t.danger} />Delete
        </button>
      </BulkActionBar>

      {grouped.map((g, gi) => (
        <Fragment key={gi}>
          {g.stack && (
            <div style={{ padding: '10px 20px 4px', display: 'flex', alignItems: 'center', gap: 8, fontSize: 11, color: t.fg3, fontWeight: 600, letterSpacing: '0.06em', textTransform: 'uppercase', background: t.bg }}>
              <Icon name="layers" size={11} color={t.fg3} />
              <span>{g.stack}</span>
              <span style={{ flex: 1, height: 1, background: t.border, marginLeft: 4 }} />
            </div>
          )}
          {g.items.map(c => {
            const sel = selected === c.id;
            const isPicked = picked.has(c.id);
            return (
              <div key={c.id} onClick={() => setSelected(c.id)}
                style={{
                  ...tableRow(t, COLS_CONTAINERS),
                  background: sel ? t.selected : (isPicked ? t.hover : 'transparent'),
                  borderLeft: sel ? `2px solid ${t.accent}` : '2px solid transparent',
                }}
                onMouseEnter={e => { if (!sel && !isPicked) (e.currentTarget as HTMLDivElement).style.background = t.hover; }}
                onMouseLeave={e => { if (!sel && !isPicked) (e.currentTarget as HTMLDivElement).style.background = 'transparent'; }}
              >
                <SelectCheckbox
                  t={t}
                  checked={isPicked}
                  onChange={() => toggleId(c.id)}
                  onClick={e => e.stopPropagation()}
                  title={isPicked ? 'Deselect' : 'Select'}
                />
                <div style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                  <div style={{ fontWeight: 500, color: t.fg1 }}>{c.name}</div>
                  <div style={{ fontSize: 11, color: t.fg3, fontFamily: t.mono, marginTop: 1 }}>{c.id}</div>
                </div>
                <div style={{ fontFamily: t.mono, fontSize: 12, color: t.fg2, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{c.image}</div>
                <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                  <StatusDot status={c.status} t={t} />
                  <span style={{ fontSize: 12, color: c.status === 'running' ? t.success : c.status === 'paused' ? t.warning : t.fg3, textTransform: 'capitalize' }}>{c.status}</span>
                </div>
                <div style={{ display: 'flex', alignItems: 'center', gap: 6, justifyContent: 'flex-end' }}>
                  <Sparkline data={c.cpuHistory.slice(-16)} w={40} h={14} color={c.cpu > 85 ? t.danger : c.cpu > 60 ? t.warning : t.sparkline} />
                  <span style={{ fontFamily: t.mono, fontSize: 12, color: c.cpu > 85 ? t.danger : c.cpu > 60 ? t.warning : t.fg2, fontVariantNumeric: 'tabular-nums', minWidth: 38, textAlign: 'right' }}>{c.cpu.toFixed(1)}%</span>
                </div>
                <div style={{ textAlign: 'right' }}>
                  <div style={{ fontFamily: t.mono, fontSize: 11, color: t.fg2, fontVariantNumeric: 'tabular-nums' }}>{c.mem.used.toFixed(1)}/{c.mem.limit.toFixed(0)}</div>
                  <Bar pct={c.mem.pct} color={c.mem.pct > 85 ? t.danger : c.mem.pct > 70 ? t.warning : t.success} bg={t.surfaceAlt} h={3} />
                </div>
                <div style={{ fontFamily: t.mono, fontSize: 11, color: t.fg3, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{c.ports.join(', ') || '—'}</div>
                <div style={{ display: 'flex', alignItems: 'center', gap: 4, justifyContent: 'flex-end' }}>
                  <span style={{ fontFamily: t.mono, fontSize: 11, color: t.fg3, marginRight: 8 }}>{c.uptime}</span>
                  <button onClick={e => { e.stopPropagation(); onInspect(c); }} style={iconBtn()} title="Inspect"><Icon name="info" size={13} color={t.fg2} /></button>
                  <button onClick={e => { e.stopPropagation(); onLogs(c); }} style={iconBtn()} title="Logs"><Icon name="logs" size={13} color={t.fg2} /></button>
                  {c.status === 'running'
                    ? <button onClick={e => { e.stopPropagation(); withToast(`stop ${c.name}`, api.stopContainer(c.id)).catch(() => {}); }} style={iconBtn()} title="Stop"><Icon name="stop" size={13} color={t.fg2} /></button>
                    : <button onClick={e => { e.stopPropagation(); withToast(`start ${c.name}`, api.startContainer(c.id)).catch(() => {}); }} style={iconBtn()} title="Start"><Icon name="play" size={13} color={t.success} /></button>}
                </div>
              </div>
            );
          })}
        </Fragment>
      ))}
    </div>
  );
}

// ─── Images ───────────────────────────────────────────────────────────
const COLS_IMAGES = '32px 1fr 100px 110px 100px 140px';

export function ImagesView({ t, search, onScan, onRun, onInspect }: {
  t: ThemeTokens; search: string;
  onScan: (img: Image) => void;
  onRun:  (img: Image) => void;
  onInspect: (img: Image) => void;
}) {
  const [items, setItems] = useState<Image[]>([]);
  const reload = () => api.listImages().then(setItems);
  useEffect(() => { reload(); }, []);
  const rows = items.filter(i => !search || i.ref.toLowerCase().includes(search.toLowerCase()));
  const onDelete = (img: Image) => {
    if (!confirm(`Delete image ${img.ref}?\nThis cannot be undone.`)) return;
    withToast(`delete ${img.ref}`, api.deleteImage(img.ref)).then(reload).catch(() => {});
  };

  // Bulk selection (A5). Identical pattern to ContainersView; pruned to
  // visible refs whenever the underlying list refreshes.
  const [picked, setPicked] = useState<Set<string>>(new Set());
  useEffect(() => {
    const live = new Set(items.map(i => i.ref));
    setPicked(prev => {
      const next = new Set<string>();
      let changed = false;
      for (const r of prev) {
        if (live.has(r)) next.add(r);
        else changed = true;
      }
      return changed ? next : prev;
    });
  }, [items]);
  const allChecked = rows.length > 0 && rows.every(r => picked.has(r.ref));
  const someChecked = picked.size > 0 && !allChecked;
  const toggleRef = (ref: string) => setPicked(prev => {
    const next = new Set(prev);
    if (next.has(ref)) next.delete(ref); else next.add(ref);
    return next;
  });
  const toggleAll = () => setPicked(prev => {
    if (rows.every(r => prev.has(r.ref))) return new Set();
    return new Set(rows.map(r => r.ref));
  });
  const bulkDelete = async () => {
    const refs = [...picked];
    if (!refs.length) return;
    if (!confirm(`Delete ${refs.length} image${refs.length === 1 ? '' : 's'}?\nThis cannot be undone.`)) return;
    const results = await Promise.allSettled(refs.map(r => api.deleteImage(r)));
    const ok = results.filter(r => r.status === 'fulfilled').length;
    const fail = results.length - ok;
    if (fail === 0) withToast(`Delete ${ok}`, Promise.resolve()).catch(() => {});
    else if (ok === 0) withToast(`Delete ${refs.length}`, Promise.reject(new Error('all calls failed'))).catch(() => {});
    else withToast(`Delete ${ok}/${refs.length}`, Promise.reject(new Error(`${fail} failed`))).catch(() => {});
    setPicked(new Set());
    reload();
  };

  return (
    <div style={{ flex: 1, overflow: 'auto', background: t.bg }}>
      <div style={tableHeader(t, COLS_IMAGES)}>
        <SelectCheckbox
          t={t}
          checked={allChecked}
          indeterminate={someChecked}
          onChange={toggleAll}
          title={allChecked ? 'Clear selection' : 'Select all visible'}
        />
        <span>Reference</span>
        <span style={{ textAlign: 'right' }}>Size</span>
        <span style={{ textAlign: 'right' }}>Layers</span>
        <span>Created</span><span style={{ textAlign: 'right' }}>Actions</span>
      </div>
      <BulkActionBar t={t} count={picked.size} onClear={() => setPicked(new Set())}>
        <button style={pillBtn(t, t.danger)} onClick={bulkDelete}>
          <Icon name="trash" size={11} color={t.danger} />Delete
        </button>
      </BulkActionBar>
      {rows.map(img => (
        <div key={img.id} style={{
          ...tableRow(t, COLS_IMAGES),
          background: picked.has(img.ref) ? t.hover : undefined,
        }}>
          <SelectCheckbox
            t={t}
            checked={picked.has(img.ref)}
            onChange={() => toggleRef(img.ref)}
            title={picked.has(img.ref) ? 'Deselect' : 'Select'}
          />
          <div>
            <div style={{ fontFamily: t.mono, fontSize: 13, color: t.fg1 }}>{img.ref}</div>
            <div style={{ fontFamily: t.mono, fontSize: 11, color: t.fg3, marginTop: 2 }}>{img.digest}</div>
          </div>
          <div style={{ textAlign: 'right', fontFamily: t.mono, fontSize: 12, color: t.fg1, fontVariantNumeric: 'tabular-nums' }}>
            {img.size.toFixed(img.size < 1 ? 2 : 1)} <span style={{ color: t.fg3, fontSize: 11 }}>{img.sizeUnit}</span>
          </div>
          <div style={{ textAlign: 'right', fontFamily: t.mono, fontSize: 12, color: t.fg2 }}>{img.layers}</div>
          <div style={{ fontFamily: t.mono, fontSize: 12, color: t.fg3 }}>{img.created}</div>
          <div style={{ display: 'flex', gap: 4, justifyContent: 'flex-end' }}>
            <button onClick={() => onInspect(img)} style={iconBtn()} title="Inspect"><Icon name="info" size={13} color={t.fg2} /></button>
            <button onClick={() => onScan(img)} style={iconBtn()} title="Trivy scan"><Icon name="shield" size={13} color={t.fg2} /></button>
            <button onClick={() => onRun(img)} style={iconBtn()} title="Run"><Icon name="play" size={13} color={t.fg2} /></button>
            <button onClick={() => onDelete(img)} style={iconBtn()} title="Delete"><Icon name="trash" size={13} color={t.fg2} /></button>
          </div>
        </div>
      ))}
    </div>
  );
}

// ─── Volumes ──────────────────────────────────────────────────────────
export function VolumesView({ t, search, onInspect }: {
  t: ThemeTokens; search: string; onInspect: (v: Volume) => void;
}) {
  const [items, setItems] = useState<Volume[]>([]);
  const reload = () => api.listVolumes().then(setItems);
  useEffect(() => { reload(); }, []);
  const onDelete = (v: Volume) => {
    if (!confirm(`Delete volume ${v.name}?\nAll data in this volume will be lost.`)) return;
    withToast(`delete ${v.name}`, api.deleteVolume(v.name)).then(reload).catch(() => {});
  };
  const q = search.trim().toLowerCase();
  const rows = q
    ? items.filter(v =>
        v.name.toLowerCase().includes(q) ||
        v.driver.toLowerCase().includes(q) ||
        v.mountpoint.toLowerCase().includes(q))
    : items;
  return (
    <div style={{ flex: 1, overflow: 'auto', background: t.bg, padding: 16 }}>
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(380px, 1fr))', gap: 14 }}>
        {rows.map(v => {
          const pct = (v.used / v.size) * 100;
          return (
            <div key={v.name} style={{ background: t.surface, border: `1px solid ${t.border}`, borderRadius: 10, padding: 18 }}>
              <div style={{ display: 'flex', alignItems: 'flex-start', gap: 10 }}>
                <Icon name="database" size={18} color={t.fg2} />
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ fontSize: 14, fontWeight: 500, color: t.fg1, fontFamily: t.mono }}>{v.name}</div>
                  <div style={{ fontSize: 11, color: t.fg3, fontFamily: t.mono, marginTop: 2, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{v.mountpoint}</div>
                </div>
                <div style={{ fontSize: 10, color: t.fg3, fontFamily: t.mono, padding: '2px 6px', background: t.surfaceAlt, borderRadius: 4 }}>{v.driver}</div>
              </div>
              <div style={{ marginTop: 12, display: 'flex', alignItems: 'baseline', justifyContent: 'space-between' }}>
                <div>
                  <span style={{ fontSize: 18, fontWeight: 600, fontFamily: t.mono, color: t.fg1, fontVariantNumeric: 'tabular-nums' }}>{v.used.toFixed(1)}</span>
                  <span style={{ fontSize: 12, color: t.fg3, marginLeft: 4, fontFamily: t.mono }}>/ {v.size.toFixed(0)} {v.unit}</span>
                </div>
                <span style={{ fontSize: 12, color: pct > 90 ? t.danger : pct > 70 ? t.warning : t.fg2, fontFamily: t.mono, fontVariantNumeric: 'tabular-nums' }}>{pct.toFixed(1)}%</span>
              </div>
              <div style={{ marginTop: 8 }}>
                <Bar pct={pct} color={pct > 90 ? t.danger : pct > 70 ? t.warning : t.success} bg={t.surfaceAlt} h={6} />
              </div>
              <div style={{ marginTop: 10, display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                <span style={{ fontSize: 11, color: t.fg3, fontFamily: t.mono }}>{v.refs} {v.refs === 1 ? 'reference' : 'references'}</span>
                <div style={{ display: 'flex', gap: 4 }}>
                  <button onClick={() => onInspect(v)} style={iconBtn()} title="Inspect"><Icon name="info" size={13} color={t.fg2} /></button>
                  <button onClick={() => onDelete(v)} style={iconBtn()} title="Delete volume"><Icon name="trash" size={13} color={t.fg2} /></button>
                </div>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}

// ─── Networks ─────────────────────────────────────────────────────────
const COLS_NETWORKS = '32px 1fr 100px 100px 1.4fr 1.2fr 70px 80px';

export function NetworksView({ t, search, onInspect }: {
  t: ThemeTokens; search: string; onInspect: (n: Network) => void;
}) {
  const [items, setItems] = useState<Network[]>([]);
  const reload = () => api.listNetworks().then(setItems);
  useEffect(() => { reload(); }, []);
  const q = search.trim().toLowerCase();
  const rows = q
    ? items.filter(n =>
        n.name.toLowerCase().includes(q) ||
        n.mode.toLowerCase().includes(q) ||
        n.subnet.toLowerCase().includes(q))
    : items;
  const onDelete = (n: Network) => {
    // System networks (default/host/bridge) generally can't be deleted.
    if (['default', 'host', 'bridge'].includes(n.name)) {
      withToast(`delete ${n.name}`, Promise.reject(new Error('system networks cannot be deleted'))).catch(() => {});
      return;
    }
    if (!confirm(`Delete network ${n.name}?`)) return;
    withToast(`delete ${n.name}`, api.deleteNetwork(n.id)).then(reload).catch(() => {});
  };
  return (
    <div style={{ flex: 1, overflow: 'auto', background: t.bg }}>
      <div style={tableHeader(t, COLS_NETWORKS)}>
        <span></span><span>Name</span><span>Mode</span><span>State</span><span>Subnet</span><span>DNS</span><span style={{ textAlign: 'right' }}>Conn</span><span></span>
      </div>
      {rows.map(n => (
        <div key={n.id} style={tableRow(t, COLS_NETWORKS)}>
          <Icon name="network" size={16} color={t.fg3} />
          <div style={{ fontFamily: t.mono, fontSize: 13, color: t.fg1 }}>{n.name}</div>
          <div style={{ fontFamily: t.mono, fontSize: 12, color: t.fg2 }}>{n.mode}</div>
          <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
            <StatusDot status={n.state === 'active' ? 'running' : 'stopped'} t={t} />
            <span style={{ fontSize: 12, color: n.state === 'active' ? t.success : t.fg3, textTransform: 'capitalize' }}>{n.state}</span>
          </div>
          <div style={{ fontFamily: t.mono, fontSize: 12, color: t.fg2 }}>{n.subnet} <span style={{ color: t.fg3 }}>· gw {n.gateway}</span></div>
          <div style={{ fontFamily: t.mono, fontSize: 12, color: t.fg3 }}>{n.dns.join(', ') || '—'}</div>
          <div style={{ textAlign: 'right', fontFamily: t.mono, fontSize: 12, color: t.fg1, fontVariantNumeric: 'tabular-nums' }}>{n.containers}</div>
          <div style={{ display: 'flex', justifyContent: 'flex-end', gap: 4 }}>
            <button onClick={() => onInspect(n)} style={iconBtn()} title="Inspect"><Icon name="info" size={13} color={t.fg2} /></button>
            <button onClick={() => onDelete(n)} style={iconBtn()} title="Delete network"><Icon name="trash" size={13} color={t.fg2} /></button>
          </div>
        </div>
      ))}
    </div>
  );
}

// ─── Stacks ───────────────────────────────────────────────────────────

const HEALTH_POLL_MS = 5000;

export function StacksView({ t, search }: { t: ThemeTokens; search: string }) {
  const [items, setItems] = useState<Stack[]>([]);
  // Per-stack live overlays. Health: { stackName: { service: state } }.
  // Logs: latest up/down output per stack, displayed as a footer strip.
  const [health, setHealth] = useState<Record<string, Record<string, string>>>({});
  const [logs, setLogs] = useState<Record<string, string[]>>({});
  const [busy, setBusy] = useState<Record<string, boolean>>({});

  const reload = () => api.listStacks().then(setItems);
  useEffect(() => { reload(); }, []);

  // Poll healthchecks for every stack on a 5 s tick. The per-stack call is
  // cheap (TCP connects with 2 s timeout) and runs in parallel server-side.
  useEffect(() => {
    if (!items.length) return;
    let cancelled = false;
    const tick = async () => {
      const results = await Promise.all(items.map(async s => {
        try {
          const rows = await api.stackHealth(s.name);
          return [s.name, Object.fromEntries(rows)] as const;
        } catch { return [s.name, {}] as const; }
      }));
      if (!cancelled) setHealth(Object.fromEntries(results));
    };
    tick();
    const id = window.setInterval(tick, HEALTH_POLL_MS);
    return () => { cancelled = true; window.clearInterval(id); };
  }, [items]);

  const runAction = (name: string, kind: 'up' | 'down') => {
    setBusy(b => ({ ...b, [name]: true }));
    setLogs(l => ({ ...l, [name]: [`$ stack ${kind} ${name}…`] }));
    const p = kind === 'up' ? api.stackUp(name) : api.stackDown(name);
    withToast(`stack ${kind} ${name}`, p)
      .then(out => setLogs(l => ({ ...l, [name]: out })))
      .catch(() => {})
      .finally(() => {
        setBusy(b => ({ ...b, [name]: false }));
        reload();
      });
  };

  const healthColor = (state: string) => {
    if (state === 'healthy') return t.success;
    if (state === 'unhealthy') return t.danger;
    if (state === 'waiting') return t.warning;
    // `starting (12/30s)` from the backend's start-period grace window —
    // treated like `waiting` since the service is booting, not broken.
    if (state.startsWith('starting')) return t.warning;
    return t.fg3;
  };

  // Open the native file picker, run the import, surface the result.
  // Confirms before overwriting an existing stack of the same name.
  const importCompose = async () => {
    let path: string | null = null;
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const picked = await open({
        title: 'Import a docker-compose.yml',
        multiple: false,
        directory: false,
        filters: [{ name: 'Compose', extensions: ['yml', 'yaml'] }],
      });
      path = typeof picked === 'string' ? picked : null;
    } catch (e) {
      // Dialog plugin isn't available outside Tauri — fall back to prompt.
      path = window.prompt('Path to docker-compose.yml');
    }
    if (!path) return;
    try {
      const dest = await api.importCompose(path);
      withToast(`Imported to ${dest}`, Promise.resolve()).catch(() => {});
      await reload();
    } catch (err: any) {
      const msg = String(err?.message ?? err);
      // Conflict: backend refuses overwrite by default. Prompt + retry.
      if (msg.includes('already exists')) {
        if (confirm(`${msg}\n\nOverwrite?`)) {
          try {
            const dest = await api.importCompose(path, true);
            withToast(`Imported to ${dest}`, Promise.resolve()).catch(() => {});
            await reload();
          } catch (e2: any) {
            withToast(`import compose`, Promise.reject(e2)).catch(() => {});
          }
        }
      } else {
        withToast(`import compose`, Promise.reject(err)).catch(() => {});
      }
    }
  };

  return (
    <div style={{ flex: 1, overflow: 'auto', background: t.bg, padding: 16 }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 12 }}>
        <span style={{ fontSize: 12, color: t.fg3, fontFamily: t.mono }}>
          {items.length} stack{items.length === 1 ? '' : 's'}
        </span>
        <div style={{ flex: 1 }} />
        <button onClick={importCompose} style={pillBtn(t, t.accent)}>
          <Icon name="download" size={12} color={t.accent} />Import compose
        </button>
      </div>
      <div style={{ display: 'grid', gap: 12 }}>
        {items
          .filter(s => {
            const q = search.trim().toLowerCase();
            if (!q) return true;
            return s.name.toLowerCase().includes(q)
              || s.file.toLowerCase().includes(q)
              || s.services.some(sv =>
                  sv.name.toLowerCase().includes(q) ||
                  sv.image.toLowerCase().includes(q));
          })
          .map(s => {
          const running = s.services.filter(sv => sv.state === 'running').length;
          const liveHealth = health[s.name] || {};
          const tail = logs[s.name];
          return (
            <div key={s.name} style={{ background: t.surface, border: `1px solid ${t.border}`, borderRadius: 10, padding: 18 }}>
              <div style={{ display: 'flex', alignItems: 'flex-start', gap: 14 }}>
                <Icon name="layers" size={20} color={t.accent} />
                <div style={{ flex: 1 }}>
                  <div style={{ display: 'flex', alignItems: 'baseline', gap: 12, flexWrap: 'wrap' }}>
                    <span style={{ fontSize: 16, fontWeight: 600, color: t.fg1 }}>{s.name}</span>
                    <span style={{ fontSize: 11, color: t.fg3, fontFamily: t.mono }}>{s.file}</span>
                  </div>
                  <div style={{ marginTop: 4, fontSize: 12, color: t.fg3, fontFamily: t.mono }}>
                    <span style={{ color: running === s.services.length ? t.success : running > 0 ? t.warning : t.fg3 }}>{running}/{s.services.length} running</span>
                    {' · '}{s.health}{' · restart '}{s.restart}
                  </div>
                </div>
                <div style={{ display: 'flex', gap: 6 }}>
                  <button style={{ ...pillBtn(t, t.success), opacity: busy[s.name] ? 0.5 : 1 }}
                          disabled={busy[s.name]}
                          onClick={() => runAction(s.name, 'up')}>
                    <Icon name="play" size={12} color={t.success} />Up
                  </button>
                  <button style={{ ...pillBtn(t, t.fg2), opacity: busy[s.name] ? 0.5 : 1 }}
                          disabled={busy[s.name]}
                          onClick={() => {
                            if (!confirm(`Bring stack '${s.name}' down?\nAll its services will be stopped and deleted.`)) return;
                            runAction(s.name, 'down');
                          }}>
                    <Icon name="stop" size={12} color={t.fg2} />Down
                  </button>
                </div>
              </div>
              <div style={{ marginTop: 14, display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(220px, 1fr))', gap: 8 }}>
                {s.services.map(sv => {
                  // Live healthcheck wins over the placeholder derived from state.
                  const live = liveHealth[sv.name];
                  const display = live ?? sv.health;
                  return (
                    <div key={sv.name} style={{ padding: '8px 10px', background: t.surfaceAlt, border: `1px solid ${t.border}`, borderRadius: 6, display: 'flex', alignItems: 'center', gap: 8 }}>
                      <StatusDot status={sv.state} t={t} />
                      <div style={{ flex: 1, minWidth: 0 }}>
                        <div style={{ fontSize: 12, fontWeight: 500, color: t.fg1, fontFamily: t.mono }}>{sv.name}</div>
                        <div style={{ fontSize: 10, color: t.fg3, fontFamily: t.mono, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{sv.image}</div>
                      </div>
                      <span style={{ fontSize: 10, fontFamily: t.mono, color: healthColor(display) }}>{display}</span>
                    </div>
                  );
                })}
              </div>
              {tail && (
                <div style={{ marginTop: 12, padding: '8px 10px', background: t.bg, border: `1px solid ${t.border}`, borderRadius: 6, fontFamily: t.mono, fontSize: 11, lineHeight: 1.5, maxHeight: 120, overflow: 'auto' }}>
                  {tail.map((line, i) => (
                    <div key={i} style={{ color: line.startsWith('✗') ? t.danger : line.startsWith('✓') ? t.success : line.startsWith('$') ? t.fg2 : t.fg3 }}>{line}</div>
                  ))}
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}

// ─── Logs ─────────────────────────────────────────────────────────────
const LOG_BUFFER_MAX = 5000;

type LogLevel = 'all' | 'info' | 'warn' | 'error';

function classifyLevel(line: string): 'info' | 'warn' | 'error' {
  if (line.includes('[ERROR]') || line.includes('[ERR]') || line.includes('FATAL')) return 'error';
  if (line.includes('[WARN]') || line.includes('WARNING')) return 'warn';
  return 'info';
}

export function LogsView({ t, target }: { t: ThemeTokens; target?: string }) {
  const [lines, setLines] = useState<string[]>([]);
  const [paused, setPaused] = useState(false);
  const [level, setLevel] = useState<LogLevel>('all');
  const [search, setSearch] = useState('');
  const pausedRef = useRef(paused);
  pausedRef.current = paused;

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | null = null;
    setLines([]);
    api.onLogLine(line => {
      if (cancelled || pausedRef.current) return;
      setLines(prev => prev.length >= LOG_BUFFER_MAX ? [...prev.slice(-LOG_BUFFER_MAX + 1), line] : [...prev, line]);
    }).then(fn => { if (cancelled) fn(); else unlisten = fn; });
    if (target) api.startLogStream(target).catch(e => console.warn('start_log_stream failed', e));
    return () => { cancelled = true; unlisten?.(); };
  }, [target]);

  // Filtering happens at render time, not on the buffer, so toggling level
  // or search doesn't lose lines that arrived during the previous filter.
  const visible = useMemo(() => {
    const q = search.toLowerCase();
    return lines
      .map((line, idx) => ({ line, idx, lvl: classifyLevel(line) }))
      .filter(({ line, lvl }) =>
        (level === 'all' || lvl === level) &&
        (!q || line.toLowerCase().includes(q))
      );
  }, [lines, level, search]);

  const levelPill = (key: LogLevel, label: string, color?: string) => (
    <button onClick={() => setLevel(level === key ? 'all' : key)}
      style={{
        ...pillBtn(t, color ?? t.fg2),
        background: level === key ? (color ?? t.accent) : t.surfaceAlt,
        color: level === key ? '#fff' : (color ?? t.fg2),
        borderColor: level === key ? (color ?? t.accent) : t.border,
        fontWeight: 600, letterSpacing: '0.04em',
      }}>{label}</button>
  );

  return (
    <div style={{ flex: 1, overflow: 'auto', background: t.surface, padding: 0 }}>
      <div style={{ padding: '10px 20px', display: 'flex', alignItems: 'center', gap: 10, borderBottom: `1px solid ${t.border}`, background: t.surface, position: 'sticky', top: 0, flexWrap: 'wrap' }}>
        <span style={{ width: 8, height: 8, borderRadius: '50%', background: paused ? t.fg3 : t.success, animation: paused ? 'none' : 'pulse 1.5s ease-in-out infinite' }} />
        <span style={{ fontSize: 12, fontFamily: t.mono, color: t.fg1 }}>{paused ? 'paused' : 'follow'}</span>
        <span style={{ color: t.fg3, fontSize: 12, fontFamily: t.mono }}>· {target ?? 'all sources'} · {visible.length}/{lines.length} lines</span>
        <span style={{ width: 1, height: 18, background: t.border, margin: '0 4px' }} />
        {levelPill('error', 'ERROR', t.danger)}
        {levelPill('warn',  'WARN',  t.warning)}
        {levelPill('info',  'INFO')}
        <input value={search} onChange={e => setSearch(e.target.value)} placeholder="Search…"
          style={{ padding: '5px 10px', background: t.surfaceAlt, border: `1px solid ${t.border}`, borderRadius: 6, color: t.fg1, fontSize: 12, fontFamily: t.mono, width: 200, outline: 'none' }} />
        <div style={{ flex: 1 }} />
        <button style={pillBtn(t)} onClick={() => setPaused(p => !p)}>{paused ? 'Resume' : 'Pause'}</button>
        <button style={pillBtn(t)} onClick={() => setLines([])}>Clear</button>
        <button style={pillBtn(t)} onClick={() => navigator.clipboard?.writeText(visible.map(v => v.line).join('\n'))}>Copy</button>
      </div>
      <div style={{ fontFamily: t.mono, fontSize: 12, lineHeight: 1.6, padding: 16 }}>
        {visible.map(({ line, idx, lvl }) => (
          <div key={idx} style={{
            display: 'flex', gap: 12, padding: '1px 4px',
            background: lvl === 'error' ? 'rgba(220,38,38,0.06)' : 'transparent',
            color: lvl === 'error' ? t.danger : lvl === 'warn' ? t.warning : t.fg2,
          }}>
            <span style={{ color: t.fg3, userSelect: 'none', minWidth: 32, textAlign: 'right' }}>{idx + 1}</span>
            <span style={{ whiteSpace: 'pre-wrap', wordBreak: 'break-all' }}>{line}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
