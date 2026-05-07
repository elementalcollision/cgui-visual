/* cgui prototype — main app.
   Three variations (Workbench / Editorial / Terminal), all surfaces, all interactive. */

const { useState, useEffect, useMemo, useRef } = React;
const D = window.SAMPLE;
const VARS = window.VARIATIONS;

// ─── Utilities ─────────────────────────────────────────────────────────
function fmtBytes(gib, unit = 'GiB') {
  if (gib < 0.01) return `${(gib * 1024).toFixed(1)} MiB`;
  if (gib < 1) return `${(gib * 1024).toFixed(0)} MiB`;
  return `${gib.toFixed(gib >= 100 ? 0 : 1)} ${unit}`;
}
function StatusDot({ status, t }) {
  const c = status === 'running' ? t.success : status === 'paused' ? t.warning : status === 'exited' || status === 'stopped' ? t.fg3 : t.fg3;
  return <span style={{ display: 'inline-block', width: 8, height: 8, borderRadius: '50%', background: c, marginRight: 8, verticalAlign: 'middle', flexShrink: 0 }} />;
}
function Sparkline({ data, w = 64, h = 18, color, max = 100 }) {
  const pts = data.map((v, i) => `${(i / (data.length - 1)) * w},${h - (v / max) * h}`).join(' ');
  return (
    <svg width={w} height={h} style={{ display: 'block' }}>
      <polyline points={pts} fill="none" stroke={color} strokeWidth="1.25" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}
function Bar({ pct, color, bg, h = 6, w = '100%' }) {
  return (
    <div style={{ width: w, height: h, background: bg, borderRadius: 999, overflow: 'hidden' }}>
      <div style={{ width: `${Math.min(100, pct)}%`, height: '100%', background: color, transition: 'width .3s' }} />
    </div>
  );
}
function Icon({ name, size = 14, color = 'currentColor', strokeWidth = 1.6 }) {
  // Tiny inline icon set — line, geometric, no fills.
  const s = { width: size, height: size, stroke: color, fill: 'none', strokeWidth, strokeLinecap: 'round', strokeLinejoin: 'round', display: 'block' };
  switch (name) {
    case 'box': return <svg viewBox="0 0 24 24" style={s}><path d="M3 7l9-4 9 4v10l-9 4-9-4z"/><path d="M3 7l9 4 9-4M12 11v10"/></svg>;
    case 'image': return <svg viewBox="0 0 24 24" style={s}><rect x="3" y="3" width="18" height="18" rx="2"/><circle cx="9" cy="9" r="2"/><path d="M21 15l-5-5L5 21"/></svg>;
    case 'database': return <svg viewBox="0 0 24 24" style={s}><ellipse cx="12" cy="5" rx="8" ry="3"/><path d="M4 5v6c0 1.7 3.6 3 8 3s8-1.3 8-3V5M4 11v6c0 1.7 3.6 3 8 3s8-1.3 8-3v-6"/></svg>;
    case 'network': return <svg viewBox="0 0 24 24" style={s}><circle cx="12" cy="6" r="2.5"/><circle cx="5" cy="18" r="2.5"/><circle cx="19" cy="18" r="2.5"/><path d="M12 8.5v3M10 13l-4 3M14 13l4 3"/></svg>;
    case 'layers': return <svg viewBox="0 0 24 24" style={s}><path d="M12 2l10 5-10 5L2 7l10-5z"/><path d="M2 12l10 5 10-5M2 17l10 5 10-5"/></svg>;
    case 'terminal': return <svg viewBox="0 0 24 24" style={s}><path d="M4 17l5-5-5-5M11 19h8"/></svg>;
    case 'play': return <svg viewBox="0 0 24 24" style={{ ...s, fill: color, stroke: 'none' }}><path d="M6 4l14 8-14 8z"/></svg>;
    case 'stop': return <svg viewBox="0 0 24 24" style={{ ...s, fill: color, stroke: 'none' }}><rect x="6" y="6" width="12" height="12" rx="1"/></svg>;
    case 'restart': return <svg viewBox="0 0 24 24" style={s}><path d="M3 12a9 9 0 1 0 3-6.7M3 4v5h5"/></svg>;
    case 'trash': return <svg viewBox="0 0 24 24" style={s}><path d="M3 6h18M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2M5 6l1 14a2 2 0 0 0 2 2h8a2 2 0 0 0 2-2l1-14"/></svg>;
    case 'info': return <svg viewBox="0 0 24 24" style={s}><circle cx="12" cy="12" r="9"/><path d="M12 16v-5M12 8h.01"/></svg>;
    case 'logs': return <svg viewBox="0 0 24 24" style={s}><path d="M4 4h16v16H4z"/><path d="M8 9h8M8 13h8M8 17h5"/></svg>;
    case 'search': return <svg viewBox="0 0 24 24" style={s}><circle cx="11" cy="11" r="7"/><path d="M21 21l-4.5-4.5"/></svg>;
    case 'pause': return <svg viewBox="0 0 24 24" style={{ ...s, fill: color, stroke: 'none' }}><rect x="6" y="4" width="4" height="16" rx="1"/><rect x="14" y="4" width="4" height="16" rx="1"/></svg>;
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

// ─── Window chromes ───────────────────────────────────────────────────
function MacChrome({ children, t }) {
  return (
    <div style={{ width: '100%', height: '100%', borderRadius: 12, overflow: 'hidden', background: t.surface, boxShadow: '0 24px 60px rgba(0,0,0,0.35), 0 0 0 0.5px rgba(0,0,0,0.4)', display: 'flex', flexDirection: 'column' }}>
      <div style={{ height: 38, display: 'flex', alignItems: 'center', padding: '0 14px', background: t.headerBar, borderBottom: `1px solid ${t.border}`, flexShrink: 0, gap: 12 }}>
        <div style={{ display: 'flex', gap: 8 }}>
          <div style={{ width: 12, height: 12, borderRadius: '50%', background: '#FF5F57' }} />
          <div style={{ width: 12, height: 12, borderRadius: '50%', background: '#FEBC2E' }} />
          <div style={{ width: 12, height: 12, borderRadius: '50%', background: '#28C840' }} />
        </div>
        <div style={{ flex: 1, textAlign: 'center', fontSize: 13, fontWeight: 500, color: t.fg2 }}>cgui</div>
        <div style={{ width: 52 }} />
      </div>
      <div style={{ flex: 1, overflow: 'hidden', display: 'flex', flexDirection: 'column' }}>{children}</div>
    </div>
  );
}
function FramelessChrome({ children, t }) {
  return (
    <div style={{ width: '100%', height: '100%', borderRadius: 12, overflow: 'hidden', background: t.surface, boxShadow: '0 24px 60px rgba(0,0,0,0.30), 0 0 0 0.5px rgba(0,0,0,0.4)', display: 'flex', flexDirection: 'column', border: `1px solid ${t.border}` }}>
      {children}
    </div>
  );
}

// ─── Sidebar ──────────────────────────────────────────────────────────
const TABS = [
  { key: 'containers', label: 'Containers', icon: 'box' },
  { key: 'images', label: 'Images', icon: 'image' },
  { key: 'volumes', label: 'Volumes', icon: 'database' },
  { key: 'networks', label: 'Networks', icon: 'network' },
  { key: 'stacks', label: 'Stacks', icon: 'layers' },
  { key: 'logs', label: 'Logs', icon: 'logs' },
];

function Sidebar({ tab, setTab, collapsed, t, variation, onSettings, onDoctor }) {
  const w = collapsed ? 56 : 220;
  const isTerm = variation === 'terminal';
  const isEdit = variation === 'editorial';
  return (
    <div style={{ width: w, flexShrink: 0, background: isTerm ? t.bg : t.surfaceAlt, borderRight: `1px solid ${t.border}`, display: 'flex', flexDirection: 'column', padding: collapsed ? '12px 8px' : '14px 12px', gap: 4, transition: 'width .2s' }}>
      {!collapsed && (
        <div style={{ padding: '4px 8px 12px', display: 'flex', alignItems: 'center', gap: 10 }}>
          <div style={{ width: 22, height: 22, borderRadius: isTerm ? 0 : 5, background: t.fg1, display: 'flex', alignItems: 'center', justifyContent: 'center', color: t.bg, fontFamily: t.mono, fontSize: 11, fontWeight: 700 }}>
            {isTerm ? '$_' : 'C'}
          </div>
          <div style={{ fontSize: isEdit ? 18 : 14, fontWeight: isEdit ? 400 : 600, fontFamily: isEdit ? 'var(--font-serif)' : 'inherit', fontStyle: isEdit ? 'italic' : 'normal', color: t.fg1, letterSpacing: isTerm ? '0.05em' : 0 }}>
            {isTerm ? 'cgui' : 'cgui'}
          </div>
          <div style={{ marginLeft: 'auto', fontSize: 10, color: t.fg3, fontFamily: t.mono }}>0.13.0</div>
        </div>
      )}
      {!collapsed && <Eyebrow t={t}>Resources</Eyebrow>}
      {TABS.map(item => {
        const active = tab === item.key;
        return (
          <button key={item.key} onClick={() => setTab(item.key)} title={collapsed ? item.label : ''}
            style={{
              display: 'flex', alignItems: 'center', gap: 10, padding: collapsed ? 10 : '8px 10px', borderRadius: isTerm ? 0 : 6, border: 'none', background: active ? (isTerm ? t.accentSoft : t.selected) : 'transparent',
              color: active ? t.fg1 : t.fg2, fontSize: 13, fontFamily: 'inherit', cursor: 'pointer', textAlign: 'left',
              borderLeft: active && !isTerm ? `2px solid ${t.accent}` : '2px solid transparent',
              fontWeight: active ? 500 : 400,
              justifyContent: collapsed ? 'center' : 'flex-start',
            }}>
            <Icon name={item.icon} size={16} color={active ? t.accent : t.fg2} />
            {!collapsed && <span>{item.label}</span>}
            {!collapsed && active && <span style={{ marginLeft: 'auto', fontSize: 10, fontFamily: t.mono, color: t.fg3 }}>{item.key === 'containers' ? D.containers.filter(c => c.status === 'running').length : item.key === 'stacks' ? D.stacks.length : ''}</span>}
          </button>
        );
      })}
      <div style={{ flex: 1 }} />
      {!collapsed && <Eyebrow t={t}>System</Eyebrow>}
      <button onClick={onDoctor} style={sidebarBtn(t, false, isTerm, collapsed)}>
        <Icon name="heart" size={16} color={t.fg2} />
        {!collapsed && <span>Doctor</span>}
      </button>
      <button onClick={onSettings} style={sidebarBtn(t, false, isTerm, collapsed)}>
        <Icon name="cog" size={16} color={t.fg2} />
        {!collapsed && <span>Settings</span>}
      </button>
    </div>
  );
}
function sidebarBtn(t, active, isTerm, collapsed) {
  return {
    display: 'flex', alignItems: 'center', gap: 10, padding: collapsed ? 10 : '8px 10px',
    borderRadius: isTerm ? 0 : 6, border: 'none', background: active ? t.selected : 'transparent',
    color: t.fg2, fontSize: 13, fontFamily: 'inherit', cursor: 'pointer', textAlign: 'left',
    justifyContent: collapsed ? 'center' : 'flex-start',
  };
}
function Eyebrow({ children, t }) {
  return <div style={{ fontSize: 10, fontWeight: 600, letterSpacing: '0.12em', textTransform: 'uppercase', color: t.fg3, padding: '12px 10px 6px' }}>{children}</div>;
}

// ─── Top toolbar ──────────────────────────────────────────────────────
function TopBar({ tab, t, variation, search, setSearch, onPull, onCollapse, collapsed, runtime, dark, setDark, onUpdate }) {
  const isTerm = variation === 'terminal';
  const isEdit = variation === 'editorial';
  const headings = {
    containers: { title: 'Containers', sub: `${D.containers.filter(c => c.status === 'running').length} running · ${D.containers.length} total` },
    images: { title: 'Images', sub: `${D.images.length} images · ${D.images.reduce((s, i) => s + i.size, 0).toFixed(1)} GiB on disk` },
    volumes: { title: 'Volumes', sub: `${D.volumes.length} volumes · ${D.volumes.reduce((s, v) => s + v.used, 0).toFixed(0)} GiB used` },
    networks: { title: 'Networks', sub: `${D.networks.length} networks` },
    stacks: { title: 'Stacks', sub: `${D.stacks.length} stacks · ${D.stacks.reduce((s, st) => s + st.services.filter(sv => sv.state === 'running').length, 0)} services running` },
    logs: { title: 'Logs', sub: 'mlperf-inference-llama2 · live' },
  };
  const h = headings[tab];
  return (
    <div style={{ height: isEdit ? 88 : 64, padding: '0 20px', display: 'flex', alignItems: 'center', gap: 14, borderBottom: `1px solid ${t.border}`, background: t.surface, flexShrink: 0 }}>
      <button onClick={onCollapse} style={{ background: 'none', border: 'none', cursor: 'pointer', color: t.fg2, padding: 6, display: 'flex' }}><Icon name="menu" size={18} /></button>
      <div style={{ minWidth: 0, flex: '0 1 auto' }}>
        <div style={{ fontSize: isEdit ? 28 : 18, fontWeight: isEdit ? 400 : 600, fontFamily: isEdit ? 'var(--font-serif)' : 'inherit', color: t.fg1, lineHeight: 1.1, letterSpacing: isEdit ? '-0.01em' : 0 }}>{h.title}</div>
        <div style={{ fontSize: 12, color: t.fg3, fontFamily: t.mono, marginTop: 2 }}>{h.sub}</div>
      </div>
      <div style={{ flex: 1 }} />
      <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '0 12px', height: 32, background: t.surfaceAlt, border: `1px solid ${t.border}`, borderRadius: isTerm ? 0 : 8, width: 280 }}>
        <Icon name="search" size={14} color={t.fg3} />
        <input value={search} onChange={e => setSearch(e.target.value)} placeholder={`Filter ${tab}…`} style={{ background: 'transparent', border: 'none', outline: 'none', color: t.fg1, fontSize: 13, fontFamily: 'inherit', flex: 1, minWidth: 0 }} />
        <kbd style={{ fontSize: 10, color: t.fg3, fontFamily: t.mono, padding: '2px 5px', background: t.surface, border: `1px solid ${t.border}`, borderRadius: 3 }}>/</kbd>
      </div>
      <div style={{ display: 'flex', alignItems: 'center', gap: 6, padding: '4px 10px', height: 32, background: t.accentSoft, color: t.accent, border: `1px solid ${t.border}`, borderRadius: isTerm ? 0 : 999, fontSize: 11, fontFamily: t.mono }}>
        <span style={{ width: 6, height: 6, borderRadius: '50%', background: t.success }} />
        runtime: {runtime}
      </div>
      {onUpdate && (
        <button onClick={onUpdate} title="Updates available" style={{ display: 'flex', alignItems: 'center', gap: 6, padding: '4px 10px', height: 32, background: 'transparent', border: `1px solid ${t.warning}`, color: t.warning, borderRadius: isTerm ? 0 : 6, fontSize: 11, fontFamily: t.mono, cursor: 'pointer' }}>
          <Icon name="download" size={12} color={t.warning} />
          2 updates
        </button>
      )}
      <button onClick={() => setDark(!dark)} style={{ background: 'transparent', border: `1px solid ${t.border}`, borderRadius: isTerm ? 0 : 6, color: t.fg2, width: 32, height: 32, cursor: 'pointer', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
        <Icon name={dark ? 'sun' : 'moon'} size={14} />
      </button>
      <button onClick={onPull} style={{ display: 'flex', alignItems: 'center', gap: 6, padding: '0 14px', height: 32, background: t.fg1, color: t.bg, border: 'none', borderRadius: isTerm ? 0 : 6, fontSize: 13, fontFamily: 'inherit', fontWeight: 500, cursor: 'pointer' }}>
        <Icon name="plus" size={14} color={t.bg} />
        Pull image
      </button>
    </div>
  );
}

window.cguiBase = { fmtBytes, StatusDot, Sparkline, Bar, Icon, MacChrome, FramelessChrome, Sidebar, TopBar, Eyebrow, TABS };
