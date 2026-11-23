import { Icon, iconBtn } from '../components';
import type { ThemeTokens } from '../theme';
import type { TrivyFinding } from '../types';
import { nvdUrl } from './TrivyModal';

export function CveDrawer({ t, f, sevColor, onClose }: {
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
        <button onClick={onClose} style={iconBtn()} aria-label="Close"><Icon name="x" size={14} color={t.fg2} /></button>
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
