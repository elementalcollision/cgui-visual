import { useEffect, useState } from 'react';
import { api } from '../api';
import { Icon, iconBtn, pillBtn } from '../components';
import { errMessage } from '../loadState';
import type { ThemeTokens } from '../theme';
import type { ScanResponse, Severity, TrivyFinding, VulnHistory } from '../types';
import { Backdrop } from './shared';
import { CveDrawer } from './CveDrawer';
import { VulnTrendStrip } from './VulnTrendStrip';

// Synthesise a vendor-neutral lookup URL when trivy doesn't report any
// References. NVD has the broadest CVE coverage and supports a stable
// path scheme by id, so it makes a sane default link target.
export function nvdUrl(cve: string): string {
  return `https://nvd.nist.gov/vuln/detail/${encodeURIComponent(cve)}`;
}

export function TrivyModal({ t, image, onClose }: { t: ThemeTokens; image?: string; onClose: () => void }) {
  const [resp, setResp] = useState<ScanResponse | null>(null);
  // Bumped by "Retry" on a failed scan to re-run the effect below.
  const [attempt, setAttempt] = useState(0);
  const [filter, setFilter] = useState<Severity | null>(null);
  const [search, setSearch] = useState('');
  const [selected, setSelected] = useState<TrivyFinding | null>(null);
  // Vuln-history strip (B7). Loaded after the scan completes so the
  // newly-recorded scan is included in the trend.
  const [vulnHistory, setVulnHistory] = useState<VulnHistory | null>(null);
  useEffect(() => {
    let cancelled = false;
    setResp(null);
    setVulnHistory(null);
    api.scanImage(image || '').then(r => {
      if (cancelled) return;
      setResp(r);
      // Refresh trend after the backend has persisted this scan (only
      // completed scans are persisted).
      if (r.status === 'ok') {
        api.vulnHistory(r.result.image).then(h => { if (!cancelled) setVulnHistory(h); });
      }
    }).catch(e => {
      if (!cancelled) setResp({ status: 'failed', message: errMessage(e) });
    });
    return () => { cancelled = true; };
  }, [image, attempt]);
  if (!resp) return null;
  if (resp.status !== 'ok') {
    // Neither of these is a clean result: never render counts/findings.
    const notInstalled = resp.status === 'notInstalled';
    return (
      <Backdrop title={notInstalled ? 'Trivy is not installed' : 'Scan failed'} onClose={onClose}>
        <div role="alert" style={{ width: 560, background: t.surface, border: `1px solid ${t.border}`, borderRadius: 12, overflow: 'hidden' }}>
          <div style={{ padding: '16px 22px', borderBottom: `1px solid ${t.border}`, display: 'flex', alignItems: 'center', gap: 12 }}>
            <Icon name="shield" size={18} color={t.danger} />
            <div style={{ flex: 1 }}>
              <div style={{ fontSize: 15, fontWeight: 600, color: t.fg1 }}>{notInstalled ? 'Trivy is not installed' : 'Scan failed'}</div>
              <div style={{ fontSize: 12, color: t.fg3, fontFamily: t.mono }}>{image}</div>
            </div>
            <button onClick={onClose} style={iconBtn()} aria-label="Close"><Icon name="x" size={16} color={t.fg2} /></button>
          </div>
          <div style={{ padding: '18px 22px', fontSize: 13, color: t.fg2, lineHeight: 1.5 }}>
            {notInstalled ? (
              <>The image was <b>not</b> scanned, so there is no result. Install Trivy to enable scanning:
                <pre style={{ margin: '10px 0 0', fontFamily: t.mono, fontSize: 12, color: t.fg1 }}>brew install trivy</pre></>
            ) : (
              <>The image was <b>not</b> scanned, so this is not a clean result.
                <pre style={{ margin: '10px 0 0', whiteSpace: 'pre-wrap', fontFamily: t.mono, fontSize: 11, color: t.fg2, background: t.surfaceAlt, border: `1px solid ${t.border}`, borderRadius: 6, padding: '8px 10px' }}>{resp.message}</pre></>
            )}
          </div>
          <div style={{ padding: '12px 22px', borderTop: `1px solid ${t.border}`, display: 'flex', justifyContent: 'flex-end', gap: 8, background: t.surfaceAlt }}>
            <button style={pillBtn(t)} onClick={() => setAttempt(n => n + 1)}>{notInstalled ? 'Check again' : 'Retry'}</button>
            <button style={pillBtn(t)} onClick={onClose}>Close</button>
          </div>
        </div>
      </Backdrop>
    );
  }
  const data = resp.result;
  const sevColor: Record<Severity, string> = { CRITICAL: t.danger, HIGH: '#E5704A', MEDIUM: t.warning, LOW: t.fg3 };
  const findings = data.findings.filter(f =>
    (!filter || f.sev === filter) &&
    (!search || (f.cve + f.pkg + f.title).toLowerCase().includes(search.toLowerCase()))
  );
  return (
    <Backdrop title="Trivy scan" onClose={onClose}>
      <div style={{ width: 1000, maxHeight: '85vh', background: t.surface, border: `1px solid ${t.border}`, borderRadius: 12, overflow: 'hidden', display: 'flex', flexDirection: 'column' }}>
        <div style={{ padding: '16px 22px', borderBottom: `1px solid ${t.border}`, display: 'flex', alignItems: 'center', gap: 12 }}>
          <Icon name="shield" size={18} color={t.warning} />
          <div style={{ flex: 1 }}>
            <div style={{ fontSize: 15, fontWeight: 600, color: t.fg1 }}>Trivy scan</div>
            <div style={{ fontSize: 12, color: t.fg3, fontFamily: t.mono }}>{data.image}</div>
          </div>
          <button onClick={onClose} style={iconBtn()} aria-label="Close"><Icon name="x" size={16} color={t.fg2} /></button>
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
        {vulnHistory && vulnHistory.points.length > 0 && (
          <VulnTrendStrip t={t} sevColor={sevColor} history={vulnHistory} />
        )}
        {/* Findings table + slide-in drawer. The drawer is a simple flex
            sibling rather than a separate overlay so it cooperates with the
            modal's max-height + scrollbars. */}
        <div style={{ flex: 1, display: 'flex', minHeight: 0 }}>
          <div style={{ flex: 1, overflow: 'auto', borderRight: selected ? `1px solid ${t.border}` : 'none' }}>
            {data.findings.length === 0 && (
              <div role="status" style={{ padding: '40px 22px', textAlign: 'center', color: t.success, fontSize: 13 }}>
                Scan completed: no vulnerabilities found.
              </div>
            )}
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
