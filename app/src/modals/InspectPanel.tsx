import { Fragment } from 'react';
import type { ThemeTokens } from '../theme';
import type { Container } from '../types';
import { Muted } from './shared';
import { InspectTab, ParsedInspect } from './inspect';
import { TrendsPanel } from './TrendsPanel';

// Compact tab-strip for the inspect modal. Disabled tabs (no data for
// that projection) render dim and stay un-clickable.
export function InspectTabs({ t, tab, setTab, parsed }: {
  t: ThemeTokens; tab: InspectTab; setTab: (v: InspectTab) => void; parsed: ParsedInspect | null;
}) {
  const tabs: { id: InspectTab; label: string; count: number; available: boolean }[] = [
    { id: 'trends',  label: 'Trends',  count: 0,                            available: true },
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

export function InspectPanel({ t, tab, parsed, json, container }: {
  t: ThemeTokens; tab: InspectTab; parsed: ParsedInspect | null; json: string;
  container: Container;
}) {
  if (tab === 'trends') {
    return <TrendsPanel t={t} container={container} />;
  }
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
          if (n.aliases?.length) out.push({ k: `${n.name} (aliases)`, v: n.aliases.join(', ') });
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
