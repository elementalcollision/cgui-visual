// Shared accessibility primitives: the modal dialog shell (role, labelling,
// Esc, focus move/trap/restore, backdrop click) and a visually-hidden helper.
// Kept dependency-free and small: TextPromptHost (not lazy) imports it too.

import { useEffect, useRef, useState, type CSSProperties, type ReactNode } from 'react';

// Visually hidden but exposed to assistive tech (the usual sr-only recipe).
export const srOnly: CSSProperties = {
  position: 'absolute', width: 1, height: 1, padding: 0, margin: -1,
  overflow: 'hidden', clip: 'rect(0 0 0 0)', whiteSpace: 'nowrap', border: 0,
};

export function VisuallyHidden({ children }: { children: ReactNode }) {
  return <span style={srOnly}>{children}</span>;
}

const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]):not([type="hidden"]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

function focusables(root: HTMLElement): HTMLElement[] {
  return Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE))
    .filter(el => el.getAttribute('aria-hidden') !== 'true');
}

// Open dialogs, oldest first. Only the last one reacts to Esc / Tab, so a
// prompt stacked on top of a modal closes alone.
const stack: symbol[] = [];

// Wires one dialog container: moves focus in on open, traps Tab, closes on
// Esc (unless escapeCloses is false, e.g. a terminal where Esc belongs to the
// shell), and restores focus to the previously focused element on close.
export function useDialog(onClose: () => void, opts: { escapeCloses?: boolean } = {}) {
  const ref = useRef<HTMLDivElement>(null);
  // Captured during the first render: autoFocus children steal focus at
  // commit, before any effect runs, so an effect would see the wrong element.
  const [opener] = useState<Element | null>(() => (typeof document === 'undefined' ? null : document.activeElement));
  const closeRef = useRef(onClose);
  closeRef.current = onClose;
  const escapeCloses = opts.escapeCloses !== false;

  useEffect(() => {
    const el = ref.current;
    const id = Symbol('dialog');
    stack.push(id);
    if (el && !el.contains(document.activeElement)) el.focus();

    const onKey = (e: KeyboardEvent) => {
      if (stack[stack.length - 1] !== id || !el) return;
      if (e.key === 'Escape') {
        if (e.defaultPrevented || !escapeCloses) return;
        e.preventDefault();
        closeRef.current();
        return;
      }
      if (e.key !== 'Tab' || e.defaultPrevented) return;
      const items = focusables(el);
      if (items.length === 0) { e.preventDefault(); el.focus(); return; }
      const first = items[0];
      const last = items[items.length - 1];
      const active = document.activeElement as HTMLElement | null;
      if (!active || !el.contains(active) || active === el) {
        e.preventDefault();
        (e.shiftKey ? last : first).focus();
      } else if (e.shiftKey && active === first) {
        e.preventDefault(); last.focus();
      } else if (!e.shiftKey && active === last) {
        e.preventDefault(); first.focus();
      }
    };
    document.addEventListener('keydown', onKey);
    return () => {
      document.removeEventListener('keydown', onKey);
      const i = stack.indexOf(id);
      if (i >= 0) stack.splice(i, 1);
      if (opener instanceof HTMLElement && opener.isConnected) opener.focus();
    };
  }, [opener, escapeCloses]);

  return ref;
}

// Modal overlay + dialog container. A click closes only when both mousedown
// and mouseup landed on the backdrop itself, so a drag-select that ends
// outside the dialog doesn't dismiss it (and lose form state).
export function DialogBackdrop({ onClose, children, title, labelId, background = 'rgba(0,0,0,0.45)', zIndex = 100, escapeCloses }: {
  onClose: () => void;
  children: ReactNode;
  /** Accessible name (aria-label). Prefer labelId when a visible heading exists. */
  title?: string;
  /** id of the element naming the dialog (aria-labelledby). */
  labelId?: string;
  background?: string;
  zIndex?: number;
  escapeCloses?: boolean;
}) {
  const ref = useDialog(onClose, { escapeCloses });
  const downOnBackdrop = useRef(false);
  return (
    <div
      onMouseDown={e => { downOnBackdrop.current = e.target === e.currentTarget; }}
      onClick={e => {
        const ok = downOnBackdrop.current && e.target === e.currentTarget;
        downOnBackdrop.current = false;
        if (ok) onClose();
      }}
      style={{ position: 'absolute', inset: 0, background, display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex, backdropFilter: 'blur(2px)' }}>
      <div
        ref={ref} role="dialog" aria-modal="true" tabIndex={-1}
        aria-label={labelId ? undefined : title} aria-labelledby={labelId}
        style={{ outline: 'none' }}>
        {children}
      </div>
    </div>
  );
}
