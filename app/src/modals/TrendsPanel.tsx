import { useEffect, useState } from 'react';
import { api } from '../api';
import { Sparkline } from '../components';
import type { ThemeTokens } from '../theme';
import type { Container, HistoryPoint } from '../types';
import { Muted } from './shared';

// Long-form metric trends (B6). Loads a configurable time window from
// the SQLite sidecar and plots CPU / Memory / Net / Disk as sparklines.
// Re-fetches whenever the user picks a different window or when the
// modal is re-opened.
export function TrendsPanel({ t, container }: { t: ThemeTokens; container: Container }) {
  type Window = { label: string; secs: number };
  const windows: Window[] = [
    { label: '1h',  secs: 3600 },
    { label: '6h',  secs: 6 * 3600 },
    { label: '24h', secs: 24 * 3600 },
    { label: '7d',  secs: 7 * 24 * 3600 },
  ];
  const [windowSecs, setWindowSecs] = useState(windows[2].secs);
  const [points, setPoints] = useState<HistoryPoint[] | null>(null);
  useEffect(() => {
    let cancelled = false;
    setPoints(null);
    api.containerHistory(container.id, windowSecs).then(p => {
      if (!cancelled) setPoints(p);
    });
    return () => { cancelled = true; };
  }, [container.id, windowSecs]);

  if (points === null) {
    return <Muted t={t}>Loading history…</Muted>;
  }
  if (points.length < 2) {
    return (
      <div>
        <WindowChips t={t} windows={windows} active={windowSecs} setActive={setWindowSecs} />
        <Muted t={t}>
          No history yet for this container. Samples accumulate every 2 s while cgui is running; come back in a few minutes.
        </Muted>
      </div>
    );
  }
  const cpu = points.map(p => p.cpu);
  const mem = points.map(p => p.memUsed);
  const net = points.map(p => p.netBps / 1024 / 1024);   // MB/s
  const disk = points.map(p => p.diskBps / 1024 / 1024); // MB/s
  const last = points[points.length - 1];
  const first = points[0];
  const fmtTs = (s: number) => new Date(s * 1000).toLocaleTimeString();
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
        <WindowChips t={t} windows={windows} active={windowSecs} setActive={setWindowSecs} />
        <span style={{ flex: 1 }} />
        <span style={{ fontFamily: t.mono, fontSize: 11, color: t.fg3 }}>
          {points.length} samples · {fmtTs(first.ts)} → {fmtTs(last.ts)}
        </span>
      </div>
      <TrendCard t={t} label="CPU"     unit="%"    series={cpu}  current={`${last.cpu.toFixed(1)}%`} />
      <TrendCard t={t} label="Memory"  unit="GiB"  series={mem}  current={`${last.memUsed.toFixed(2)} GiB`} />
      <TrendCard t={t} label="Network" unit="MB/s" series={net}  current={`${(last.netBps / 1024 / 1024).toFixed(2)} MB/s`} />
      <TrendCard t={t} label="Disk"    unit="MB/s" series={disk} current={`${(last.diskBps / 1024 / 1024).toFixed(2)} MB/s`} />
    </div>
  );
}

function WindowChips({ t, windows, active, setActive }: {
  t: ThemeTokens; windows: { label: string; secs: number }[];
  active: number; setActive: (s: number) => void;
}) {
  return (
    <div style={{ display: 'flex', gap: 4 }}>
      {windows.map(w => {
        const sel = w.secs === active;
        return (
          <button
            key={w.secs}
            onClick={() => setActive(w.secs)}
            style={{
              padding: '4px 10px',
              fontSize: 11, fontFamily: t.mono, fontWeight: 600,
              color: sel ? t.fg1 : t.fg3,
              background: sel ? t.selected : t.surfaceAlt,
              border: `1px solid ${sel ? t.accent : t.border}`,
              borderRadius: 4, cursor: 'pointer',
            }}
          >{w.label}</button>
        );
      })}
    </div>
  );
}

function TrendCard({ t, label, unit, series, current }: {
  t: ThemeTokens; label: string; unit: string; series: number[]; current: string;
}) {
  const max = Math.max(0.0001, ...series);
  return (
    <div style={{
      padding: '12px 14px',
      background: t.surfaceAlt, border: `1px solid ${t.border}`, borderRadius: 8,
      display: 'grid', gridTemplateColumns: '120px 1fr', gap: 14, alignItems: 'center',
    }}>
      <div>
        <div style={{ fontSize: 10, color: t.fg3, fontWeight: 600, letterSpacing: '0.08em', textTransform: 'uppercase' }}>{label}</div>
        <div style={{ fontFamily: t.mono, fontSize: 18, fontWeight: 600, color: t.fg1, fontVariantNumeric: 'tabular-nums', marginTop: 2 }}>{current}</div>
        <div style={{ fontSize: 10, color: t.fg3, fontFamily: t.mono, marginTop: 2 }}>peak {max.toFixed(2)} {unit}</div>
      </div>
      <Sparkline data={series} w={420} h={50} color={t.sparkline} max={max * 1.1} />
    </div>
  );
}
