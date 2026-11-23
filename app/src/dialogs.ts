// Shared confirm / text-prompt helpers.
//
// Why this exists: `window.confirm` / `window.prompt` are NOT usable in the
// packaged macOS app.
//   * wry's WKUIDelegate (wry-0.57.3/src/wkwebview/class/wry_web_view_ui_delegate.rs)
//     implements only the file-open panel, media-capture permission and
//     createWebView. It does not implement runJavaScript{Alert,Confirm,
//     TextInput}Panel, so WebKit answers prompt() with null.
//   * tauri-plugin-dialog injects an init script (src/init-iife.js) that
//     replaces `window.confirm` with an `async` function. That returns a
//     Promise, and a Promise is always truthy, so `if (!confirm(...)) return;`
//     never returned: every confirmation was silently auto-accepted.
//
// So inside Tauri we use the plugin's native dialogs (awaited), and outside
// Tauri (browser dev server, tests) we fall back to the window functions.
// Every helper fails closed: if the native dialog cannot be shown the answer
// is "no" / null, never "yes".

export interface ConfirmOptions {
  title?: string;
  okLabel?: string;
  cancelLabel?: string;
  kind?: 'info' | 'warning' | 'error';
}

function inTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

// Resolves true only when the user explicitly picks the OK button.
export async function askConfirm(message: string, opts: ConfirmOptions = {}): Promise<boolean> {
  if (!inTauri()) {
    return typeof window !== 'undefined' && window.confirm(message) === true;
  }
  try {
    const { confirm } = await import('@tauri-apps/plugin-dialog');
    return (await confirm(message, {
      title: opts.title,
      kind: opts.kind ?? 'warning',
      okLabel: opts.okLabel ?? 'OK',
      cancelLabel: opts.cancelLabel ?? 'Cancel',
    })) === true;
  } catch {
    return false;
  }
}

// ---- text prompt -------------------------------------------------------
// The dialog plugin has no text-input dialog, so while <TextPromptHost/>
// (modals.tsx, mounted once in App) is mounted this opens an in-app modal.
// Requests are served one at a time.

export interface TextRequest {
  message: string;
  defaultValue: string;
  resolve: (v: string | null) => void;
}

let current: TextRequest | null = null;
const waiting: TextRequest[] = [];
const listeners = new Set<(r: TextRequest | null) => void>();

function emit() { for (const l of listeners) l(current); }

export function subscribeTextPrompt(fn: (r: TextRequest | null) => void): () => void {
  listeners.add(fn);
  fn(current);
  return () => { listeners.delete(fn); };
}

// Called by the host when the user submits (string) or cancels (null).
export function settleTextPrompt(value: string | null) {
  const r = current;
  if (!r) return;
  current = waiting.shift() ?? null;
  r.resolve(value);
  emit();
}

// Resolves with the entered string, or null when cancelled.
export function askText(message: string, defaultValue = ''): Promise<string | null> {
  // No in-app host mounted (browser dev server, isolated component tests):
  // use the browser's own prompt.
  if (listeners.size === 0) {
    if (typeof window === 'undefined') return Promise.resolve(null);
    return Promise.resolve(window.prompt(message, defaultValue));
  }
  return new Promise(resolve => {
    const req: TextRequest = { message, defaultValue, resolve };
    if (current) waiting.push(req); else { current = req; emit(); }
  });
}
