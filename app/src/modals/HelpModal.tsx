import { Fragment } from 'react';
import { Icon, iconBtn } from '../components';
import type { ThemeTokens } from '../theme';
import { Backdrop } from './shared';

// Keyboard-shortcut cheat sheet. Triggered by '?' anywhere in the app
// (matches the StatusBar hint). Static — no state — so it's cheap to
// render every time. Grouped so users can scan to the section they
// need rather than scrolling a flat list.
export function HelpModal({ t, onClose }: { t: ThemeTokens; onClose: () => void }) {
  type Row = { keys: string; label: string };
  type Group = { title: string; rows: Row[] };
  const groups: Group[] = [
    {
      title: 'Global',
      rows: [
        { keys: '⌘K', label: 'Open command palette' },
        { keys: '⌘,', label: 'Open Settings' },
        { keys: '⌘1 … ⌘6', label: 'Switch tabs (Containers / Images / Volumes / Networks / Stacks / Logs)' },
        { keys: '/', label: 'Focus the filter input' },
        { keys: '?', label: 'Open this help' },
        { keys: 'Esc', label: 'Close the active modal' },
      ],
    },
    {
      title: 'Containers tab',
      rows: [
        { keys: '↑ / ↓', label: 'Move row selection up / down' },
        { keys: '↵', label: 'Inspect the selected container (open detail)' },
        { keys: 'L', label: 'Open Logs for the selected container' },
      ],
    },
    {
      title: 'Images tab',
      rows: [
        { keys: 'S', label: 'Run a Trivy scan on the first visible image' },
      ],
    },
  ];
  return (
    <Backdrop title="Keyboard shortcuts" onClose={onClose}>
      <div style={{ width: 580, maxHeight: '85vh', background: t.surface, border: `1px solid ${t.border}`, borderRadius: 12, overflow: 'hidden', display: 'flex', flexDirection: 'column' }}>
        <div style={{ padding: '14px 22px', borderBottom: `1px solid ${t.border}`, display: 'flex', alignItems: 'center', gap: 12 }}>
          <Icon name="info" size={18} color={t.fg2} />
          <div style={{ flex: 1, fontSize: 15, fontWeight: 600, color: t.fg1 }}>Keyboard shortcuts</div>
          <button onClick={onClose} style={iconBtn()} aria-label="Close"><Icon name="x" size={16} color={t.fg2} /></button>
        </div>
        <div style={{ flex: 1, overflow: 'auto', padding: '14px 22px', display: 'flex', flexDirection: 'column', gap: 18 }}>
          {groups.map(g => (
            <div key={g.title}>
              <div style={{ fontSize: 10, fontWeight: 600, letterSpacing: '0.08em', textTransform: 'uppercase', color: t.fg3, marginBottom: 8 }}>{g.title}</div>
              <div style={{ display: 'grid', gridTemplateColumns: '120px 1fr', rowGap: 6, columnGap: 14 }}>
                {g.rows.map(r => (
                  <Fragment key={r.keys + r.label}>
                    <kbd style={{
                      fontFamily: t.mono, fontSize: 11, color: t.fg1,
                      background: t.surfaceAlt, border: `1px solid ${t.border}`,
                      borderRadius: 4, padding: '2px 6px',
                      whiteSpace: 'nowrap',
                      justifySelf: 'start',
                    }}>{r.keys}</kbd>
                    <span style={{ fontSize: 12, color: t.fg2 }}>{r.label}</span>
                  </Fragment>
                ))}
              </div>
            </div>
          ))}
        </div>
        <div style={{ padding: '10px 22px', borderTop: `1px solid ${t.border}`, background: t.surfaceAlt, fontFamily: t.mono, fontSize: 10, color: t.fg3 }}>
          Esc to close
        </div>
      </div>
    </Backdrop>
  );
}
