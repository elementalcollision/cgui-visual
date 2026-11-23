import { useEffect, useState } from 'react';
import { Icon, iconBtn, pillBtn } from '../components';
import type { ThemeTokens } from '../theme';
import { Backdrop } from './shared';

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
    <Backdrop title={title} onClose={onClose}>
      <div style={{ width: 720, maxHeight: '85vh', background: t.surface, border: `1px solid ${t.border}`, borderRadius: 12, overflow: 'hidden', display: 'flex', flexDirection: 'column', boxShadow: '0 24px 80px rgba(0,0,0,0.4)' }}>
        <div style={{ padding: '18px 22px', borderBottom: `1px solid ${t.border}`, display: 'flex', alignItems: 'flex-start', gap: 12 }}>
          <Icon name="info" size={20} color={t.accent} />
          <div style={{ flex: 1 }}>
            <div style={{ fontSize: 18, fontWeight: 600, color: t.fg1 }}>{title}</div>
            <div style={{ fontSize: 12, color: t.fg3, fontFamily: t.mono, marginTop: 4 }}>{subtitle}</div>
          </div>
          <button onClick={onClose} style={iconBtn()} aria-label="Close"><Icon name="x" size={16} color={t.fg2} /></button>
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
