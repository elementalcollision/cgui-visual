import { useState } from 'react';
import { api } from '../api';
import { Icon, pillBtn } from '../components';
import type { ThemeTokens } from '../theme';
import { Backdrop } from './shared';

// Shown on first launch when Apple's `container` CLI isn't on PATH.
// Primary CTA points users at Apple's official GitHub release page (their
// recommended install path); secondary path is the community-maintained
// Homebrew formula. The "Re-check" button avoids needing a relaunch
// after install — the polling task picks up the CLI on its next tick
// and the modal auto-closes via the `onAvailable` callback.
export function OnboardingModal({ t, onAvailable, onDismiss }: {
  t: ThemeTokens;
  onAvailable: () => void;
  onDismiss: () => void;
}) {
  const [checking, setChecking] = useState(false);
  const [copied, setCopied] = useState(false);

  const recheck = async () => {
    setChecking(true);
    try {
      const ok = await api.runtimeAvailable();
      if (ok) onAvailable();
    } finally {
      setChecking(false);
    }
  };

  const openApple = async () => {
    const url = 'https://github.com/apple/container/releases';
    if ('__TAURI_INTERNALS__' in window) {
      // tauri-plugin-opener: open URL in the system default browser.
      try {
        const { openUrl } = await import('@tauri-apps/plugin-opener');
        await openUrl(url);
        return;
      } catch (e) { console.warn('opener failed, falling back', e); }
    }
    window.open(url, '_blank', 'noopener,noreferrer');
  };

  const copyBrew = async () => {
    await navigator.clipboard?.writeText('brew install container');
    setCopied(true);
    setTimeout(() => setCopied(false), 2500);
  };

  return (
    <Backdrop title="Container CLI not detected" onClose={onDismiss}>
      <div style={{ width: 560, background: t.surface, border: `1px solid ${t.border}`, borderRadius: 12, overflow: 'hidden', display: 'flex', flexDirection: 'column', boxShadow: '0 24px 80px rgba(0,0,0,0.4)' }}>
        <div style={{ padding: '20px 22px', borderBottom: `1px solid ${t.border}`, display: 'flex', alignItems: 'flex-start', gap: 12 }}>
          <Icon name="info" size={22} color={t.warning} />
          <div style={{ flex: 1 }}>
            <div style={{ fontSize: 17, fontWeight: 600, color: t.fg1 }}>Apple <code style={{ background: t.surfaceAlt, padding: '0 4px', borderRadius: 3, fontFamily: t.mono }}>container</code> CLI not detected</div>
            <div style={{ fontSize: 12, color: t.fg3, marginTop: 4 }}>cgui-visual is showing sample data until the runtime is installed.</div>
          </div>
        </div>

        <div style={{ padding: '18px 22px', display: 'flex', flexDirection: 'column', gap: 16 }}>
          <div>
            <div style={{ fontSize: 11, fontWeight: 600, letterSpacing: '0.08em', textTransform: 'uppercase', color: t.fg3, marginBottom: 8 }}>Recommended — Apple's signed installer</div>
            <button onClick={openApple} style={{
              display: 'flex', alignItems: 'center', gap: 10, width: '100%',
              padding: '12px 14px', background: t.fg1, color: t.bg, border: 'none',
              borderRadius: 8, fontSize: 13, fontFamily: 'inherit', fontWeight: 500,
              cursor: 'pointer', textAlign: 'left',
            }}>
              <Icon name="download" size={16} color={t.bg} />
              <div style={{ flex: 1 }}>
                <div>Download from apple/container releases</div>
                <div style={{ fontSize: 11, opacity: 0.7, marginTop: 2, fontFamily: t.mono }}>github.com/apple/container/releases</div>
              </div>
              <span style={{ fontSize: 14, opacity: 0.7 }}>↗</span>
            </button>
            <div style={{ fontSize: 11, color: t.fg3, marginTop: 8, lineHeight: 1.5 }}>
              Apple's signed <code style={{ fontFamily: t.mono }}>.pkg</code> is the canonical install. Includes update + uninstall scripts and verifies macOS 26+.
            </div>
          </div>

          <div style={{ height: 1, background: t.border }} />

          <div>
            <div style={{ fontSize: 11, fontWeight: 600, letterSpacing: '0.08em', textTransform: 'uppercase', color: t.fg3, marginBottom: 8 }}>Or, with Homebrew</div>
            <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
              <code style={{
                flex: 1, padding: '8px 12px', background: t.bg,
                border: `1px solid ${t.border}`, borderRadius: 6,
                fontFamily: t.mono, fontSize: 12, color: t.fg1,
              }}>brew install container</code>
              <button onClick={copyBrew} style={pillBtn(t)}>{copied ? '✓ Copied' : 'Copy'}</button>
            </div>
          </div>
        </div>

        <div style={{ padding: '12px 22px', borderTop: `1px solid ${t.border}`, display: 'flex', gap: 8, justifyContent: 'space-between', alignItems: 'center', background: t.surfaceAlt }}>
          <span style={{ fontSize: 11, color: t.fg3, fontFamily: t.mono }}>
            {checking ? 'Re-checking…' : 'Already installed?'}
          </span>
          <div style={{ display: 'flex', gap: 8 }}>
            <button onClick={onDismiss} style={pillBtn(t)}>Continue with sample data</button>
            <button onClick={recheck} disabled={checking}
                    style={{ padding: '6px 14px', background: t.accent, color: t.accentInk, border: 'none', borderRadius: 6, fontSize: 12, fontWeight: 500, cursor: checking ? 'wait' : 'pointer', opacity: checking ? 0.6 : 1 }}>
              Re-check
            </button>
          </div>
        </div>
      </div>
    </Backdrop>
  );
}
