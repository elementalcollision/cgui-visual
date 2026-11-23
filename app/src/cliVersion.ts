// Capability gate: features that need a newer Apple `container` CLI are shown
// disabled (with a tooltip) on older CLIs instead of failing when clicked.
//
//   const gate = useCliAtLeast('1.6.0');
//   <button disabled={!gate.ok} title={gate.ok ? 'Commit…' : gate.title}>
//
// An unknown version (CLI missing, unparseable output, still detecting) fails
// open: the CLI itself will report anything it can't do.

import { useEffect, useState } from 'react';
import { api } from './api';

/** Numeric components of `x.y.z`; ignores a leading `v`, `-pre` and `+build`. */
function parts(v: string): number[] | null {
  const core = v.trim().replace(/^v/, '').split('+')[0].split('-')[0];
  const nums = core.split('.').map(p => (/^\d+$/.test(p) ? Number(p) : NaN));
  return nums.length > 0 && nums.every(n => !Number.isNaN(n)) ? nums : null;
}

/** True when `version` >= `min`. Unparseable input on either side is `true` (fail open). */
export function versionAtLeast(version: string | null | undefined, min: string): boolean {
  if (!version) return true;
  const a = parts(version);
  const b = parts(min);
  if (!a || !b) return true;
  const n = Math.max(a.length, b.length);
  for (let i = 0; i < n; i++) {
    const d = (a[i] ?? 0) - (b[i] ?? 0);
    if (d !== 0) return d > 0;
  }
  return true;
}

/** "Needs Apple container 1.6 or later" (trailing `.0` components dropped). */
export function needsCliTitle(min: string): string {
  const shown = (parts(min) ?? [])
    .join('.')
    .replace(/(\.0)+$/, '');
  return `Needs Apple container ${shown || min} or later`;
}

let cached: Promise<string | null> | null = null;

/** Detect the CLI version once per session (rejections aren't cached). */
function detect(): Promise<string | null> {
  if (!cached) {
    cached = api.cliVersion().catch(() => null).then(v => {
      if (v === null) cached = null; // retry next time rather than pin "unknown"
      return v;
    });
  }
  return cached;
}

/** Forget the detected version (after a services restart / CLI upgrade). */
export function resetCliVersion(): void {
  cached = null;
}

export interface CliGate {
  /** Feature usable (or version still unknown). */
  ok: boolean;
  /** Tooltip to put on the disabled control; undefined when usable. */
  title: string | undefined;
}

/** Detected CLI version: undefined while detecting, null when unknown. */
export function useCliVersion(): string | null | undefined {
  const [v, setV] = useState<string | null | undefined>(undefined);
  useEffect(() => {
    let live = true;
    detect().then(x => { if (live) setV(x); });
    return () => { live = false; };
  }, []);
  return v;
}

export function useCliAtLeast(min: string): CliGate {
  const v = useCliVersion();
  const ok = versionAtLeast(v, min);
  return { ok, title: ok ? undefined : needsCliTitle(min) };
}
