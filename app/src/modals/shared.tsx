import type { ReactNode } from 'react';
import { DialogBackdrop } from '../a11y';
import type { ThemeTokens } from '../theme';

// Shared modal shell (role=dialog, Esc, focus trap/restore, drag-safe backdrop
// click) lives in a11y.tsx. `title` is the dialog's accessible name; always
// pass one for new modals.
export function Backdrop({ onClose, title, children }: { onClose: () => void; title?: string; children: ReactNode }) {
  return <DialogBackdrop onClose={onClose} title={title}>{children}</DialogBackdrop>;
}

export function Muted({ t, children }: { t: ThemeTokens; children: ReactNode }) {
  return <div style={{ fontSize: 12, color: t.fg3, fontStyle: 'italic' }}>{children}</div>;
}
