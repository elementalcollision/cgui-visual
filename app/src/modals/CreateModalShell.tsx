import type { ReactNode } from 'react';
import { Icon, iconBtn, pillBtn } from '../components';
import type { ThemeTokens } from '../theme';
import { Backdrop } from './shared';

// Small create forms for volumes and networks (`container volume create`,
// `container network create`). Fields validate inline; Create stays disabled
// until the form is valid, and CLI errors surface via withToast.
export function useCreateFormStyles(t: ThemeTokens) {
  return {
    input: {
      width: '100%', padding: '8px 10px', background: t.bg, border: `1px solid ${t.border}`,
      borderRadius: 6, color: t.fg1, fontSize: 12, fontFamily: t.mono, outline: 'none',
      boxSizing: 'border-box' as const,
    },
    label: { fontSize: 11, fontWeight: 600, letterSpacing: '0.08em', textTransform: 'uppercase' as const, color: t.fg3, marginBottom: 6, display: 'block' },
    opt: { color: t.fg3, fontWeight: 400, textTransform: 'none' as const, letterSpacing: 0 },
  };
}

export function FieldNote({ t, error, children }: { t: ThemeTokens; error?: string | null; children?: ReactNode }) {
  if (!error && !children) return null;
  return <div style={{ fontSize: 10, color: error ? t.danger : t.fg3, marginTop: 4, lineHeight: 1.4 }}>{error ?? children}</div>;
}

export function CreateModalShell({ t, title, subtitle, busy, canSubmit, submitLabel, onSubmit, onClose, children }: {
  t: ThemeTokens; title: string; subtitle: string; busy: boolean; canSubmit: boolean;
  submitLabel: string; onSubmit: () => void; onClose: () => void; children: ReactNode;
}) {
  return (
    <Backdrop onClose={onClose} title={title}>
      <form onSubmit={e => { e.preventDefault(); if (canSubmit && !busy) onSubmit(); }}
            style={{ width: 460, background: t.surface, border: `1px solid ${t.border}`, borderRadius: 12, overflow: 'hidden', display: 'flex', flexDirection: 'column', boxShadow: '0 24px 80px rgba(0,0,0,0.4)' }}>
        <div style={{ padding: '18px 22px', borderBottom: `1px solid ${t.border}`, display: 'flex', alignItems: 'flex-start', gap: 12 }}>
          <Icon name="plus" size={20} color={t.accent} />
          <div style={{ flex: 1 }}>
            <div style={{ fontSize: 18, fontWeight: 600, color: t.fg1 }}>{title}</div>
            <div style={{ fontSize: 12, color: t.fg3, fontFamily: t.mono, marginTop: 4 }}>{subtitle}</div>
          </div>
          <button type="button" onClick={onClose} style={iconBtn()} aria-label="Close" title="Close"><Icon name="x" size={16} color={t.fg2} /></button>
        </div>
        <div style={{ padding: '18px 22px', display: 'flex', flexDirection: 'column', gap: 14, maxHeight: '62vh', overflow: 'auto' }}>
          {children}
        </div>
        <div style={{ padding: '12px 22px', borderTop: `1px solid ${t.border}`, display: 'flex', gap: 8, justifyContent: 'flex-end', background: t.surfaceAlt }}>
          <button type="button" style={pillBtn(t)} onClick={onClose} disabled={busy}>Cancel</button>
          <button type="submit" disabled={!canSubmit || busy}
                  style={{ padding: '6px 14px', background: t.fg1, color: t.bg, border: 'none', borderRadius: 6, fontSize: 12, fontWeight: 500, cursor: busy ? 'wait' : 'pointer', opacity: !canSubmit || busy ? 0.5 : 1 }}>
            {busy ? 'Creating…' : submitLabel}
          </button>
        </div>
      </form>
    </Backdrop>
  );
}
