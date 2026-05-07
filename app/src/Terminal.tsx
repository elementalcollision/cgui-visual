// Embedded terminal modal (B5).
//
// Lazy-loaded via `import('./Terminal')` from ModalsHost so xterm.js
// (≈250 KB) only ships to users who actually open a terminal. The
// component owns one xterm.js instance + one backend pty session for
// its lifetime; opening a second terminal makes a fresh session.

import { useEffect, useRef, useState } from 'react';
import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import '@xterm/xterm/css/xterm.css';
import { api } from './api';
import type { Container } from './types';
import type { ThemeTokens } from './theme';
import { Icon, iconBtn, pillBtn } from './components';

// Map our theme tokens to xterm.js's palette so the colours match the
// surrounding app chrome. Only the keys xterm cares about are set;
// everything else falls back to the default palette.
function xtermTheme(t: ThemeTokens) {
  return {
    background: t.bg,
    foreground: t.fg1,
    cursor: t.accent,
    cursorAccent: t.bg,
    selectionBackground: t.selected,
    black: '#0a0b10',
    red: t.danger,
    green: t.success,
    yellow: t.warning,
    blue: t.accent,
    magenta: '#c678dd',
    cyan: '#56b6c2',
    white: t.fg2,
    brightBlack: t.fg3,
    brightRed: t.danger,
    brightGreen: t.success,
    brightYellow: t.warning,
    brightBlue: t.accent,
    brightMagenta: '#c678dd',
    brightCyan: '#56b6c2',
    brightWhite: t.fg1,
  };
}

// Decode the base64 payload pty:tick events ship. We need the raw byte
// array (not a UTF-8 string) so xterm.js can correctly handle multi-
// byte sequences split across chunks; xterm's `write` accepts Uint8Array.
function b64ToBytes(b64: string): Uint8Array {
  const bin = atob(b64);
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  return bytes;
}

export default function TerminalModal({ container, t, onClose }: {
  container: Container;
  t: ThemeTokens;
  onClose: () => void;
}) {
  const hostRef = useRef<HTMLDivElement>(null);
  const [status, setStatus] = useState<'connecting' | 'live' | 'closed' | 'error'>('connecting');
  const [errMsg, setErrMsg] = useState<string>('');

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;

    let cancelled = false;
    let sessionId: string | null = null;
    let unlistenTick: (() => void) | null = null;
    let unlistenDone: (() => void) | null = null;

    const term = new Terminal({
      fontFamily: t.mono,
      fontSize: 13,
      theme: xtermTheme(t),
      cursorBlink: true,
      scrollback: 5000,
      convertEol: true,
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(host);
    // Initial fit can throw if the host hasn't laid out yet; defer one tick.
    requestAnimationFrame(() => {
      try { fit.fit(); } catch { /* layout race — will retry below */ }
    });

    const boot = async () => {
      try {
        const cols = term.cols || 80;
        const rows = term.rows || 24;
        const id = await api.ptyOpen(container.id, cols, rows);
        if (cancelled) {
          // Modal closed mid-spawn; immediately reap.
          api.ptyClose(id).catch(() => {});
          return;
        }
        sessionId = id;
        setStatus('live');

        // Pipe pty bytes → xterm.
        unlistenTick = await api.onPtyTick(id, b64 => {
          term.write(b64ToBytes(b64));
        });
        unlistenDone = await api.onPtyDone(id, () => {
          setStatus('closed');
        });
        // Pipe keystrokes → pty stdin.
        const sub = term.onData(data => {
          if (!sessionId) return;
          api.ptyWrite(sessionId, data).catch(e => {
            console.warn('pty_write failed', e);
          });
        });

        // Resize: refit on every window resize.
        const onResize = () => {
          try {
            fit.fit();
            if (sessionId) api.ptyResize(sessionId, term.cols, term.rows).catch(() => {});
          } catch { /* fit can throw mid-layout; ignore */ }
        };
        window.addEventListener('resize', onResize);

        // Stash teardown handles on the term so the cleanup fn below
        // can grab them without juggling closures.
        (term as unknown as { __subs: () => void }).__subs = () => {
          sub.dispose();
          window.removeEventListener('resize', onResize);
        };
      } catch (e: any) {
        setStatus('error');
        setErrMsg(typeof e === 'string' ? e : (e?.message ?? String(e)));
      }
    };
    boot();

    return () => {
      cancelled = true;
      unlistenTick?.();
      unlistenDone?.();
      const subs = (term as unknown as { __subs?: () => void }).__subs;
      subs?.();
      if (sessionId) api.ptyClose(sessionId).catch(() => {});
      term.dispose();
    };
    // Theme changes don't tear down the session — they just restyle xterm
    // on the next mount. eslint-disable: list intentionally minimal so
    // a single session lives for the modal's full lifetime.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [container.id]);

  const statusColor =
    status === 'live' ? t.success :
    status === 'connecting' ? t.warning :
    status === 'error' ? t.danger :
    t.fg3;

  return (
    <div onClick={onClose} style={{
      position: 'absolute', inset: 0, background: 'rgba(0,0,0,0.55)',
      display: 'flex', alignItems: 'center', justifyContent: 'center',
      zIndex: 100, backdropFilter: 'blur(2px)',
    }}>
      <div onClick={e => e.stopPropagation()} style={{
        width: '85vw', maxWidth: 1100, height: '78vh',
        background: t.surface, border: `1px solid ${t.border}`, borderRadius: 12,
        overflow: 'hidden', display: 'flex', flexDirection: 'column',
        boxShadow: '0 24px 80px rgba(0,0,0,0.45)',
      }}>
        <div style={{
          padding: '14px 18px', borderBottom: `1px solid ${t.border}`,
          display: 'flex', alignItems: 'center', gap: 12,
        }}>
          <Icon name="logs" size={18} color={t.accent} />
          <div style={{ flex: 1, minWidth: 0 }}>
            <div style={{ fontSize: 14, fontWeight: 600, color: t.fg1 }}>{container.name}</div>
            <div style={{ fontSize: 11, color: t.fg3, fontFamily: t.mono, marginTop: 2 }}>
              <span style={{ color: statusColor }}>● {status}</span>
              {' · '}{container.id}
              {' · '}exec -it {container.id} /bin/sh
            </div>
          </div>
          <button onClick={onClose} style={iconBtn()} title="Close terminal">
            <Icon name="x" size={16} color={t.fg2} />
          </button>
        </div>
        <div ref={hostRef} style={{
          flex: 1, minHeight: 0, padding: 8, background: t.bg,
        }} />
        {status === 'error' && (
          <div style={{
            padding: '10px 18px', borderTop: `1px solid ${t.border}`,
            background: t.surfaceAlt, fontFamily: t.mono, fontSize: 11, color: t.danger,
          }}>
            pty open failed: {errMsg}
          </div>
        )}
        {status === 'closed' && (
          <div style={{
            padding: '8px 18px', borderTop: `1px solid ${t.border}`,
            background: t.surfaceAlt, display: 'flex', alignItems: 'center', gap: 10,
            fontFamily: t.mono, fontSize: 11, color: t.fg3,
          }}>
            <span>Session ended.</span>
            <div style={{ flex: 1 }} />
            <button style={pillBtn(t)} onClick={onClose}>Close</button>
          </div>
        )}
      </div>
    </div>
  );
}
