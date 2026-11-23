import { useEffect, useMemo, useState } from 'react';
import { api } from '../api';
import { useCliAtLeast } from '../cliVersion';
import { Icon, iconBtn, pillBtn } from '../components';
import { commitContainerFlow } from '../containerActions';
import { askText } from '../dialogs';
import { errMessage } from '../loadState';
import type { ThemeTokens } from '../theme';
import { toast, withToast } from '../toast';
import type { Container } from '../types';
import { Backdrop } from './shared';
import { InspectTab, parseInspect } from './inspect';
import { InspectTabs, InspectPanel } from './InspectPanel';

export function DetailModal({ item, t, onClose, onExec, onImagesChanged }: {
  item: Container; t: ThemeTokens; onClose: () => void;
  // Open the embedded terminal for this container. The host (ModalsHost)
  // swaps the modal type so the DetailModal closes as the Terminal mounts.
  onExec: (c: Container) => void;
  // A commit created a new image: refresh the Images list.
  onImagesChanged?: () => void;
}) {
  const commitGate = useCliAtLeast('1.6.0');
  const [json, setJson] = useState<string>('Loading…');
  const [tab, setTab] = useState<InspectTab>('trends');
  useEffect(() => {
    api.inspectContainer(item.id).then(setJson)
      .catch(e => setJson(`Failed to load inspect data: ${errMessage(e)}`));
  }, [item.id]);
  const c = item;
  // Parse once per JSON change. Errors are tolerated — the Raw tab still
  // shows whatever came back, even when it isn't valid JSON.
  const parsed = useMemo(() => parseInspect(json), [json]);
  return (
    <Backdrop title={`Container ${c.name}`} onClose={onClose}>
      <div style={{ width: 760, maxHeight: '85vh', background: t.surface, border: `1px solid ${t.border}`, borderRadius: 12, overflow: 'hidden', display: 'flex', flexDirection: 'column', boxShadow: '0 24px 80px rgba(0,0,0,0.4)' }}>
        <div style={{ padding: '18px 22px', borderBottom: `1px solid ${t.border}`, display: 'flex', alignItems: 'flex-start', gap: 12 }}>
          <Icon name="box" size={20} color={t.accent} />
          <div style={{ flex: 1 }}>
            <div style={{ fontSize: 18, fontWeight: 600, color: t.fg1 }}>{c.name}</div>
            <div style={{ fontSize: 12, color: t.fg3, fontFamily: t.mono, marginTop: 4 }}>{c.shortId} · {c.image}</div>
          </div>
          <button onClick={onClose} style={iconBtn()} aria-label="Close"><Icon name="x" size={16} color={t.fg2} /></button>
        </div>
        <div style={{ padding: '14px 22px', display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: 14, borderBottom: `1px solid ${t.border}` }}>
          {[
            { l: 'Status', v: c.status, color: c.status === 'running' ? t.success : t.fg3 },
            { l: 'Uptime', v: c.uptime, color: undefined as string | undefined },
            { l: 'CPU', v: `${c.cpu.toFixed(1)}%`, color: c.cpu > 85 ? t.danger : c.cpu > 60 ? t.warning : t.fg1 },
            { l: 'Memory', v: `${c.mem.used.toFixed(1)} / ${c.mem.limit.toFixed(0)} GiB`, color: undefined as string | undefined },
          ].map((m, i) => (
            <div key={i}>
              <div style={{ fontSize: 10, color: t.fg3, fontWeight: 600, letterSpacing: '0.08em', textTransform: 'uppercase' }}>{m.l}</div>
              <div style={{ marginTop: 4, fontSize: 16, fontWeight: 500, color: m.color || t.fg1, textTransform: 'capitalize' }}>{m.v}</div>
            </div>
          ))}
        </div>
        <div style={{ padding: '14px 22px', borderBottom: `1px solid ${t.border}` }}>
          <div style={{ fontSize: 10, color: t.fg3, fontWeight: 600, letterSpacing: '0.08em', textTransform: 'uppercase', marginBottom: 6 }}>Command</div>
          <div style={{ fontFamily: t.mono, fontSize: 12, color: t.fg2, padding: '8px 10px', background: t.bg, border: `1px solid ${t.border}`, borderRadius: 6 }}>{c.cmd?.length ? c.cmd.join(' ') : '—'}</div>
        </div>
        <InspectTabs t={t} tab={tab} setTab={setTab} parsed={parsed} />
        <div style={{ flex: 1, overflow: 'auto', padding: '14px 22px', minHeight: 200 }}>
          <InspectPanel t={t} tab={tab} parsed={parsed} json={json} container={c} />
        </div>
        <div style={{ padding: '12px 22px', borderTop: `1px solid ${t.border}`, display: 'flex', flexWrap: 'wrap', gap: 8, justifyContent: 'flex-end', background: t.surfaceAlt }}>
          {/* File operations (phase 2): copy in/out + filesystem export. */}
          <button style={pillBtn(t)} title="Copy a file from this container to a local folder"
                  onClick={async () => {
                    const src = await askText(`Path inside ${c.name} to copy out:`, '/');
                    if (!src) return;
                    const { open } = await import('@tauri-apps/plugin-dialog');
                    const dir = await open({ directory: true, multiple: false, title: 'Destination folder' });
                    if (typeof dir !== 'string') return;
                    withToast(`copy out ${src}`, api.copyPath(`${c.id}:${src}`, dir)).catch(() => {});
                  }}>Copy out…</button>
          <button style={pillBtn(t)} title="Copy a local file into this container"
                  onClick={async () => {
                    const { open } = await import('@tauri-apps/plugin-dialog');
                    const file = await open({ multiple: false, title: 'File to copy into the container' });
                    if (typeof file !== 'string') return;
                    const dst = await askText(`Destination path inside ${c.name}:`, '/tmp/');
                    if (!dst) return;
                    withToast('copy in', api.copyPath(file, `${c.id}:${dst}`)).catch(() => {});
                  }}>Copy in…</button>
          <button style={pillBtn(t)} title="Export the container's filesystem as a tar archive"
                  onClick={async () => {
                    const { save } = await import('@tauri-apps/plugin-dialog');
                    const dest = await save({ defaultPath: `${c.name}.tar`, filters: [{ name: 'tar archive', extensions: ['tar'] }] });
                    if (!dest) return;
                    withToast(`export ${c.name}`, api.exportContainer(c.id, dest)).catch(() => {});
                  }}>Export tar…</button>
          <button style={{ ...pillBtn(t), opacity: commitGate.ok ? 1 : 0.5 }} disabled={!commitGate.ok}
                  title={commitGate.title ?? 'Save this container’s filesystem as a new image (container commit). Works on running and stopped containers.'}
                  onClick={() => { void commitContainerFlow(c, onImagesChanged); }}>Commit to image…</button>
          <div style={{ flex: 1 }} />
          {c.status === 'running' && (
            <button style={pillBtn(t)}
                    title="Trim the container's disk images to return freed space to macOS (container clean). Running containers only; no data is removed."
                    onClick={() => withToast(`reclaim space in ${c.name}`, api.cleanContainers([c.id]))
                      .then(() => toast(`Reclaimed space in ${c.name}`, 'info'))
                      .catch(() => {})}>Reclaim space</button>
          )}
          <button style={pillBtn(t)} onClick={() => withToast(`restart ${c.name}`, api.restartContainer(c.id)).catch(() => {})}>Restart</button>
          <button style={pillBtn(t)} onClick={() => onExec(c)} disabled={c.status !== 'running'}
                  title={c.status === 'running' ? 'Open embedded terminal' : 'Container is not running'}>
            Exec /bin/sh
          </button>
          {c.status === 'running' && (
            <button style={pillBtn(t, t.danger)}
                    title="Send a signal to the container's init process"
                    onClick={async () => {
                      const sig = await askText(
                        `Signal to send to ${c.name}:\n(SIGKILL force-kills; SIGTERM asks it to exit cleanly; SIGHUP reloads many daemons)`,
                        'SIGKILL'
                      );
                      if (!sig) return;
                      withToast(`kill ${c.name} (${sig.trim()})`, api.killContainer(c.id, sig.trim())).then(onClose).catch(() => {});
                    }}>Kill…</button>
          )}
          {c.status === 'running'
            ? <button style={pillBtn(t, t.danger)} onClick={() => withToast(`stop ${c.name}`, api.stopContainer(c.id)).then(onClose).catch(() => {})}>Stop</button>
            : <button style={pillBtn(t, t.success)} onClick={() => withToast(`start ${c.name}`, api.startContainer(c.id)).then(onClose).catch(() => {})}>Start</button>}
        </div>
      </div>
    </Backdrop>
  );
}
