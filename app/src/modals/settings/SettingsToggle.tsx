import { useId } from 'react';
import type { ThemeTokens } from '../../theme';

// Tiny labeled-toggle row used by SettingsModal. Pure CSS switch so the
// modal stays a leaf chunk with no extra deps.
export function SettingsToggle({ t, label, hint, value, onChange }: {
  t: ThemeTokens; label: string; hint?: string;
  value: boolean; onChange: (v: boolean) => void;
}) {
  const uid = useId();
  const labelId = `${uid}-label`;
  const hintId = `${uid}-hint`;
  return (
    <div style={{
      display: 'flex', alignItems: 'center', justifyContent: 'space-between',
      padding: '10px 0', borderBottom: `1px solid ${t.border}`, gap: 12,
    }}>
      <div style={{ minWidth: 0 }}>
        <div id={labelId} style={{ fontSize: 13, color: t.fg1 }}>{label}</div>
        {hint && <div id={hintId} style={{ fontSize: 11, color: t.fg3, marginTop: 2 }}>{hint}</div>}
      </div>
      <button
        type="button" role="switch" aria-checked={value}
        aria-labelledby={labelId} aria-describedby={hint ? hintId : undefined}
        onClick={() => onChange(!value)}
        style={{
          flex: '0 0 auto',
          width: 38, height: 22, borderRadius: 999,
          background: value ? t.accent : t.surfaceAlt,
          border: `1px solid ${value ? t.accent : t.border}`,
          position: 'relative', cursor: 'pointer', transition: 'background 120ms ease',
        }}
      >
        <span style={{
          position: 'absolute', top: 1, left: value ? 17 : 1,
          width: 18, height: 18, borderRadius: '50%',
          background: '#fff',
          transition: 'left 120ms ease',
        }} />
      </button>
    </div>
  );
}
