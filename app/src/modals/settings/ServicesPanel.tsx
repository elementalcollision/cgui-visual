import { useEffect, useState } from 'react';
import { api } from '../../api';
import { pillBtn } from '../../components';
import { askConfirm } from '../../dialogs';
import type { ThemeTokens } from '../../theme';
import { withToast } from '../../toast';

// Runtime service controls (`container system start/stop`). Sudo-free,
// so wired as direct one-click buttons. Start is additive (no confirm);
// stop disrupts running containers, so it confirms first.
export function ServicesPanel({ t }: { t: ThemeTokens }) {
  const [running, setRunning] = useState<boolean | null>(null);
  const [busy, setBusy] = useState(false);
  const reload = () => api.systemStatus().then(setRunning).catch(() => setRunning(null));
  useEffect(() => { reload(); }, []);
  const act = (label: string, p: Promise<void>) => {
    setBusy(true);
    withToast(label, p).then(reload).catch(() => {}).finally(() => setBusy(false));
  };
  const dotColor = running === null ? t.fg3 : running ? t.success : t.warning;
  const stateLabel = running === null ? 'Checking…' : running ? 'Services running' : 'Services stopped';
  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '8px 0' }}>
      <span style={{ width: 8, height: 8, borderRadius: '50%', background: dotColor }} />
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ fontSize: 13, color: t.fg1 }}>{stateLabel}</div>
        <div style={{ fontSize: 11, color: t.fg3, fontFamily: t.mono }}>container system start / stop</div>
      </div>
      <button style={{ ...pillBtn(t), opacity: busy ? 0.5 : 1 }} disabled={busy}
              onClick={() => act('start services', api.systemStart())}>Start</button>
      <button style={{ ...pillBtn(t, t.danger), opacity: busy ? 0.5 : 1 }} disabled={busy}
              title="Stops the runtime services; running containers are affected"
              onClick={async () => {
                if (!(await askConfirm('Stop container services?\nRunning containers will be affected.', { title: 'Stop services', okLabel: 'Stop' }))) return;
                act('stop services', api.systemStop());
              }}>Stop</button>
    </div>
  );
}
