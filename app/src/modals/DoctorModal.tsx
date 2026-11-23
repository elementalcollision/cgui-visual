import { useEffect, useState } from 'react';
import { api } from '../api';
import { Icon, iconBtn } from '../components';
import type { ThemeTokens } from '../theme';
import type { DoctorCheck, DoctorFix } from '../types';
import { Backdrop } from './shared';

// Fire a DoctorFix. URL fixes go through the opener plugin (or a fallback
// window.open in browser-dev mode). Copy fixes write to the clipboard and
// flash the button label so the user sees confirmation.
// Maps a DoctorFix `run` action token to its dedicated, fixed-argv
// command. Closed set — an unknown action throws rather than doing
// anything resembling arbitrary execution.
const DOCTOR_ACTIONS: Record<string, () => Promise<void>> = {
  'system-start': () => api.systemStart(),
  'system-restart': () => api.systemRestart(),
};

async function runDoctorFix(fix: DoctorFix): Promise<'opened' | 'copied' | 'ran'> {
  if (fix.kind === 'url') {
    try {
      const { openUrl } = await import('@tauri-apps/plugin-opener');
      await openUrl(fix.url);
    } catch {
      window.open(fix.url, '_blank', 'noopener,noreferrer');
    }
    return 'opened';
  }
  if (fix.kind === 'run') {
    const action = DOCTOR_ACTIONS[fix.action];
    if (!action) throw new Error(`unknown doctor action: ${fix.action}`);
    await action();
    return 'ran';
  }
  await navigator.clipboard?.writeText(fix.command);
  return 'copied';
}

function DoctorRow({ t, d }: { t: ThemeTokens; d: DoctorCheck }) {
  const [flash, setFlash] = useState<'opened' | 'copied' | 'ran' | null>(null);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    if (!flash) return;
    const id = window.setTimeout(() => setFlash(null), 1500);
    return () => window.clearTimeout(id);
  }, [flash]);
  const onClick = async () => {
    if (!d.fix || busy) return;
    // `run` fixes shell out and can take a few seconds; reflect that.
    const isRun = d.fix.kind === 'run';
    if (isRun) setBusy(true);
    try {
      const verb = await runDoctorFix(d.fix);
      setFlash(verb);
    } catch (e) {
      // Fallback toast via the lazy import to avoid a circular dep.
      const { toast } = await import('../toast');
      toast(`Doctor fix failed: ${typeof e === 'string' ? e : (e as Error)?.message ?? 'unknown'}`);
    } finally {
      if (isRun) setBusy(false);
    }
  };
  const label = busy ? '…'
              : flash === 'copied' ? '✓ Copied'
              : flash === 'opened' ? '✓ Opened'
              : flash === 'ran' ? '✓ Done'
              : d.fix?.label;
  const fixTitle = !d.fix ? undefined
                 : d.fix.kind === 'url' ? d.fix.url
                 : d.fix.kind === 'copy' ? d.fix.command
                 : d.fix.label;
  return (
    <div style={{
      display: 'flex', alignItems: 'center', gap: 10, padding: '6px 0',
      color: d.ok ? t.fg1 : d.warn ? t.warning : t.danger,
    }}>
      <span style={{ width: 16, color: d.ok ? t.success : t.warning, fontWeight: 700 }}>{d.ok ? '✓' : '!'}</span>
      <span style={{ flex: 1 }}>{d.text}</span>
      {d.fix && (
        <button
          onClick={onClick}
          disabled={busy}
          style={{
            padding: '4px 10px',
            background: t.surfaceAlt, color: t.fg2,
            border: `1px solid ${t.border}`, borderRadius: 4,
            fontSize: 11, fontFamily: t.mono, cursor: busy ? 'wait' : 'pointer',
            whiteSpace: 'nowrap', opacity: busy ? 0.6 : 1,
          }}
          title={fixTitle}
        >{label}</button>
      )}
    </div>
  );
}

export function DoctorModal({ t, onClose }: { t: ThemeTokens; onClose: () => void }) {
  const [checks, setChecks] = useState<DoctorCheck[]>([]);
  useEffect(() => { api.doctor().then(setChecks); }, []);
  const passed = checks.filter(c => c.ok).length;
  const warns = checks.filter(c => !c.ok && c.warn).length;
  const fails = checks.filter(c => !c.ok && !c.warn).length;
  return (
    <Backdrop title="cgui doctor" onClose={onClose}>
      <div style={{ width: 600, background: t.surface, border: `1px solid ${t.border}`, borderRadius: 12, overflow: 'hidden' }}>
        <div style={{ padding: '16px 22px', borderBottom: `1px solid ${t.border}`, display: 'flex', alignItems: 'center', gap: 12 }}>
          <Icon name="heart" size={18} color={t.success} />
          <div style={{ flex: 1, fontSize: 15, fontWeight: 600, color: t.fg1 }}>cgui doctor</div>
          <button onClick={onClose} style={iconBtn()} aria-label="Close"><Icon name="x" size={16} color={t.fg2} /></button>
        </div>
        <div style={{ padding: '14px 22px', fontFamily: t.mono, fontSize: 12 }}>
          {checks.map((d, i) => <DoctorRow key={i} t={t} d={d} />)}
        </div>
        <div style={{ padding: '12px 22px', borderTop: `1px solid ${t.border}`, background: t.surfaceAlt, fontFamily: t.mono, fontSize: 11, color: t.fg3 }}>
          == {passed} passed · {warns} warnings · {fails} failures ==
        </div>
      </div>
    </Backdrop>
  );
}
