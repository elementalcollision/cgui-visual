// UI strings are rendered as plain text, never markdown. Backend errors and
// hints are written with `backticks` around commands; drop the markers
// before anything user-visible so they don't show up literally.
export function plainText(s: string): string {
  return s.replace(/`/g, '');
}

// Rejections from the backend (strings or Error objects) with the markers
// stripped, preserving the original shape.
export function plainError(e: unknown): unknown {
  if (typeof e === 'string') return plainText(e);
  if (e && typeof (e as { message?: unknown }).message === 'string') {
    const o = e as { message: string };
    if (o.message.includes('`')) o.message = plainText(o.message);
  }
  return e;
}
