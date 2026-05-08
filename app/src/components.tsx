// Shared chrome/primitives: Icon, Sparkline, Bar, StatusDot, FramelessChrome,
// Sidebar, TopBar, StatusBar — Workbench variation only.

import { memo, useEffect, useRef } from 'react';
import type { CSSProperties, MouseEvent, ReactNode } from 'react';
import type { ThemeTokens } from './theme';
import type { ContainerStatus, Tab, Runtime } from './types';

// ─── utils ─────────────────────────────────────────────────────────────
export function fmtBytes(gib: number, unit = 'GiB'): string {
  if (gib < 0.01) return `${(gib * 1024).toFixed(1)} MiB`;
  if (gib < 1) return `${(gib * 1024).toFixed(0)} MiB`;
  return `${gib.toFixed(gib >= 100 ? 0 : 1)} ${unit}`;
}

// ─── icons ─────────────────────────────────────────────────────────────
type IconName =
  | 'box' | 'image' | 'database' | 'network' | 'layers' | 'terminal'
  | 'play' | 'stop' | 'restart' | 'trash' | 'info' | 'logs' | 'search'
  | 'pause' | 'plus' | 'download' | 'shield' | 'sun' | 'moon' | 'menu'
  | 'check' | 'x' | 'cog' | 'heart' | 'chevron';

export function Icon({ name, size = 14, color = 'currentColor', strokeWidth = 1.6 }: {
  name: IconName; size?: number; color?: string; strokeWidth?: number;
}) {
  const s: CSSProperties = { width: size, height: size, stroke: color, fill: 'none', strokeWidth, strokeLinecap: 'round', strokeLinejoin: 'round', display: 'block' };
  const filled: CSSProperties = { ...s, fill: color, stroke: 'none' };
  switch (name) {
    case 'box': return <svg viewBox="0 0 24 24" style={s}><path d="M3 7l9-4 9 4v10l-9 4-9-4z"/><path d="M3 7l9 4 9-4M12 11v10"/></svg>;
    case 'image': return <svg viewBox="0 0 24 24" style={s}><rect x="3" y="3" width="18" height="18" rx="2"/><circle cx="9" cy="9" r="2"/><path d="M21 15l-5-5L5 21"/></svg>;
    case 'database': return <svg viewBox="0 0 24 24" style={s}><ellipse cx="12" cy="5" rx="8" ry="3"/><path d="M4 5v6c0 1.7 3.6 3 8 3s8-1.3 8-3V5M4 11v6c0 1.7 3.6 3 8 3s8-1.3 8-3v-6"/></svg>;
    case 'network': return <svg viewBox="0 0 24 24" style={s}><circle cx="12" cy="6" r="2.5"/><circle cx="5" cy="18" r="2.5"/><circle cx="19" cy="18" r="2.5"/><path d="M12 8.5v3M10 13l-4 3M14 13l4 3"/></svg>;
    case 'layers': return <svg viewBox="0 0 24 24" style={s}><path d="M12 2l10 5-10 5L2 7l10-5z"/><path d="M2 12l10 5 10-5M2 17l10 5 10-5"/></svg>;
    case 'terminal': return <svg viewBox="0 0 24 24" style={s}><path d="M4 17l5-5-5-5M11 19h8"/></svg>;
    case 'play': return <svg viewBox="0 0 24 24" style={filled}><path d="M6 4l14 8-14 8z"/></svg>;
    case 'stop': return <svg viewBox="0 0 24 24" style={filled}><rect x="6" y="6" width="12" height="12" rx="1"/></svg>;
    case 'restart': return <svg viewBox="0 0 24 24" style={s}><path d="M3 12a9 9 0 1 0 3-6.7M3 4v5h5"/></svg>;
    case 'trash': return <svg viewBox="0 0 24 24" style={s}><path d="M3 6h18M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2M5 6l1 14a2 2 0 0 0 2 2h8a2 2 0 0 0 2-2l1-14"/></svg>;
    case 'info': return <svg viewBox="0 0 24 24" style={s}><circle cx="12" cy="12" r="9"/><path d="M12 16v-5M12 8h.01"/></svg>;
    case 'logs': return <svg viewBox="0 0 24 24" style={s}><path d="M4 4h16v16H4z"/><path d="M8 9h8M8 13h8M8 17h5"/></svg>;
    case 'search': return <svg viewBox="0 0 24 24" style={s}><circle cx="11" cy="11" r="7"/><path d="M21 21l-4.5-4.5"/></svg>;
    case 'pause': return <svg viewBox="0 0 24 24" style={filled}><rect x="6" y="4" width="4" height="16" rx="1"/><rect x="14" y="4" width="4" height="16" rx="1"/></svg>;
    case 'plus': return <svg viewBox="0 0 24 24" style={s}><path d="M12 5v14M5 12h14"/></svg>;
    case 'download': return <svg viewBox="0 0 24 24" style={s}><path d="M12 3v12M7 10l5 5 5-5M5 21h14"/></svg>;
    case 'shield': return <svg viewBox="0 0 24 24" style={s}><path d="M12 2l8 3v7c0 5-4 8-8 10-4-2-8-5-8-10V5z"/></svg>;
    case 'sun': return <svg viewBox="0 0 24 24" style={s}><circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M4.93 19.07l1.41-1.41M17.66 6.34l1.41-1.41"/></svg>;
    case 'moon': return <svg viewBox="0 0 24 24" style={s}><path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"/></svg>;
    case 'menu': return <svg viewBox="0 0 24 24" style={s}><path d="M4 7h16M4 12h16M4 17h16"/></svg>;
    case 'check': return <svg viewBox="0 0 24 24" style={s}><path d="M5 12l5 5L20 7"/></svg>;
    case 'x': return <svg viewBox="0 0 24 24" style={s}><path d="M6 6l12 12M18 6L6 18"/></svg>;
    case 'cog': return <svg viewBox="0 0 24 24" style={s}><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3 1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8 1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z"/></svg>;
    case 'heart': return <svg viewBox="0 0 24 24" style={s}><path d="M20.8 4.6a5.5 5.5 0 0 0-7.8 0L12 5.7l-1-1.1a5.5 5.5 0 0 0-7.8 7.8l1 1L12 21l7.8-7.6 1-1a5.5 5.5 0 0 0 0-7.8z"/></svg>;
    case 'chevron': return <svg viewBox="0 0 24 24" style={s}><path d="M9 18l6-6-6-6"/></svg>;
    default: return null;
  }
}

// ─── small primitives ─────────────────────────────────────────────────
export function StatusDot({ status, t }: { status: ContainerStatus | string; t: ThemeTokens }) {
  const c = status === 'running' ? t.success : status === 'paused' ? t.warning : t.fg3;
  return <span style={{ display: 'inline-block', width: 8, height: 8, borderRadius: '50%', background: c, marginRight: 8, verticalAlign: 'middle', flexShrink: 0 }} />;
}

// Memoized — re-renders only when data, color, or sizing actually change. The
// ContainersView mounts one Sparkline per row and the dashboard mounts four
// more, so this is the highest-traffic visual on every poll tick.
export const Sparkline = memo(function Sparkline({ data, w = 64, h = 18, color, max = 100 }: {
  data: number[]; w?: number; h?: number; color: string; max?: number;
}) {
  const pts = data.map((v, i) => `${(i / (data.length - 1)) * w},${h - (v / max) * h}`).join(' ');
  return (
    <svg width={w} height={h} style={{ display: 'block' }}>
      <polyline points={pts} fill="none" stroke={color} strokeWidth="1.25" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}, (a, b) =>
  a.w === b.w && a.h === b.h && a.color === b.color && a.max === b.max &&
  a.data.length === b.data.length && a.data.every((v, i) => v === b.data[i])
);

export function Bar({ pct, color, bg, h = 6, w = '100%' }: {
  pct: number; color: string; bg: string; h?: number; w?: number | string;
}) {
  return (
    <div style={{ width: w, height: h, background: bg, borderRadius: 999, overflow: 'hidden' }}>
      <div style={{ width: `${Math.min(100, pct)}%`, height: '100%', background: color, transition: 'width .3s' }} />
    </div>
  );
}

// ─── shared style helpers ─────────────────────────────────────────────
export function iconBtn(): CSSProperties {
  return { background: 'transparent', border: 'none', padding: 5, borderRadius: 4, cursor: 'pointer', display: 'flex', alignItems: 'center', justifyContent: 'center' };
}

export function pillBtn(t: ThemeTokens, accent?: string): CSSProperties {
  return { display: 'inline-flex', alignItems: 'center', gap: 4, padding: '4px 10px', background: t.surfaceAlt, border: `1px solid ${t.border}`, borderRadius: 999, color: accent || t.fg2, fontSize: 11, fontFamily: 'inherit', cursor: 'pointer' };
}

export function tableHeader(t: ThemeTokens, cols: string): CSSProperties {
  return {
    display: 'grid', gridTemplateColumns: cols, gap: 12,
    padding: '10px 20px', alignItems: 'center',
    fontSize: 11, color: t.fg3, fontWeight: 600,
    letterSpacing: '0.06em', textTransform: 'uppercase',
    borderBottom: `1px solid ${t.border}`, background: t.surface,
    position: 'sticky', top: 0, zIndex: 1,
  };
}

export function tableRow(t: ThemeTokens, cols: string): CSSProperties {
  return {
    display: 'grid', gridTemplateColumns: cols, gap: 12,
    padding: '12px 20px', alignItems: 'center',
    borderBottom: `1px solid ${t.border}`, fontSize: 13, color: t.fg1,
    cursor: 'pointer',
  };
}

// ─── window chrome ────────────────────────────────────────────────────
// Rounded-corner shell around the app body. macOS native traffic lights are
// drawn by Tauri at the OS level above this surface.
export function FramelessChrome({ children, t }: { children: ReactNode; t: ThemeTokens }) {
  return (
    <div style={{ width: '100%', height: '100%', borderRadius: 12, overflow: 'hidden', background: t.surface, boxShadow: '0 24px 60px rgba(0,0,0,0.30), 0 0 0 0.5px rgba(0,0,0,0.4)', display: 'flex', flexDirection: 'column', border: `1px solid ${t.border}` }}>
      {children}
    </div>
  );
}

// ─── sidebar ──────────────────────────────────────────────────────────
const TABS: { key: Tab; label: string; icon: IconName }[] = [
  { key: 'containers', label: 'Containers', icon: 'box' },
  { key: 'images', label: 'Images', icon: 'image' },
  { key: 'volumes', label: 'Volumes', icon: 'database' },
  { key: 'networks', label: 'Networks', icon: 'network' },
  { key: 'stacks', label: 'Stacks', icon: 'layers' },
  { key: 'logs', label: 'Logs', icon: 'logs' },
];

function Eyebrow({ children, t }: { children: ReactNode; t: ThemeTokens }) {
  return <div style={{ fontSize: 10, fontWeight: 600, letterSpacing: '0.12em', textTransform: 'uppercase', color: t.fg3, padding: '12px 10px 6px' }}>{children}</div>;
}

function sidebarBtn(t: ThemeTokens, collapsed: boolean): CSSProperties {
  return {
    display: 'flex', alignItems: 'center', gap: 10, padding: collapsed ? 10 : '8px 10px',
    borderRadius: 6, border: 'none', background: 'transparent',
    color: t.fg2, fontSize: 13, fontFamily: 'inherit', cursor: 'pointer', textAlign: 'left',
    justifyContent: collapsed ? 'center' : 'flex-start',
  };
}

export function Sidebar({ tab, setTab, collapsed, t, onSettings, onDoctor, runningCount, stacksCount }: {
  tab: Tab; setTab: (t: Tab) => void; collapsed: boolean; t: ThemeTokens;
  onSettings: () => void; onDoctor: () => void;
  runningCount: number; stacksCount: number;
}) {
  const w = collapsed ? 56 : 220;
  return (
    <div style={{ width: w, flexShrink: 0, background: t.surfaceAlt, borderRight: `1px solid ${t.border}`, display: 'flex', flexDirection: 'column', padding: collapsed ? '12px 8px' : '14px 12px', gap: 4, transition: 'width .2s' }}>
      {!collapsed && (
        <div style={{ padding: '4px 8px 12px', display: 'flex', alignItems: 'center', gap: 10 }}>
          <div style={{ width: 22, height: 22, borderRadius: 5, background: t.fg1, display: 'flex', alignItems: 'center', justifyContent: 'center', color: t.bg, fontFamily: t.mono, fontSize: 11, fontWeight: 700 }}>C</div>
          <div style={{ fontSize: 14, fontWeight: 600, color: t.fg1 }}>cgui</div>
          {/* App version pulled from package.json at build time via the
              Vite `define` (__APP_VERSION__). Matches the version on
              the macOS About panel and on GitHub Releases. */}
          <div style={{ marginLeft: 'auto', fontSize: 10, color: t.fg3, fontFamily: t.mono }}>{__APP_VERSION__}</div>
        </div>
      )}
      {!collapsed && <Eyebrow t={t}>Resources</Eyebrow>}
      {TABS.map(item => {
        const active = tab === item.key;
        const count = item.key === 'containers' ? runningCount : item.key === 'stacks' ? stacksCount : null;
        return (
          <button key={item.key} onClick={() => setTab(item.key)} title={collapsed ? item.label : ''}
            style={{
              display: 'flex', alignItems: 'center', gap: 10, padding: collapsed ? 10 : '8px 10px',
              borderRadius: 6, border: 'none',
              background: active ? t.selected : 'transparent',
              color: active ? t.fg1 : t.fg2, fontSize: 13, fontFamily: 'inherit', cursor: 'pointer', textAlign: 'left',
              borderLeft: active ? `2px solid ${t.accent}` : '2px solid transparent',
              fontWeight: active ? 500 : 400,
              justifyContent: collapsed ? 'center' : 'flex-start',
            }}>
            <Icon name={item.icon} size={16} color={active ? t.accent : t.fg2} />
            {!collapsed && <span>{item.label}</span>}
            {!collapsed && active && count !== null && (
              <span style={{ marginLeft: 'auto', fontSize: 10, fontFamily: t.mono, color: t.fg3 }}>{count}</span>
            )}
          </button>
        );
      })}
      <div style={{ flex: 1 }} />
      {!collapsed && <Eyebrow t={t}>System</Eyebrow>}
      <button onClick={onDoctor} style={sidebarBtn(t, collapsed)}>
        <Icon name="heart" size={16} color={t.fg2} />
        {!collapsed && <span>Doctor</span>}
      </button>
      <button onClick={onSettings} style={sidebarBtn(t, collapsed)}>
        <Icon name="cog" size={16} color={t.fg2} />
        {!collapsed && <span>Settings</span>}
      </button>
    </div>
  );
}

// ─── top toolbar ──────────────────────────────────────────────────────
export function TopBar({ tab, t, search, setSearch, onPull, onCollapse, runtime, dark, setDark, onUpdate, updateCount = 0, updatesSeen = false, headings }: {
  tab: Tab; t: ThemeTokens;
  search: string; setSearch: (s: string) => void;
  onPull: () => void; onCollapse: () => void;
  runtime: Runtime;
  dark: boolean; setDark: (v: boolean) => void;
  onUpdate: (() => void) | null;
  /** Live count from `listUpdates`. Drives the badge label. */
  updateCount?: number;
  /** True after the user has opened (and dismissed) the modal at
   *  least once this session. Mutes the badge colour from
   *  warning-orange to a subtle frame so it stops glaring. */
  updatesSeen?: boolean;
  headings: Record<Tab, { title: string; sub: string }>;
}) {
  const h = headings[tab];
  return (
    <div style={{ height: 64, padding: '0 20px', display: 'flex', alignItems: 'center', gap: 14, borderBottom: `1px solid ${t.border}`, background: t.surface, flexShrink: 0 }}>
      <button onClick={onCollapse} style={{ background: 'none', border: 'none', cursor: 'pointer', color: t.fg2, padding: 6, display: 'flex' }}><Icon name="menu" size={18} /></button>
      <div style={{ minWidth: 0, flex: '0 1 auto' }}>
        <div style={{ fontSize: 18, fontWeight: 600, color: t.fg1, lineHeight: 1.1 }}>{h.title}</div>
        <div style={{ fontSize: 12, color: t.fg3, fontFamily: t.mono, marginTop: 2 }}>{h.sub}</div>
      </div>
      <div style={{ flex: 1 }} />
      <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '0 12px', height: 32, background: t.surfaceAlt, border: `1px solid ${t.border}`, borderRadius: 8, width: 280 }}>
        <Icon name="search" size={14} color={t.fg3} />
        <input value={search} onChange={e => setSearch(e.target.value)} placeholder={`Filter ${tab}…`} style={{ background: 'transparent', border: 'none', outline: 'none', color: t.fg1, fontSize: 13, fontFamily: 'inherit', flex: 1, minWidth: 0 }} />
        <kbd style={{ fontSize: 10, color: t.fg3, fontFamily: t.mono, padding: '2px 5px', background: t.surface, border: `1px solid ${t.border}`, borderRadius: 3 }}>/</kbd>
      </div>
      <div style={{ display: 'flex', alignItems: 'center', gap: 6, padding: '4px 10px', height: 32, background: t.accentSoft, color: t.accent, border: `1px solid ${t.border}`, borderRadius: 999, fontSize: 11, fontFamily: t.mono }}>
        <span style={{ width: 6, height: 6, borderRadius: '50%', background: t.success }} />
        runtime: {runtime}
      </div>
      {onUpdate && updateCount > 0 && (
        // Two visual states: unseen (warning-orange, demands attention)
        // and seen (muted frame, stays clickable so the user can re-open
        // the modal). Label uses the real count, not a hardcoded literal.
        <button
          onClick={onUpdate}
          title={updatesSeen ? 'Companion updates (opened)' : 'Companion updates available'}
          style={{
            display: 'flex', alignItems: 'center', gap: 6,
            padding: '4px 10px', height: 32,
            background: 'transparent',
            border: `1px solid ${updatesSeen ? t.border : t.warning}`,
            color: updatesSeen ? t.fg3 : t.warning,
            borderRadius: 6, fontSize: 11, fontFamily: t.mono,
            cursor: 'pointer',
          }}
        >
          <Icon name="download" size={12} color={updatesSeen ? t.fg3 : t.warning} />
          {updateCount} update{updateCount === 1 ? '' : 's'}
        </button>
      )}
      <button onClick={() => setDark(!dark)} style={{ background: 'transparent', border: `1px solid ${t.border}`, borderRadius: 6, color: t.fg2, width: 32, height: 32, cursor: 'pointer', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
        <Icon name={dark ? 'sun' : 'moon'} size={14} />
      </button>
      <button onClick={onPull} style={{ display: 'flex', alignItems: 'center', gap: 6, padding: '0 14px', height: 32, background: t.fg1, color: t.bg, border: 'none', borderRadius: 6, fontSize: 13, fontFamily: 'inherit', fontWeight: 500, cursor: 'pointer' }}>
        <Icon name="plus" size={14} color={t.bg} />
        Pull image
      </button>
    </div>
  );
}

// ─── status bar ───────────────────────────────────────────────────────
// Pretty-print elapsed seconds since the last container poll. Buckets to
// "just now" inside a tick window so the label doesn't churn 1↔2 every
// frame. Returns "—" when we haven't received a tick yet.
export function fmtAgo(lastMs: number, nowMs: number = Date.now()): string {
  if (!lastMs) return '—';
  const s = Math.max(0, Math.floor((nowMs - lastMs) / 1000));
  if (s < 2) return 'just now';
  if (s < 60) return `${s}s ago`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m ago`;
  const h = Math.floor(m / 60);
  return `${h}h ago`;
}

export function StatusBar({ t, runtime, tab, lastTickAt }: {
  t: ThemeTokens; runtime: Runtime; tab: Tab;
  lastTickAt?: number;
}) {
  // Hints are scoped to whichever shortcuts are actually wired for the
  // active tab. The handler lives in App.tsx — keep this list in sync
  // with the branches there. `?` is global so it appears on every tab.
  const hints: { keys: string; label: string }[] = (() => {
    if (tab === 'containers') {
      return [
        { keys: '↑/↓', label: 'navigate' },
        { keys: '↵',   label: 'inspect' },
        { keys: 'L',   label: 'logs' },
      ];
    }
    if (tab === 'images') {
      return [{ keys: 'S', label: 'scan' }];
    }
    return [];
  })();
  return (
    <div style={{
      height: 26, borderTop: `1px solid ${t.border}`, background: t.surfaceAlt,
      display: 'flex', alignItems: 'center', padding: '0 14px', gap: 14,
      fontSize: 11, color: t.fg3, fontFamily: t.mono, flexShrink: 0,
    }}>
      <span><span style={{ color: t.success }}>●</span> connected</span>
      <span style={{ color: t.fg3 }}>·</span>
      <span>runtime <span style={{ color: t.fg2 }}>{runtime}</span></span>
      <span style={{ color: t.fg3 }}>·</span>
      <span>tab <span style={{ color: t.fg2 }}>{tab}</span></span>
      {lastTickAt !== undefined && (
        <>
          <span style={{ color: t.fg3 }}>·</span>
          <span title="time since last container poll">
            updated <span style={{ color: t.fg2 }}>{fmtAgo(lastTickAt)}</span>
          </span>
        </>
      )}
      <div style={{ flex: 1 }} />
      {hints.map(h => (
        <span key={h.keys} style={{ color: t.fg3 }}>
          <span style={{ color: t.fg2 }}>{h.keys}</span> {h.label}
        </span>
      ))}
      <span style={{ color: t.fg3 }}>
        <span style={{ color: t.fg2 }}>?</span> help
      </span>
    </div>
  );
}

// ─── Selection primitives (A5) ────────────────────────────────────────
//
// SelectCheckbox: a small native checkbox with an `indeterminate` prop
// (HTML doesn't accept it as an attribute, so we set it on the imperative
// handle after every render). Used by list views to drive bulk selection.

export function SelectCheckbox({ t, checked, indeterminate, onChange, onClick, title }: {
  t: ThemeTokens;
  checked: boolean;
  indeterminate?: boolean;
  onChange: () => void;
  onClick?: (e: MouseEvent<HTMLInputElement>) => void;
  title?: string;
}) {
  const ref = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (ref.current) ref.current.indeterminate = !!indeterminate && !checked;
  }, [indeterminate, checked]);
  return (
    <input
      ref={ref}
      type="checkbox"
      checked={checked}
      onChange={onChange}
      onClick={onClick}
      title={title}
      style={{
        cursor: 'pointer',
        accentColor: t.accent,
        margin: 0,
      }}
    />
  );
}

// BulkActionBar: floating bottom strip that appears whenever a list view
// has at least one selected row. Children are the action buttons; the
// bar itself owns the count + clear-selection control.

export function BulkActionBar({ t, count, onClear, children }: {
  t: ThemeTokens;
  count: number;
  onClear: () => void;
  children: ReactNode;
}) {
  if (count === 0) return null;
  return (
    <div style={{
      position: 'sticky', bottom: 0, zIndex: 10,
      display: 'flex', alignItems: 'center', gap: 10,
      padding: '10px 18px',
      background: t.surface, borderTop: `1px solid ${t.border}`,
      boxShadow: '0 -8px 24px rgba(0,0,0,0.18)',
      fontFamily: t.mono, fontSize: 12, color: t.fg2,
    }}>
      <span style={{
        padding: '3px 10px', borderRadius: 999,
        background: t.accent, color: t.accentInk, fontWeight: 600,
      }}>{count}</span>
      <span>selected</span>
      <div style={{ flex: 1 }} />
      {children}
      <button onClick={onClear} title="Clear selection"
        style={{
          marginLeft: 4,
          padding: '6px 10px',
          background: 'transparent',
          color: t.fg3,
          border: `1px solid ${t.border}`,
          borderRadius: 6,
          fontSize: 11, fontFamily: t.mono, cursor: 'pointer',
        }}
      >Clear</button>
    </div>
  );
}
