import { useEffect, useState } from 'react';
import { api } from '../../api';
import { pillBtn } from '../../components';
import { askConfirm } from '../../dialogs';
import { errMessage, looksLikeServicesDown } from '../../loadState';
import type { ThemeTokens } from '../../theme';
import { withToast } from '../../toast';
import type { Container } from '../../types';

// Builder VM state (`container builder status/start/stop/delete`).
// The builder is the dedicated VM that `container build` runs in;
// surfacing it here gives users a handle when builds hang or the VM
// holds memory they want back.
export function BuilderPanel({ t }: { t: ThemeTokens }) {
  const [builder, setBuilder] = useState<Container | null | undefined>(undefined);
  const [busy, setBusy] = useState(false);
  const [servicesDown, setServicesDown] = useState(false);
  const reload = () => api.builderStatus()
    .then(b => { setServicesDown(false); setBuilder(b); })
    .catch(e => {
      // `container builder` is a CLI plugin on 1.6+: with services stopped
      // it fails with "Plugins are unavailable ...".
      setServicesDown(looksLikeServicesDown(errMessage(e)));
      setBuilder(null);
    });
  useEffect(() => { reload(); }, []);
  const act = (label: string, p: Promise<void>) => {
    setBusy(true);
    withToast(label, p).then(reload).catch(() => {}).finally(() => setBusy(false));
  };
  if (servicesDown) {
    return (
      <div style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '8px 0' }}>
        <span style={{ width: 8, height: 8, borderRadius: '50%', background: t.warning }} />
        <div style={{ flex: 1, fontSize: 13, color: t.fg1 }}>Container services aren't running</div>
        <button style={{ ...pillBtn(t, t.success), opacity: busy ? 0.5 : 1 }} disabled={busy}
                onClick={() => act('start services', api.systemStart())}>Start services</button>
      </div>
    );
  }
  const running = builder?.status === 'running';
  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '8px 0' }}>
      <span style={{ width: 8, height: 8, borderRadius: '50%', background: builder === undefined ? t.fg3 : running ? t.success : t.fg3 }} />
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ fontSize: 13, color: t.fg1 }}>
          {builder === undefined ? 'Probing…' : builder === null ? 'No builder instance' : `Builder ${builder.status}`}
        </div>
        {builder && (
          <div style={{ fontSize: 11, color: t.fg3, fontFamily: t.mono, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
            {builder.image} · {builder.mem.limit.toFixed(0)} {builder.mem.unit} limit
          </div>
        )}
      </div>
      {running
        ? <button style={{ ...pillBtn(t), opacity: busy ? 0.5 : 1 }} disabled={busy}
                  onClick={() => act('stop builder', api.builderStop())}>Stop</button>
        : <button style={{ ...pillBtn(t), opacity: busy ? 0.5 : 1 }} disabled={busy}
                  onClick={() => act('start builder', api.builderStart())}>Start</button>}
      {builder && (
        <button style={{ ...pillBtn(t, t.danger), opacity: busy ? 0.5 : 1 }} disabled={busy}
                title="Delete the builder VM and its cache; the next build recreates it"
                onClick={async () => {
                  if (!(await askConfirm('Delete the builder VM?\nIts build cache is lost; the next build recreates it.', { title: 'Delete builder', okLabel: 'Delete' }))) return;
                  act('delete builder', api.builderDelete());
                }}>Delete</button>
      )}
    </div>
  );
}
