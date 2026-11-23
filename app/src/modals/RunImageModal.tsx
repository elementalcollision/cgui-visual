import { useState } from 'react';
import { api } from '../api';
import { useCliAtLeast } from '../cliVersion';
import { Icon, iconBtn, pillBtn } from '../components';
import type { ThemeTokens } from '../theme';
import { withToast } from '../toast';
import { splitList, validateAlias } from '../validate';
import { Backdrop } from './shared';

export function RunImageModal({ t, image, onLaunched, onClose }: {
  t: ThemeTokens; image: string;
  onLaunched: (id: string) => void; onClose: () => void;
}) {
  const [name, setName] = useState('');
  const [ports, setPorts] = useState('');
  const [env, setEnv] = useState('');
  const [command, setCommand] = useState('');
  const [busy, setBusy] = useState(false);
  // Advanced block (phase 4). Collapsed by default — the simple path
  // stays simple; power users disclose resources/mounts/identity.
  const [advanced, setAdvanced] = useState(false);
  const [cpus, setCpus] = useState('');
  const [memory, setMemory] = useState('');
  const [volumes, setVolumes] = useState('');
  const [labels, setLabels] = useState('');
  const [network, setNetwork] = useState('');
  // DNS aliases (container 1.6+): `--network <net>,alias=NAME,...`.
  const aliasGate = useCliAtLeast('1.6.0');
  const [aliases, setAliases] = useState('');
  const aliasList = splitList(aliases);
  const aliasError = aliasList.map(validateAlias).find(Boolean) ?? null;
  const [workdir, setWorkdir] = useState('');
  const [user, setUser] = useState('');
  const [platform, setPlatform] = useState('');
  const [rm, setRm] = useState(false);
  const [rosetta, setRosetta] = useState(false);

  const submit = () => {
    if (aliasError) return;
    setBusy(true);
    // `withToast` surfaces backend errors (image-pull failure, port
    // already in use, container_system not running, etc.) — previously
    // these were silently swallowed and the user just saw the modal
    // dim and recover with no feedback.
    withToast(
      `run ${image}`,
      api.runImage({
        image,
        name: name.trim() || undefined,
        ports: ports.split('\n').map(l => l.trim()).filter(Boolean),
        env: env.split('\n').map(l => l.trim()).filter(Boolean),
        command: command.trim() || undefined,
        cpus: cpus.trim() ? Number(cpus.trim()) : undefined,
        memory: memory.trim() || undefined,
        volumes: volumes.split('\n').map(l => l.trim()).filter(Boolean),
        labels: labels.split('\n').map(l => l.trim()).filter(Boolean),
        network: network.trim() || undefined,
        aliases: aliasList.length ? aliasList : undefined,
        workdir: workdir.trim() || undefined,
        user: user.trim() || undefined,
        platform: platform.trim() || undefined,
        rm,
        rosetta,
      }),
    )
      .then(id => {
        onLaunched(id);
        onClose();
      })
      .catch(() => {
        // Error already toasted by withToast. Drop the busy flag so
        // the user can edit the form and retry without re-opening.
        setBusy(false);
      });
  };

  const inputStyle = {
    width: '100%', padding: '8px 10px', background: t.bg, border: `1px solid ${t.border}`,
    borderRadius: 6, color: t.fg1, fontSize: 12, fontFamily: t.mono, outline: 'none',
    boxSizing: 'border-box' as const,
  };
  const labelStyle = { fontSize: 11, fontWeight: 600, letterSpacing: '0.08em', textTransform: 'uppercase' as const, color: t.fg3, marginBottom: 6, display: 'block' };

  return (
    <Backdrop title="Run image" onClose={onClose}>
      <div style={{ width: 560, background: t.surface, border: `1px solid ${t.border}`, borderRadius: 12, overflow: 'hidden', display: 'flex', flexDirection: 'column', boxShadow: '0 24px 80px rgba(0,0,0,0.4)' }}>
        <div style={{ padding: '18px 22px', borderBottom: `1px solid ${t.border}`, display: 'flex', alignItems: 'flex-start', gap: 12 }}>
          <Icon name="play" size={20} color={t.success} />
          <div style={{ flex: 1 }}>
            <div style={{ fontSize: 18, fontWeight: 600, color: t.fg1 }}>Run image</div>
            <div style={{ fontSize: 12, color: t.fg3, fontFamily: t.mono, marginTop: 4 }}>{image}</div>
          </div>
          <button onClick={onClose} style={iconBtn()} aria-label="Close"><Icon name="x" size={16} color={t.fg2} /></button>
        </div>
        <div style={{ padding: '18px 22px', display: 'flex', flexDirection: 'column', gap: 14, maxHeight: '62vh', overflow: 'auto' }}>
          <div>
            <label style={labelStyle}>Name <span style={{ color: t.fg3, fontWeight: 400, textTransform: 'none', letterSpacing: 0 }}>(optional)</span></label>
            <input style={inputStyle} value={name} onChange={e => setName(e.target.value)} placeholder="myservice" />
          </div>
          <div>
            <label style={labelStyle}>Port mappings <span style={{ color: t.fg3, fontWeight: 400, textTransform: 'none', letterSpacing: 0 }}>(host:container, one per line)</span></label>
            <textarea style={{ ...inputStyle, minHeight: 60, resize: 'vertical' }} value={ports} onChange={e => setPorts(e.target.value)} placeholder="8080:80&#10;5432:5432" />
          </div>
          <div>
            <label style={labelStyle}>Environment <span style={{ color: t.fg3, fontWeight: 400, textTransform: 'none', letterSpacing: 0 }}>(KEY=value, one per line)</span></label>
            <textarea style={{ ...inputStyle, minHeight: 60, resize: 'vertical' }} value={env} onChange={e => setEnv(e.target.value)} placeholder="NODE_ENV=production&#10;DEBUG=true" />
          </div>
          <div>
            <label style={labelStyle}>Override command <span style={{ color: t.fg3, fontWeight: 400, textTransform: 'none', letterSpacing: 0 }}>(optional)</span></label>
            <input style={inputStyle} value={command} onChange={e => setCommand(e.target.value)} placeholder="sh -c 'echo hello'" />
          </div>

          <button onClick={() => setAdvanced(a => !a)}
            style={{ background: 'none', border: 'none', cursor: 'pointer', color: t.fg2, fontSize: 12, display: 'flex', alignItems: 'center', gap: 6, padding: 0, fontFamily: 'inherit' }}>
            <span style={{ display: 'inline-block', transform: advanced ? 'rotate(90deg)' : 'none', transition: 'transform 120ms ease' }}>
              <Icon name="chevron" size={12} color={t.fg3} />
            </span>
            Advanced options
          </button>
          {advanced && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
              <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12 }}>
                <div>
                  <label style={labelStyle}>CPUs</label>
                  <input style={inputStyle} value={cpus} onChange={e => setCpus(e.target.value.replace(/[^0-9]/g, ''))} placeholder="4" />
                </div>
                <div>
                  <label style={labelStyle}>Memory</label>
                  <input style={inputStyle} value={memory} onChange={e => setMemory(e.target.value)} placeholder="2G" />
                </div>
                <div>
                  <label style={labelStyle}>Network</label>
                  <input style={inputStyle} value={network} onChange={e => setNetwork(e.target.value)} placeholder="default" />
                </div>
                <div>
                  <label style={labelStyle} htmlFor="run-aliases">DNS aliases</label>
                  <input id="run-aliases" style={{ ...inputStyle, opacity: aliasGate.ok ? 1 : 0.5, borderColor: aliasError ? t.danger : t.border }}
                         value={aliases} onChange={e => setAliases(e.target.value)}
                         disabled={!aliasGate.ok} title={aliasGate.title}
                         aria-invalid={aliasError ? true : undefined}
                         placeholder="api.example.test, db.example.test" spellCheck={false} />
                  <div style={{ fontSize: 10, color: aliasError ? t.danger : t.fg3, marginTop: 4, lineHeight: 1.4 }}>
                    {aliasError ?? 'Extra names for this container on the network (comma separated). Use fully qualified names unless a default DNS domain is set; uses the default network when none is chosen.'}
                  </div>
                </div>
                <div>
                  <label style={labelStyle}>Platform</label>
                  <input style={inputStyle} value={platform} onChange={e => setPlatform(e.target.value)} placeholder="linux/arm64" />
                </div>
                <div>
                  <label style={labelStyle}>Working dir</label>
                  <input style={inputStyle} value={workdir} onChange={e => setWorkdir(e.target.value)} placeholder="/srv" />
                </div>
                <div>
                  <label style={labelStyle}>User</label>
                  <input style={inputStyle} value={user} onChange={e => setUser(e.target.value)} placeholder="name or uid[:gid]" />
                </div>
              </div>
              <div>
                <label style={labelStyle}>Volumes <span style={{ color: t.fg3, fontWeight: 400, textTransform: 'none', letterSpacing: 0 }}>(volume-or-path:containerpath, one per line)</span></label>
                <textarea style={{ ...inputStyle, minHeight: 48, resize: 'vertical' }} value={volumes} onChange={e => setVolumes(e.target.value)} placeholder="mydata:/data" />
              </div>
              <div>
                <label style={labelStyle}>Labels <span style={{ color: t.fg3, fontWeight: 400, textTransform: 'none', letterSpacing: 0 }}>(key=value, one per line)</span></label>
                <textarea style={{ ...inputStyle, minHeight: 48, resize: 'vertical' }} value={labels} onChange={e => setLabels(e.target.value)} placeholder="team=infra" />
              </div>
              <div style={{ display: 'flex', gap: 18 }}>
                <label style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 13, color: t.fg1, cursor: 'pointer' }}>
                  <input type="checkbox" checked={rm} onChange={e => setRm(e.target.checked)} style={{ accentColor: t.accent }} />
                  Remove after exit (--rm)
                </label>
                <label style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 13, color: t.fg1, cursor: 'pointer' }}>
                  <input type="checkbox" checked={rosetta} onChange={e => setRosetta(e.target.checked)} style={{ accentColor: t.accent }} />
                  Rosetta (x86 images)
                </label>
              </div>
            </div>
          )}
        </div>
        <div style={{ padding: '12px 22px', borderTop: `1px solid ${t.border}`, display: 'flex', gap: 8, justifyContent: 'flex-end', background: t.surfaceAlt }}>
          <button style={pillBtn(t)} onClick={onClose} disabled={busy}>Cancel</button>
          <button style={{ padding: '6px 14px', background: t.fg1, color: t.bg, border: 'none', borderRadius: 6, fontSize: 12, fontWeight: 500, cursor: busy ? 'wait' : 'pointer', opacity: busy ? 0.6 : 1 }}
                  onClick={submit} disabled={busy || !!aliasError}>{busy ? 'Starting…' : 'Run'}</button>
        </div>
      </div>
    </Backdrop>
  );
}
