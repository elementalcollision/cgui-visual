import { useEffect, useState } from 'react';
import { api } from '../../api';
import { useCliAtLeast } from '../../cliVersion';
import { Icon, pillBtn } from '../../components';
import { askConfirm } from '../../dialogs';
import type { ThemeTokens } from '../../theme';
import { withToast } from '../../toast';
import { MachineLogsModal } from '../MachineLogsModal';
import { JsonInspectModal } from '../JsonInspectModal';

// Container machines (`container machine …`, new platform concept in
// 1.0): named VMs that containers run inside. Machines boot on demand
// when a container targets them, so there's no Start button — only
// Stop, Set default, Delete, and a create form.
export function MachinesPanel({ t }: { t: ThemeTokens }) {
  type M = { id: string; status: string; cpus: number; memory: number; diskSize: number; isDefault: boolean; createdDate: string };
  const [machines, setMachines] = useState<M[] | null>(null);
  const [busy, setBusy] = useState(false);
  const reload = () => api.machineList().then(setMachines).catch(() => setMachines([]));
  useEffect(() => { reload(); }, []);
  const [showCreate, setShowCreate] = useState(false);
  const [name, setName] = useState('');
  const [image, setImage] = useState('alpine:latest');
  const [cpus, setCpus] = useState('');
  const [memory, setMemory] = useState('');
  // Account options (container 1.6+): -u/--user, --uid, --gid, --home.
  const userGate = useCliAtLeast('1.6.0');
  const [showUser, setShowUser] = useState(false);
  const [mUser, setMUser] = useState('');
  const [mUid, setMUid] = useState('');
  const [mGid, setMGid] = useState('');
  const [mHome, setMHome] = useState('');
  const homeError = mHome.trim() && !mHome.trim().startsWith('/') ? 'The home directory must be an absolute path, e.g. /home/dev' : null;
  const userError = /\s/.test(mUser.trim()) ? 'The user must not contain spaces' : null;
  const userOpts = {
    user: mUser.trim() || undefined,
    uid: mUid.trim() ? Number(mUid.trim()) : undefined,
    gid: mGid.trim() ? Number(mGid.trim()) : undefined,
    home: mHome.trim() || undefined,
  };
  const hasUserOpts = Object.values(userOpts).some(v => v !== undefined);
  // Per-machine tools: `machine logs` and `machine inspect`.
  const [logsFor, setLogsFor] = useState<string | null>(null);
  const [inspectFor, setInspectFor] = useState<string | null>(null);
  const act = (label: string, p: Promise<void>) => {
    setBusy(true);
    withToast(label, p).then(reload).catch(() => {}).finally(() => setBusy(false));
  };
  const gib = (b: number) => `${(b / 1024 ** 3).toFixed(b >= 1024 ** 3 ? 0 : 1)}G`;
  const inputStyle = {
    flex: 1, minWidth: 0, padding: '6px 10px', background: t.surfaceAlt, color: t.fg1,
    border: `1px solid ${t.border}`, borderRadius: 4, fontFamily: t.mono, fontSize: 12, outline: 'none',
  };
  return (
    <div>
      {machines === null && <div style={{ fontFamily: t.mono, fontSize: 11, color: t.fg3 }}>Loading…</div>}
      {machines?.length === 0 && (
        <div style={{ fontFamily: t.mono, fontSize: 11, color: t.fg3, marginBottom: 8 }}>
          No machines. Containers run in per-container VMs until a machine exists.
        </div>
      )}
      {machines?.map(m => (
        <div key={m.id} style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '8px 0', borderBottom: `1px solid ${t.border}` }}>
          <span style={{ width: 8, height: 8, borderRadius: '50%', background: m.status === 'running' ? t.success : t.fg3 }} />
          <div style={{ flex: 1, minWidth: 0 }}>
            <div style={{ fontSize: 13, color: t.fg1, fontFamily: t.mono }}>
              {m.id}
              {m.isDefault && <span style={{ marginLeft: 8, fontSize: 10, color: t.accent, border: `1px solid ${t.accent}`, borderRadius: 999, padding: '1px 7px' }}>default</span>}
            </div>
            <div style={{ fontSize: 11, color: t.fg3, fontFamily: t.mono }}>
              {m.status} · {m.cpus} cpu · {gib(m.memory)} mem · {gib(m.diskSize)} disk
            </div>
          </div>
          <button style={pillBtn(t)} title="Show this machine's logs (container machine logs)"
                  onClick={() => setLogsFor(m.id)}>Logs</button>
          <button style={pillBtn(t)} title="Show this machine's configuration (container machine inspect)"
                  onClick={() => setInspectFor(m.id)}>Inspect</button>
          {!m.isDefault && (
            <button style={{ ...pillBtn(t), opacity: busy ? 0.5 : 1 }} disabled={busy}
                    onClick={() => act(`set default ${m.id}`, api.machineSetDefault(m.id))}>Set default</button>
          )}
          {m.status === 'running' && (
            <button style={{ ...pillBtn(t), opacity: busy ? 0.5 : 1 }} disabled={busy}
                    onClick={() => act(`stop ${m.id}`, api.machineStop(m.id))}>Stop</button>
          )}
          <button style={{ ...pillBtn(t, t.danger), opacity: busy ? 0.5 : 1 }} disabled={busy}
                  onClick={async () => {
                    if (!(await askConfirm(`Delete machine ${m.id}?\nIts disk image is removed.`, { title: 'Delete machine', okLabel: 'Delete' }))) return;
                    act(`delete ${m.id}`, api.machineDelete(m.id));
                  }}>Delete</button>
        </div>
      ))}
      {!showCreate ? (
        <button style={{ ...pillBtn(t), marginTop: 10 }} onClick={() => setShowCreate(true)}>New machine…</button>
      ) : (
        <div style={{ display: 'flex', flexDirection: 'column', gap: 8, marginTop: 10 }}>
          <div style={{ display: 'flex', gap: 8 }}>
            <input style={inputStyle} value={name} onChange={e => setName(e.target.value)} placeholder="machine name" spellCheck={false} />
            <input style={inputStyle} value={image} onChange={e => setImage(e.target.value)} placeholder="image (e.g. alpine:latest)" spellCheck={false} />
          </div>
          <button type="button" onClick={() => setShowUser(v => !v)} disabled={!userGate.ok} title={userGate.title}
                  style={{ background: 'none', border: 'none', cursor: userGate.ok ? 'pointer' : 'not-allowed', color: t.fg2, fontSize: 12, display: 'flex', alignItems: 'center', gap: 6, padding: 0, fontFamily: 'inherit', alignSelf: 'flex-start', opacity: userGate.ok ? 1 : 0.5 }}>
            <span style={{ display: 'inline-block', transform: showUser && userGate.ok ? 'rotate(90deg)' : 'none', transition: 'transform 120ms ease' }}>
              <Icon name="chevron" size={12} color={t.fg3} />
            </span>
            User options
          </button>
          {showUser && userGate.ok && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
              <div style={{ display: 'flex', gap: 8 }}>
                <input style={inputStyle} aria-label="Machine user" value={mUser} onChange={e => setMUser(e.target.value)} placeholder="user (name or uid[:gid])" spellCheck={false} aria-invalid={userError ? true : undefined} />
                <input style={{ ...inputStyle, flex: '0 0 80px' }} aria-label="Machine UID" value={mUid} onChange={e => setMUid(e.target.value.replace(/[^0-9]/g, ''))} placeholder="uid" />
                <input style={{ ...inputStyle, flex: '0 0 80px' }} aria-label="Machine GID" value={mGid} onChange={e => setMGid(e.target.value.replace(/[^0-9]/g, ''))} placeholder="gid" />
              </div>
              <input style={{ ...inputStyle, flex: 'none', borderColor: homeError ? t.danger : t.border }} aria-label="Machine home" value={mHome} onChange={e => setMHome(e.target.value)} placeholder="home directory (default /home/<user>)" spellCheck={false} aria-invalid={homeError ? true : undefined} />
              <div style={{ fontSize: 10, color: homeError || userError ? t.danger : t.fg3 }}>
                {homeError ?? userError ?? 'The machine account defaults to your macOS user, uid and gid.'}
              </div>
            </div>
          )}
          <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
            <input style={{ ...inputStyle, flex: '0 0 90px' }} value={cpus} onChange={e => setCpus(e.target.value.replace(/[^0-9]/g, ''))} placeholder="cpus" />
            <input style={{ ...inputStyle, flex: '0 0 90px' }} value={memory} onChange={e => setMemory(e.target.value)} placeholder="mem (4G)" />
            <div style={{ flex: 1 }} />
            <button style={pillBtn(t)} onClick={() => setShowCreate(false)}>Cancel</button>
            <button style={{ ...pillBtn(t), opacity: name.trim() && image.trim() && !homeError && !userError ? 1 : 0.5 }}
                    disabled={!name.trim() || !image.trim() || busy || !!homeError || !!userError}
                    onClick={() => {
                      act(`create ${name.trim()}`, api.machineCreate(name.trim(), image.trim(),
                        cpus.trim() ? Number(cpus.trim()) : undefined, memory.trim() || undefined, true,
                        showUser && userGate.ok && hasUserOpts ? userOpts : undefined));
                      setShowCreate(false); setName('');
                    }}>Create</button>
          </div>
          <div style={{ fontSize: 10, color: t.fg3 }}>
            Created without booting — it boots on first use.
          </div>
        </div>
      )}
      {logsFor && <MachineLogsModal t={t} name={logsFor} onClose={() => setLogsFor(null)} />}
      {inspectFor && (
        <JsonInspectModal t={t} title={inspectFor} subtitle="container machine inspect"
                          fetcher={() => api.machineInspect(inspectFor)} onClose={() => setInspectFor(null)} />
      )}
    </div>
  );
}
