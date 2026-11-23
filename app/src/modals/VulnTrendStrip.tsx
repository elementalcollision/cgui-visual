import type { ThemeTokens } from '../theme';
import type { Severity, VulnHistory } from '../types';

// Stacked-bar trend strip for one image's recent trivy scans (B7).
// Renders newest-on-the-right so the eye reads left-to-right as time.
// Each bar's height is normalised to the worst total in the window so
// growth is visually obvious. Hover tooltip shows per-severity counts.
export function VulnTrendStrip({ t, sevColor, history }: {
  t: ThemeTokens;
  sevColor: Record<Severity, string>;
  history: VulnHistory;
}) {
  // Backend returns most-recent-first; flip for chronological display.
  const points = [...history.points].reverse();
  const maxTotal = points.reduce((m, p) => Math.max(m, p.total), 1);
  const barW = Math.max(8, Math.min(20, Math.floor(420 / Math.max(points.length, 1))));
  const stripH = 60;
  const fmtTs = (s: number) => new Date(s * 1000).toLocaleString();
  return (
    <div style={{
      padding: '12px 22px', borderBottom: `1px solid ${t.border}`,
      background: t.surfaceAlt, display: 'flex', alignItems: 'center', gap: 16,
    }}>
      <div style={{ minWidth: 0 }}>
        <div style={{ fontSize: 10, color: t.fg3, fontWeight: 600, letterSpacing: '0.08em', textTransform: 'uppercase' }}>
          Trend
        </div>
        <div style={{ fontFamily: t.mono, fontSize: 11, color: t.fg2, marginTop: 2 }}>
          {points.length} scan{points.length === 1 ? '' : 's'}
        </div>
        {history.newSinceLast.length > 0 && (
          <div style={{
            marginTop: 6, padding: '4px 8px', display: 'inline-block',
            background: `${t.danger}22`, border: `1px solid ${t.danger}`, borderRadius: 4,
            fontFamily: t.mono, fontSize: 10, color: t.danger,
          }}
          title={history.newSinceLast.join(', ')}
          >
            +{history.newSinceLast.length} new since last
          </div>
        )}
      </div>
      <div style={{ flex: 1, display: 'flex', gap: 2, alignItems: 'flex-end', height: stripH }}>
        {points.map((p, i) => {
          // Stack severities tallest-impact on top so the colour-mass
          // matches the visual weight a viewer expects.
          const total = Math.max(1, p.total);
          const h = (p.total / maxTotal) * stripH;
          const segs: { sev: Severity; n: number }[] = [
            { sev: 'CRITICAL', n: p.critical },
            { sev: 'HIGH',     n: p.high },
            { sev: 'MEDIUM',   n: p.medium },
            { sev: 'LOW',      n: p.low },
          ];
          return (
            <div
              key={i}
              title={`${fmtTs(p.scannedAt)}\nC ${p.critical} · H ${p.high} · M ${p.medium} · L ${p.low}\ntotal ${p.total}`}
              style={{
                width: barW, height: h, display: 'flex', flexDirection: 'column',
                background: t.bg, border: `1px solid ${t.border}`,
                borderRadius: 2, overflow: 'hidden',
              }}
            >
              {segs.map(s => s.n > 0 && (
                <span key={s.sev} style={{
                  flex: s.n / total,
                  background: sevColor[s.sev],
                }} />
              ))}
            </div>
          );
        })}
      </div>
    </div>
  );
}
