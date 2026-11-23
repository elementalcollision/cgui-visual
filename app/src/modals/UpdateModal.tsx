import { useEffect, useState } from 'react';
import { api } from '../api';
import { Icon, iconBtn, pillBtn } from '../components';
import { errMessage } from '../loadState';
import type { ThemeTokens } from '../theme';
import type { Update } from '../types';
import { Backdrop } from './shared';

export function UpdateModal({ t, onClose }: { t: ThemeTokens; onClose: () => void }) {
  const [items, setItems] = useState<Update[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [idx, setIdx] = useState(0);
  useEffect(() => {
    api.listUpdates().then(setItems).catch(e => setError(errMessage(e)));
  }, []);
  if (error) {
    return (
      <Backdrop title="Update check failed" onClose={onClose}>
        <div role="alert" style={{ width: 520, background: t.surface, border: `1px solid ${t.border}`, borderRadius: 12, padding: '18px 22px' }}>
          <div style={{ fontSize: 15, fontWeight: 600, color: t.fg1 }}>Couldn't check for updates</div>
          <div style={{ marginTop: 8, fontSize: 12, color: t.fg2, fontFamily: t.mono }}>{error}</div>
          <div style={{ marginTop: 14, textAlign: 'right' }}><button style={pillBtn(t)} onClick={onClose}>Close</button></div>
        </div>
      </Backdrop>
    );
  }
  if (!items.length) return null;
  const u = items[idx];

  // Open the companion's GitHub release page through the opener plugin
  // when available, falling back to window.open in browser-dev mode.
  const openRelease = async () => {
    if (!u.url) return;
    try {
      const { openUrl } = await import('@tauri-apps/plugin-opener');
      await openUrl(u.url);
    } catch {
      window.open(u.url, '_blank', 'noopener,noreferrer');
    }
  };

  return (
    <Backdrop title="Update available" onClose={onClose}>
      <div style={{ width: 640, maxHeight: '85vh', background: t.surface, border: `1px solid ${t.border}`, borderRadius: 12, overflow: 'hidden', display: 'flex', flexDirection: 'column' }}>
        <div style={{ padding: '16px 22px', borderBottom: `1px solid ${t.border}`, display: 'flex', alignItems: 'center', gap: 12 }}>
          <Icon name="download" size={18} color={t.warning} />
          <div style={{ flex: 1, minWidth: 0 }}>
            {/* Header now leads with the companion name so users don't
                read this as a cgui-visual self-update prompt. */}
            <div style={{ fontSize: 15, fontWeight: 600, color: t.fg1 }}>
              {u.component} update available
            </div>
            <div style={{ fontSize: 12, color: t.fg3, fontFamily: t.mono, marginTop: 2 }}>
              {u.installed} → {u.latest} · {u.published}
            </div>
          </div>
          <div style={{ display: 'flex', gap: 4 }}>
            {items.map((_, i) => (
              <button key={i} onClick={() => setIdx(i)} aria-label={`Update ${i + 1} of ${items.length}`} aria-current={i === idx ? 'true' : undefined} style={{ width: 8, height: 8, borderRadius: '50%', background: i === idx ? t.accent : t.border, border: 'none', cursor: 'pointer' }} />
            ))}
          </div>
          <button onClick={onClose} style={iconBtn()} aria-label="Close"><Icon name="x" size={16} color={t.fg2} /></button>
        </div>
        <div style={{ flex: 1, overflow: 'auto', padding: '16px 22px', maxHeight: 360 }}>
          <pre style={{ margin: 0, fontFamily: t.mono, fontSize: 12, lineHeight: 1.6, color: t.fg2, whiteSpace: 'pre-wrap' }}>{u.notes}</pre>
        </div>
        <div style={{ padding: '12px 22px', borderTop: `1px solid ${t.border}`, display: 'flex', alignItems: 'center', gap: 8, background: t.surfaceAlt }}>
          {/* Disambiguation footnote: cgui-visual self-updates via a
              different surface (the in-app updater banner). */}
          <span style={{ fontSize: 11, color: t.fg3, fontFamily: t.mono }}>
            cgui-visual updates separately via the in-app updater.
          </span>
          <div style={{ flex: 1 }} />
          <button style={{ ...pillBtn(t), padding: '6px 14px' }} onClick={onClose}>Dismiss</button>
          <button
            onClick={openRelease}
            disabled={!u.url}
            style={{
              padding: '6px 14px',
              background: t.fg1, color: t.bg, border: 'none', borderRadius: 6,
              fontSize: 12, fontWeight: 500,
              cursor: u.url ? 'pointer' : 'default',
              opacity: u.url ? 1 : 0.5,
            }}
          >View release ↗</button>
        </div>
      </div>
    </Backdrop>
  );
}
