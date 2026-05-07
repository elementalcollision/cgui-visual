/* cgui — modals: Detail, Pull, Trivy, Update, Doctor, Settings */

const { Icon, Bar } = window.cguiBase;
const { iconBtn, pillBtn } = window.cguiViews;

function Backdrop({ onClose, children }) {
  return (
    <div onClick={onClose} style={{ position: 'absolute', inset: 0, background: 'rgba(0,0,0,0.45)', display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 100, backdropFilter: 'blur(2px)' }}>
      <div onClick={e => e.stopPropagation()}>{children}</div>
    </div>
  );
}

function DetailModal({ item, t, variation, onClose }) {
  if (!item) return null;
  const isTerm = variation === 'terminal';
  const isEdit = variation === 'editorial';
  const c = item;
  return (
    <Backdrop onClose={onClose}>
      <div style={{ width: 720, maxHeight: '85vh', background: t.surface, border: `1px solid ${t.border}`, borderRadius: isTerm ? 0 : 12, overflow: 'hidden', display: 'flex', flexDirection: 'column', boxShadow: '0 24px 80px rgba(0,0,0,0.4)' }}>
        <div style={{ padding: '18px 22px', borderBottom: `1px solid ${t.border}`, display: 'flex', alignItems: 'flex-start', gap: 12 }}>
          <Icon name="box" size={20} color={t.accent} />
          <div style={{ flex: 1 }}>
            <div style={{ fontSize: isEdit ? 24 : 18, fontWeight: isEdit ? 400 : 600, fontFamily: isEdit ? 'var(--font-serif)' : 'inherit', color: t.fg1 }}>{c.name}</div>
            <div style={{ fontSize: 12, color: t.fg3, fontFamily: t.mono, marginTop: 4 }}>{c.id} · {c.image}</div>
          </div>
          <button onClick={onClose} style={iconBtn(t)}><Icon name="x" size={16} color={t.fg2} /></button>
        </div>
        <div style={{ padding: '14px 22px', display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: 14, borderBottom: `1px solid ${t.border}` }}>
          {[
            { l: 'Status', v: c.status, color: c.status === 'running' ? t.success : t.fg3 },
            { l: 'Uptime', v: c.uptime },
            { l: 'CPU', v: `${c.cpu.toFixed(1)}%`, color: c.cpu > 85 ? t.danger : c.cpu > 60 ? t.warning : t.fg1 },
            { l: 'Memory', v: `${c.mem.used.toFixed(1)} / ${c.mem.limit.toFixed(0)} GiB` },
          ].map((m, i) => (
            <div key={i}>
              <div style={{ fontSize: 10, color: t.fg3, fontWeight: 600, letterSpacing: '0.08em', textTransform: 'uppercase' }}>{m.l}</div>
              <div style={{ marginTop: 4, fontSize: isEdit ? 22 : 16, fontWeight: 500, fontFamily: isEdit ? 'var(--font-serif)' : 'inherit', color: m.color || t.fg1, textTransform: 'capitalize' }}>{m.v}</div>
            </div>
          ))}
        </div>
        <div style={{ padding: '14px 22px', borderBottom: `1px solid ${t.border}` }}>
          <div style={{ fontSize: 10, color: t.fg3, fontWeight: 600, letterSpacing: '0.08em', textTransform: 'uppercase', marginBottom: 6 }}>Command</div>
          <div style={{ fontFamily: t.mono, fontSize: 12, color: t.fg2, padding: '8px 10px', background: t.bg, border: `1px solid ${t.border}`, borderRadius: isTerm ? 0 : 6 }}>{c.cmd ? c.cmd.join(' ') : '—'}</div>
        </div>
        <div style={{ flex: 1, overflow: 'auto', padding: '14px 22px', minHeight: 200 }}>
          <div style={{ fontSize: 10, color: t.fg3, fontWeight: 600, letterSpacing: '0.08em', textTransform: 'uppercase', marginBottom: 8 }}>Inspect</div>
          <pre style={{ margin: 0, fontFamily: t.mono, fontSize: 11, lineHeight: 1.6, color: t.fg2, whiteSpace: 'pre-wrap', wordBreak: 'break-all' }}>{SAMPLE.inspectJson}</pre>
        </div>
        <div style={{ padding: '12px 22px', borderTop: `1px solid ${t.border}`, display: 'flex', gap: 8, justifyContent: 'flex-end', background: t.surfaceAlt }}>
          <button style={pillBtn(t)}>Logs</button>
          <button style={pillBtn(t)}>Exec /bin/sh</button>
          <button style={pillBtn(t, t.danger)}>Stop</button>
        </div>
      </div>
    </Backdrop>
  );
}

function PullModal({ t, variation, onClose }) {
  const [progress, setProgress] = React.useState(0);
  const [lines, setLines] = React.useState([SAMPLE.pullStream[0]]);
  React.useEffect(() => {
    let i = 1;
    const id = setInterval(() => {
      if (i >= SAMPLE.pullStream.length) { clearInterval(id); return; }
      setLines(L => [...L, SAMPLE.pullStream[i]]);
      const m = SAMPLE.pullStream[i].match(/(\d+)%/);
      if (m) setProgress(Number(m[1]));
      i++;
    }, 380);
    return () => clearInterval(id);
  }, []);
  const isTerm = variation === 'terminal';
  return (
    <Backdrop onClose={onClose}>
      <div style={{ width: 640, background: t.surface, border: `1px solid ${t.border}`, borderRadius: isTerm ? 0 : 12, overflow: 'hidden', display: 'flex', flexDirection: 'column', boxShadow: '0 24px 80px rgba(0,0,0,0.4)' }}>
        <div style={{ padding: '16px 22px', borderBottom: `1px solid ${t.border}`, display: 'flex', alignItems: 'center', gap: 12 }}>
          <Icon name="download" size={18} color={t.accent} />
          <div style={{ flex: 1 }}>
            <div style={{ fontSize: 15, fontWeight: 600, color: t.fg1 }}>Pulling image</div>
            <div style={{ fontSize: 12, color: t.fg3, fontFamily: t.mono, marginTop: 2 }}>mlcommons/inference:llama2-70b</div>
          </div>
          <button onClick={onClose} style={iconBtn(t)}><Icon name="x" size={16} color={t.fg2} /></button>
        </div>
        <div style={{ padding: '14px 22px', borderBottom: `1px solid ${t.border}` }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 6, fontFamily: t.mono, fontSize: 12, color: t.fg2 }}>
            <span>Layer 3 of 24</span><span>{progress}%</span>
          </div>
          <Bar pct={progress} color={t.accent} bg={t.surfaceAlt} h={8} />
        </div>
        <div style={{ flex: 1, overflow: 'auto', padding: 16, fontFamily: t.mono, fontSize: 11, lineHeight: 1.6, background: t.bg, maxHeight: 280 }}>
          {lines.map((l, i) => (
            <div key={i} style={{ color: l.includes('✓') ? t.success : l.includes('▸') ? t.fg2 : t.fg1, padding: '1px 0' }}>{l}</div>
          ))}
        </div>
        <div style={{ padding: '12px 22px', borderTop: `1px solid ${t.border}`, display: 'flex', justifyContent: 'space-between', alignItems: 'center', background: t.surfaceAlt }}>
          <span style={{ fontSize: 11, color: t.fg3, fontFamily: t.mono }}>Esc to background — pull continues</span>
          <button onClick={onClose} style={pillBtn(t)}>Background</button>
        </div>
      </div>
    </Backdrop>
  );
}

function TrivyModal({ t, variation, onClose }) {
  const [filter, setFilter] = React.useState(null);
  const [search, setSearch] = React.useState('');
  const isTerm = variation === 'terminal';
  const sevColor = { CRITICAL: t.danger, HIGH: '#E5704A', MEDIUM: t.warning, LOW: t.fg3 };
  const findings = SAMPLE.trivy.findings.filter(f =>
    (!filter || f.sev === filter) &&
    (!search || (f.cve + f.pkg + f.title).toLowerCase().includes(search.toLowerCase()))
  );
  return (
    <Backdrop onClose={onClose}>
      <div style={{ width: 800, maxHeight: '85vh', background: t.surface, border: `1px solid ${t.border}`, borderRadius: isTerm ? 0 : 12, overflow: 'hidden', display: 'flex', flexDirection: 'column' }}>
        <div style={{ padding: '16px 22px', borderBottom: `1px solid ${t.border}`, display: 'flex', alignItems: 'center', gap: 12 }}>
          <Icon name="shield" size={18} color={t.warning} />
          <div style={{ flex: 1 }}>
            <div style={{ fontSize: 15, fontWeight: 600, color: t.fg1 }}>Trivy scan</div>
            <div style={{ fontSize: 12, color: t.fg3, fontFamily: t.mono }}>{SAMPLE.trivy.image}</div>
          </div>
          <button onClick={onClose} style={iconBtn(t)}><Icon name="x" size={16} color={t.fg2} /></button>
        </div>
        <div style={{ padding: '14px 22px', display: 'flex', gap: 8, borderBottom: `1px solid ${t.border}` }}>
          {Object.entries(SAMPLE.trivy.counts).map(([sev, n]) => (
            <button key={sev} onClick={() => setFilter(filter === sev ? null : sev)} style={{
              padding: '6px 12px', background: filter === sev ? sevColor[sev] : t.surfaceAlt, color: filter === sev ? '#fff' : sevColor[sev],
              border: `1px solid ${filter === sev ? sevColor[sev] : t.border}`, borderRadius: isTerm ? 0 : 6, fontSize: 11, fontWeight: 600, fontFamily: t.mono, cursor: 'pointer', letterSpacing: '0.05em',
            }}>{sev} {n}</button>
          ))}
          <div style={{ flex: 1 }} />
          <input placeholder="Search CVE / package…" value={search} onChange={e => setSearch(e.target.value)}
            style={{ padding: '6px 10px', background: t.surfaceAlt, border: `1px solid ${t.border}`, borderRadius: isTerm ? 0 : 6, color: t.fg1, fontSize: 12, fontFamily: t.mono, width: 200, outline: 'none' }} />
        </div>
        <div style={{ flex: 1, overflow: 'auto' }}>
          {findings.map((f, i) => (
            <div key={i} style={{ padding: '12px 22px', borderBottom: `1px solid ${t.border}`, display: 'grid', gridTemplateColumns: '80px 140px 1fr 110px', gap: 12, alignItems: 'center' }}>
              <span style={{ padding: '2px 8px', borderRadius: 999, background: sevColor[f.sev], color: '#fff', fontSize: 10, fontWeight: 700, fontFamily: t.mono, textAlign: 'center', letterSpacing: '0.04em' }}>{f.sev}</span>
              <span style={{ fontFamily: t.mono, fontSize: 12, color: t.fg1 }}>{f.cve}</span>
              <div>
                <div style={{ fontSize: 13, color: t.fg1 }}>{f.title}</div>
                <div style={{ fontSize: 11, color: t.fg3, fontFamily: t.mono, marginTop: 2 }}>{f.pkg} · {f.installed} → {f.fixed}</div>
              </div>
              <button style={pillBtn(t)}>Upgrade</button>
            </div>
          ))}
        </div>
      </div>
    </Backdrop>
  );
}

function UpdateModal({ t, variation, onClose }) {
  const [idx, setIdx] = React.useState(0);
  const u = SAMPLE.updates[idx];
  const isTerm = variation === 'terminal';
  return (
    <Backdrop onClose={onClose}>
      <div style={{ width: 640, maxHeight: '85vh', background: t.surface, border: `1px solid ${t.border}`, borderRadius: isTerm ? 0 : 12, overflow: 'hidden', display: 'flex', flexDirection: 'column' }}>
        <div style={{ padding: '16px 22px', borderBottom: `1px solid ${t.border}`, display: 'flex', alignItems: 'center', gap: 12 }}>
          <Icon name="download" size={18} color={t.warning} />
          <div style={{ flex: 1 }}>
            <div style={{ fontSize: 15, fontWeight: 600, color: t.fg1 }}>Update available</div>
            <div style={{ fontSize: 12, color: t.fg3, fontFamily: t.mono }}>{u.component} {u.installed} → {u.latest} · {u.published}</div>
          </div>
          <div style={{ display: 'flex', gap: 4 }}>
            {SAMPLE.updates.map((_, i) => (
              <button key={i} onClick={() => setIdx(i)} style={{ width: 8, height: 8, borderRadius: '50%', background: i === idx ? t.accent : t.border, border: 'none', cursor: 'pointer' }} />
            ))}
          </div>
          <button onClick={onClose} style={iconBtn(t)}><Icon name="x" size={16} color={t.fg2} /></button>
        </div>
        <div style={{ flex: 1, overflow: 'auto', padding: '16px 22px', maxHeight: 360 }}>
          <pre style={{ margin: 0, fontFamily: t.mono, fontSize: 12, lineHeight: 1.6, color: t.fg2, whiteSpace: 'pre-wrap' }}>{u.notes}</pre>
        </div>
        <div style={{ padding: '12px 22px', borderTop: `1px solid ${t.border}`, display: 'flex', gap: 8, background: t.surfaceAlt }}>
          <button style={{ ...pillBtn(t), padding: '6px 14px' }}>[O] Open release</button>
          <button style={{ ...pillBtn(t), padding: '6px 14px' }}>[L] Later</button>
          <div style={{ flex: 1 }} />
          <button style={{ padding: '6px 14px', background: t.fg1, color: t.bg, border: 'none', borderRadius: isTerm ? 0 : 6, fontSize: 12, fontWeight: 500, cursor: 'pointer' }}>[I] Install</button>
        </div>
      </div>
    </Backdrop>
  );
}

function DoctorModal({ t, variation, onClose }) {
  const isTerm = variation === 'terminal';
  return (
    <Backdrop onClose={onClose}>
      <div style={{ width: 600, background: t.surface, border: `1px solid ${t.border}`, borderRadius: isTerm ? 0 : 12, overflow: 'hidden' }}>
        <div style={{ padding: '16px 22px', borderBottom: `1px solid ${t.border}`, display: 'flex', alignItems: 'center', gap: 12 }}>
          <Icon name="heart" size={18} color={t.success} />
          <div style={{ flex: 1, fontSize: 15, fontWeight: 600, color: t.fg1 }}>cgui doctor</div>
          <button onClick={onClose} style={iconBtn(t)}><Icon name="x" size={16} color={t.fg2} /></button>
        </div>
        <div style={{ padding: '14px 22px', fontFamily: t.mono, fontSize: 12 }}>
          {SAMPLE.doctor.map((d, i) => (
            <div key={i} style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '6px 0', color: d.ok ? t.fg1 : d.warn ? t.warning : t.danger }}>
              <span style={{ width: 16, color: d.ok ? t.success : t.warning, fontWeight: 700 }}>{d.ok ? '✓' : '!'}</span>
              <span>{d.text}</span>
            </div>
          ))}
        </div>
        <div style={{ padding: '12px 22px', borderTop: `1px solid ${t.border}`, background: t.surfaceAlt, fontFamily: t.mono, fontSize: 11, color: t.fg3 }}>
          == 6 passed · 2 warnings · 0 failures ==
        </div>
      </div>
    </Backdrop>
  );
}

function SettingsModal({ t, variation, onClose, runtime, setRuntime }) {
  const isTerm = variation === 'terminal';
  return (
    <Backdrop onClose={onClose}>
      <div style={{ width: 640, background: t.surface, border: `1px solid ${t.border}`, borderRadius: isTerm ? 0 : 12, overflow: 'hidden', maxHeight: '85vh', display: 'flex', flexDirection: 'column' }}>
        <div style={{ padding: '16px 22px', borderBottom: `1px solid ${t.border}`, display: 'flex', alignItems: 'center', gap: 12 }}>
          <Icon name="cog" size={18} color={t.fg2} />
          <div style={{ flex: 1, fontSize: 15, fontWeight: 600, color: t.fg1 }}>Settings</div>
          <button onClick={onClose} style={iconBtn(t)}><Icon name="x" size={16} color={t.fg2} /></button>
        </div>
        <div style={{ flex: 1, overflow: 'auto', padding: '18px 22px' }}>
          <div style={{ fontSize: 11, fontWeight: 600, letterSpacing: '0.08em', textTransform: 'uppercase', color: t.fg3, marginBottom: 10 }}>Runtime profile</div>
          {['container', 'docker', 'podman'].map(r => (
            <label key={r} style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '10px 12px', borderRadius: isTerm ? 0 : 6, background: r === runtime ? t.selected : 'transparent', border: `1px solid ${r === runtime ? t.accent : t.border}`, marginBottom: 6, cursor: 'pointer' }}>
              <input type="radio" checked={r === runtime} onChange={() => setRuntime(r)} style={{ accentColor: t.accent }} />
              <div style={{ flex: 1 }}>
                <div style={{ fontSize: 13, color: t.fg1, fontWeight: 500, fontFamily: t.mono }}>{r}</div>
                <div style={{ fontSize: 11, color: t.fg3, fontFamily: t.mono }}>{r === 'container' ? '/usr/local/bin/container' : r === 'docker' ? '/usr/local/bin/docker' : '/opt/homebrew/bin/podman'}</div>
              </div>
              {r === runtime && <span style={{ fontSize: 10, color: t.success, fontFamily: t.mono }}>● ACTIVE</span>}
            </label>
          ))}
          <div style={{ fontSize: 11, fontWeight: 600, letterSpacing: '0.08em', textTransform: 'uppercase', color: t.fg3, margin: '20px 0 10px' }}>Resource alerts</div>
          {[
            { l: 'CPU warn', v: '60%' }, { l: 'CPU alert', v: '85%' },
            { l: 'Memory warn', v: '70%' }, { l: 'Memory alert', v: '90%' },
          ].map((row, i) => (
            <div key={i} style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '8px 0', borderBottom: `1px solid ${t.border}` }}>
              <span style={{ fontSize: 13, color: t.fg2 }}>{row.l}</span>
              <span style={{ fontSize: 12, color: t.fg1, fontFamily: t.mono, padding: '4px 10px', background: t.surfaceAlt, border: `1px solid ${t.border}`, borderRadius: isTerm ? 0 : 4 }}>{row.v}</span>
            </div>
          ))}
          <div style={{ fontSize: 11, fontWeight: 600, letterSpacing: '0.08em', textTransform: 'uppercase', color: t.fg3, margin: '20px 0 10px' }}>State</div>
          <div style={{ fontFamily: t.mono, fontSize: 11, color: t.fg3 }}>~/.config/cgui/state.json — auto-saved</div>
        </div>
      </div>
    </Backdrop>
  );
}

window.cguiModals = { DetailModal, PullModal, TrivyModal, UpdateModal, DoctorModal, SettingsModal };
