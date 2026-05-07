/* cgui — table views (Containers, Images, Volumes, Networks, Stacks, Logs) */

const { Sparkline, Bar, StatusDot, Icon, fmtBytes } = window.cguiBase;
const { useState: useS2, useMemo: useM2 } = React;

function ContainersView({ t, variation, search, selected, setSelected, onInspect, onLogs }) {
  const isTerm = variation === 'terminal';
  const isEdit = variation === 'editorial';
  const rows = useM2(() => SAMPLE.containers.filter(c =>
    !search || c.name.includes(search.toLowerCase()) || c.image.includes(search.toLowerCase()) || c.id.includes(search.toLowerCase())
  ), [search]);

  // Group by stack for editorial / workbench; flat for terminal
  const grouped = useM2(() => {
    if (isTerm) return [{ stack: null, items: rows }];
    const m = new Map();
    rows.forEach(c => {
      const k = c.stack || '__solo';
      if (!m.has(k)) m.set(k, []);
      m.get(k).push(c);
    });
    return [...m.entries()].map(([k, items]) => ({ stack: k === '__solo' ? null : k, items }));
  }, [rows, isTerm]);

  const headerStyle = {
    display: 'grid',
    gridTemplateColumns: isTerm ? '20px 22px 1fr 1.4fr 70px 70px 80px 100px 80px' : '24px 1fr 1.2fr 80px 110px 90px 110px 100px',
    gap: 12, padding: isTerm ? '6px 16px' : '10px 20px', alignItems: 'center',
    fontSize: 11, color: t.fg3, fontWeight: 600, letterSpacing: isTerm ? 0 : '0.06em', textTransform: isTerm ? 'none' : 'uppercase',
    fontFamily: isTerm ? t.mono : 'inherit',
    borderBottom: `1px solid ${t.border}`, background: isTerm ? t.bg : t.surface,
    position: 'sticky', top: 0, zIndex: 1,
  };

  return (
    <div style={{ flex: 1, overflow: 'auto', background: t.bg }}>
      {/* Aggregate metrics */}
      {!isTerm && (
        <div style={{ padding: isEdit ? '24px 24px 8px' : '16px 20px 8px', display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: 16 }}>
          {[
            { l: 'CPU', v: '34.6', u: '%', spark: [12, 18, 22, 28, 35, 42, 38, 35, 32, 38, 41, 44, 48, 52, 48, 42, 38, 35, 34, 35, 36, 35, 34, 35] },
            { l: 'Memory', v: '111.3', u: 'GiB', spark: [62, 65, 68, 70, 72, 75, 78, 76, 74, 72, 70, 71, 72, 71, 70, 69, 68, 69, 70, 71, 70, 69, 68, 70] },
            { l: 'Network I/O', v: '142.4', u: 'MB/s', spark: [22, 28, 35, 40, 38, 32, 28, 35, 42, 48, 55, 62, 58, 52, 48, 45, 50, 55, 60, 65, 62, 58, 55, 60] },
            { l: 'Disk I/O', v: '38.2', u: 'MB/s', spark: [8, 10, 12, 15, 18, 22, 18, 15, 12, 14, 16, 18, 20, 18, 15, 14, 16, 18, 20, 22, 20, 18, 16, 18] },
          ].map((m, i) => (
            <div key={i} style={{ background: t.surface, border: `1px solid ${t.border}`, borderRadius: isTerm ? 0 : 8, padding: '14px 16px' }}>
              <div style={{ fontSize: 11, color: t.fg3, fontWeight: 500, letterSpacing: '0.04em', textTransform: 'uppercase' }}>{m.l}</div>
              <div style={{ display: 'flex', alignItems: 'baseline', gap: 6, marginTop: 4 }}>
                <span style={{ fontSize: isEdit ? 32 : 22, fontWeight: isEdit ? 400 : 600, fontFamily: isEdit ? 'var(--font-serif)' : 'inherit', color: t.fg1, fontVariantNumeric: 'tabular-nums', letterSpacing: '-0.01em' }}>{m.v}</span>
                <span style={{ fontSize: 12, color: t.fg3, fontFamily: t.mono }}>{m.u}</span>
              </div>
              <div style={{ marginTop: 6 }}>
                <Sparkline data={m.spark} w={200} h={26} color={t.sparkline} max={Math.max(...m.spark) * 1.1} />
              </div>
            </div>
          ))}
        </div>
      )}

      <div style={headerStyle}>
        {isTerm && <span>●</span>}
        <span>{isTerm ? '' : ''}</span>
        <span>{isTerm ? 'NAME' : 'Name'}</span>
        <span>{isTerm ? 'IMAGE' : 'Image'}</span>
        <span>{isTerm ? 'STATUS' : 'Status'}</span>
        <span style={{ textAlign: 'right' }}>{isTerm ? 'CPU%' : 'CPU'}</span>
        <span style={{ textAlign: 'right' }}>{isTerm ? 'MEM' : 'Memory'}</span>
        <span>{isTerm ? 'PORTS' : 'Ports'}</span>
        <span style={{ textAlign: 'right' }}>{isTerm ? 'UPTIME' : 'Uptime'}</span>
      </div>

      {grouped.map((g, gi) => (
        <React.Fragment key={gi}>
          {g.stack && !isTerm && (
            <div style={{ padding: '10px 20px 4px', display: 'flex', alignItems: 'center', gap: 8, fontSize: 11, color: t.fg3, fontWeight: 600, letterSpacing: '0.06em', textTransform: 'uppercase', background: t.bg }}>
              <Icon name="layers" size={11} color={t.fg3} />
              <span>{g.stack}</span>
              <span style={{ flex: 1, height: 1, background: t.border, marginLeft: 4 }} />
            </div>
          )}
          {g.items.map(c => {
            const sel = selected === c.id;
            return (
              <div key={c.id} onClick={() => setSelected(c.id)}
                style={{
                  display: 'grid',
                  gridTemplateColumns: isTerm ? '20px 22px 1fr 1.4fr 70px 70px 80px 100px 80px' : '24px 1fr 1.2fr 80px 110px 90px 110px 100px',
                  gap: 12, padding: isTerm ? '4px 16px' : '12px 20px', alignItems: 'center',
                  background: sel ? t.selected : 'transparent',
                  borderLeft: sel ? `2px solid ${t.accent}` : '2px solid transparent',
                  borderBottom: `1px solid ${t.border}`,
                  cursor: 'pointer', fontSize: 13, fontFamily: isTerm ? t.mono : 'inherit',
                  color: t.fg1,
                }}
                onMouseEnter={e => { if (!sel) e.currentTarget.style.background = t.hover; }}
                onMouseLeave={e => { if (!sel) e.currentTarget.style.background = 'transparent'; }}
              >
                {isTerm && <span style={{ color: c.status === 'running' ? t.success : c.status === 'paused' ? t.warning : t.fg3 }}>●</span>}
                <Icon name="box" size={isTerm ? 12 : 16} color={t.fg3} />
                <div style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                  <div style={{ fontWeight: 500, color: t.fg1 }}>{c.name}</div>
                  {!isTerm && <div style={{ fontSize: 11, color: t.fg3, fontFamily: t.mono, marginTop: 1 }}>{c.id}</div>}
                </div>
                <div style={{ fontFamily: t.mono, fontSize: 12, color: t.fg2, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{c.image}</div>
                <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                  {!isTerm && <StatusDot status={c.status} t={t} />}
                  <span style={{ fontSize: 12, color: c.status === 'running' ? t.success : c.status === 'paused' ? t.warning : t.fg3, textTransform: 'capitalize' }}>{c.status}</span>
                </div>
                <div style={{ display: 'flex', alignItems: 'center', gap: 6, justifyContent: 'flex-end' }}>
                  {!isTerm && <Sparkline data={c.cpuHistory.slice(-16)} w={40} h={14} color={c.cpu > 85 ? t.danger : c.cpu > 60 ? t.warning : t.sparkline} />}
                  <span style={{ fontFamily: t.mono, fontSize: 12, color: c.cpu > 85 ? t.danger : c.cpu > 60 ? t.warning : t.fg2, fontVariantNumeric: 'tabular-nums', minWidth: 38, textAlign: 'right' }}>{c.cpu.toFixed(1)}%</span>
                </div>
                <div style={{ textAlign: 'right' }}>
                  <div style={{ fontFamily: t.mono, fontSize: 11, color: t.fg2, fontVariantNumeric: 'tabular-nums' }}>{c.mem.used.toFixed(1)}/{c.mem.limit.toFixed(0)}</div>
                  {!isTerm && <Bar pct={c.mem.pct} color={c.mem.pct > 85 ? t.danger : c.mem.pct > 70 ? t.warning : t.success} bg={t.surfaceAlt} h={3} />}
                </div>
                <div style={{ fontFamily: t.mono, fontSize: 11, color: t.fg3, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{c.ports.join(', ') || '—'}</div>
                <div style={{ display: 'flex', alignItems: 'center', gap: 4, justifyContent: 'flex-end' }}>
                  <span style={{ fontFamily: t.mono, fontSize: 11, color: t.fg3, marginRight: 8 }}>{c.uptime}</span>
                  <button onClick={e => { e.stopPropagation(); onInspect(c); }} style={iconBtn(t)}><Icon name="info" size={13} color={t.fg2} /></button>
                  <button onClick={e => { e.stopPropagation(); onLogs(c); }} style={iconBtn(t)}><Icon name="logs" size={13} color={t.fg2} /></button>
                  {c.status === 'running'
                    ? <button style={iconBtn(t)}><Icon name="stop" size={13} color={t.fg2} /></button>
                    : <button style={iconBtn(t)}><Icon name="play" size={13} color={t.success} /></button>}
                </div>
              </div>
            );
          })}
        </React.Fragment>
      ))}
    </div>
  );
}

function ImagesView({ t, variation, search, onScan }) {
  const isTerm = variation === 'terminal';
  const isEdit = variation === 'editorial';
  const rows = SAMPLE.images.filter(i => !search || i.ref.toLowerCase().includes(search.toLowerCase()));
  return (
    <div style={{ flex: 1, overflow: 'auto', background: t.bg }}>
      <div style={tableHeader(t, isTerm, '32px 1fr 100px 110px 100px 140px')}>
        <span></span><span>{isTerm ? 'REFERENCE' : 'Reference'}</span>
        <span style={{ textAlign: 'right' }}>{isTerm ? 'SIZE' : 'Size'}</span>
        <span style={{ textAlign: 'right' }}>{isTerm ? 'LAYERS' : 'Layers'}</span>
        <span>{isTerm ? 'CREATED' : 'Created'}</span><span style={{ textAlign: 'right' }}>Actions</span>
      </div>
      {rows.map((img, i) => (
        <div key={img.id} style={tableRow(t, isTerm, '32px 1fr 100px 110px 100px 140px')}>
          <Icon name="image" size={16} color={t.fg3} />
          <div>
            <div style={{ fontFamily: t.mono, fontSize: 13, color: t.fg1 }}>{img.ref}</div>
            <div style={{ fontFamily: t.mono, fontSize: 11, color: t.fg3, marginTop: 2 }}>{img.digest}</div>
          </div>
          <div style={{ textAlign: 'right', fontFamily: t.mono, fontSize: 12, color: t.fg1, fontVariantNumeric: 'tabular-nums' }}>
            {isEdit ? <span style={{ fontFamily: 'var(--font-serif)', fontSize: 18 }}>{img.size.toFixed(img.size < 1 ? 2 : 1)}</span> : img.size.toFixed(img.size < 1 ? 2 : 1)} <span style={{ color: t.fg3, fontSize: 11 }}>{img.sizeUnit}</span>
          </div>
          <div style={{ textAlign: 'right', fontFamily: t.mono, fontSize: 12, color: t.fg2 }}>{img.layers}</div>
          <div style={{ fontFamily: t.mono, fontSize: 12, color: t.fg3 }}>{img.created}</div>
          <div style={{ display: 'flex', gap: 4, justifyContent: 'flex-end' }}>
            <button onClick={() => onScan?.(img)} style={iconBtn(t)} title="Trivy scan"><Icon name="shield" size={13} color={t.fg2} /></button>
            <button style={iconBtn(t)} title="Run"><Icon name="play" size={13} color={t.fg2} /></button>
            <button style={iconBtn(t)} title="Delete"><Icon name="trash" size={13} color={t.fg2} /></button>
          </div>
        </div>
      ))}
    </div>
  );
}

function VolumesView({ t, variation }) {
  const isTerm = variation === 'terminal';
  const isEdit = variation === 'editorial';
  return (
    <div style={{ flex: 1, overflow: 'auto', background: t.bg, padding: isEdit ? 24 : isTerm ? 0 : 16 }}>
      <div style={{ display: 'grid', gridTemplateColumns: isTerm ? '1fr' : 'repeat(auto-fill, minmax(380px, 1fr))', gap: isTerm ? 0 : 14 }}>
        {SAMPLE.volumes.map(v => {
          const pct = (v.used / v.size) * 100;
          return (
            <div key={v.name} style={{ background: t.surface, border: `1px solid ${t.border}`, borderRadius: isTerm ? 0 : 10, padding: isTerm ? '12px 16px' : 18, borderBottom: isTerm ? `1px solid ${t.border}` : undefined }}>
              <div style={{ display: 'flex', alignItems: 'flex-start', gap: 10 }}>
                <Icon name="database" size={18} color={t.fg2} />
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ fontSize: 14, fontWeight: 500, color: t.fg1, fontFamily: t.mono }}>{v.name}</div>
                  <div style={{ fontSize: 11, color: t.fg3, fontFamily: t.mono, marginTop: 2, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{v.mountpoint}</div>
                </div>
                <div style={{ fontSize: 10, color: t.fg3, fontFamily: t.mono, padding: '2px 6px', background: t.surfaceAlt, borderRadius: isTerm ? 0 : 4 }}>{v.driver}</div>
              </div>
              <div style={{ marginTop: 12, display: 'flex', alignItems: 'baseline', justifyContent: 'space-between' }}>
                <div>
                  <span style={{ fontSize: isEdit ? 28 : 18, fontWeight: isEdit ? 400 : 600, fontFamily: isEdit ? 'var(--font-serif)' : t.mono, color: t.fg1, fontVariantNumeric: 'tabular-nums' }}>{v.used.toFixed(1)}</span>
                  <span style={{ fontSize: 12, color: t.fg3, marginLeft: 4, fontFamily: t.mono }}>/ {v.size.toFixed(0)} {v.unit}</span>
                </div>
                <span style={{ fontSize: 12, color: pct > 90 ? t.danger : pct > 70 ? t.warning : t.fg2, fontFamily: t.mono, fontVariantNumeric: 'tabular-nums' }}>{pct.toFixed(1)}%</span>
              </div>
              <div style={{ marginTop: 8 }}>
                <Bar pct={pct} color={pct > 90 ? t.danger : pct > 70 ? t.warning : t.success} bg={t.surfaceAlt} h={6} />
              </div>
              <div style={{ marginTop: 10, fontSize: 11, color: t.fg3, fontFamily: t.mono }}>{v.refs} {v.refs === 1 ? 'reference' : 'references'}</div>
            </div>
          );
        })}
      </div>
    </div>
  );
}

function NetworksView({ t, variation }) {
  const isTerm = variation === 'terminal';
  return (
    <div style={{ flex: 1, overflow: 'auto', background: t.bg }}>
      <div style={tableHeader(t, isTerm, '32px 1fr 100px 100px 1.4fr 1.2fr 70px')}>
        <span></span><span>Name</span><span>Mode</span><span>State</span><span>Subnet</span><span>DNS</span><span style={{ textAlign: 'right' }}>Conn</span>
      </div>
      {SAMPLE.networks.map(n => (
        <div key={n.id} style={tableRow(t, isTerm, '32px 1fr 100px 100px 1.4fr 1.2fr 70px')}>
          <Icon name="network" size={16} color={t.fg3} />
          <div style={{ fontFamily: t.mono, fontSize: 13, color: t.fg1 }}>{n.name}</div>
          <div style={{ fontFamily: t.mono, fontSize: 12, color: t.fg2 }}>{n.mode}</div>
          <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}><StatusDot status={n.state === 'active' ? 'running' : 'stopped'} t={t} /><span style={{ fontSize: 12, color: t.success, textTransform: 'capitalize' }}>{n.state}</span></div>
          <div style={{ fontFamily: t.mono, fontSize: 12, color: t.fg2 }}>{n.subnet} <span style={{ color: t.fg3 }}>· gw {n.gateway}</span></div>
          <div style={{ fontFamily: t.mono, fontSize: 12, color: t.fg3 }}>{n.dns.join(', ') || '—'}</div>
          <div style={{ textAlign: 'right', fontFamily: t.mono, fontSize: 12, color: t.fg1, fontVariantNumeric: 'tabular-nums' }}>{n.containers}</div>
        </div>
      ))}
    </div>
  );
}

function StacksView({ t, variation, onInspect }) {
  const isTerm = variation === 'terminal';
  const isEdit = variation === 'editorial';
  return (
    <div style={{ flex: 1, overflow: 'auto', background: t.bg, padding: isTerm ? 0 : 16 }}>
      <div style={{ display: 'grid', gap: isTerm ? 0 : 12 }}>
        {SAMPLE.stacks.map(s => {
          const running = s.services.filter(sv => sv.state === 'running').length;
          const pct = (running / s.services.length) * 100;
          return (
            <div key={s.name} style={{ background: t.surface, border: `1px solid ${t.border}`, borderRadius: isTerm ? 0 : 10, padding: 18, borderBottom: isTerm ? `1px solid ${t.border}` : undefined }}>
              <div style={{ display: 'flex', alignItems: 'flex-start', gap: 14 }}>
                <Icon name="layers" size={20} color={t.accent} />
                <div style={{ flex: 1 }}>
                  <div style={{ display: 'flex', alignItems: 'baseline', gap: 12, flexWrap: 'wrap' }}>
                    <span style={{ fontSize: isEdit ? 22 : 16, fontWeight: isEdit ? 400 : 600, fontFamily: isEdit ? 'var(--font-serif)' : 'inherit', color: t.fg1 }}>{s.name}</span>
                    <span style={{ fontSize: 11, color: t.fg3, fontFamily: t.mono }}>{s.file}</span>
                  </div>
                  <div style={{ marginTop: 4, fontSize: 12, color: t.fg3, fontFamily: t.mono }}>
                    <span style={{ color: running === s.services.length ? t.success : running > 0 ? t.warning : t.fg3 }}>{running}/{s.services.length} running</span>
                    {' · '}{s.health}{' · restart '}{s.restart}
                  </div>
                </div>
                <div style={{ display: 'flex', gap: 6 }}>
                  <button style={pillBtn(t, t.success)}><Icon name="play" size={12} color={t.success} />Up</button>
                  <button style={pillBtn(t, t.fg2)}><Icon name="stop" size={12} color={t.fg2} />Down</button>
                  <button style={iconBtn(t)} onClick={() => onInspect(s)}><Icon name="info" size={13} color={t.fg2} /></button>
                </div>
              </div>
              <div style={{ marginTop: 14, display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(220px, 1fr))', gap: 8 }}>
                {s.services.map(sv => (
                  <div key={sv.name} style={{ padding: '8px 10px', background: t.surfaceAlt, border: `1px solid ${t.border}`, borderRadius: isTerm ? 0 : 6, display: 'flex', alignItems: 'center', gap: 8 }}>
                    <StatusDot status={sv.state} t={t} />
                    <div style={{ flex: 1, minWidth: 0 }}>
                      <div style={{ fontSize: 12, fontWeight: 500, color: t.fg1, fontFamily: t.mono }}>{sv.name}</div>
                      <div style={{ fontSize: 10, color: t.fg3, fontFamily: t.mono, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{sv.image}</div>
                    </div>
                    <span style={{ fontSize: 10, fontFamily: t.mono, color: sv.health === 'healthy' ? t.success : sv.health === 'waiting' ? t.warning : t.fg3 }}>{sv.health}</span>
                  </div>
                ))}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}

function LogsView({ t, variation }) {
  const isTerm = variation === 'terminal';
  return (
    <div style={{ flex: 1, overflow: 'auto', background: isTerm ? t.bg : t.surface, padding: 0 }}>
      <div style={{ padding: '12px 20px', display: 'flex', alignItems: 'center', gap: 10, borderBottom: `1px solid ${t.border}`, background: t.surface, position: 'sticky', top: 0 }}>
        <span style={{ width: 8, height: 8, borderRadius: '50%', background: t.success, animation: 'pulse 1.5s ease-in-out infinite' }} />
        <span style={{ fontSize: 12, fontFamily: t.mono, color: t.fg1 }}>follow</span>
        <span style={{ color: t.fg3, fontSize: 12, fontFamily: t.mono }}>· mlperf-inference-llama2 · {SAMPLE.logs.length} lines</span>
        <div style={{ flex: 1 }} />
        <button style={pillBtn(t)}>Pause</button>
        <button style={pillBtn(t)}>Clear</button>
        <button style={pillBtn(t)}>Copy</button>
      </div>
      <div style={{ fontFamily: t.mono, fontSize: 12, lineHeight: 1.6, padding: 16 }}>
        {SAMPLE.logs.map((line, i) => {
          const isErr = line.includes('[ERROR]');
          const isWarn = line.includes('[WARN]');
          return (
            <div key={i} style={{ display: 'flex', gap: 12, padding: '1px 4px', background: isErr ? 'rgba(220,38,38,0.06)' : 'transparent', color: isErr ? t.danger : isWarn ? t.warning : t.fg2 }}>
              <span style={{ color: t.fg3, userSelect: 'none', minWidth: 32, textAlign: 'right' }}>{i + 1}</span>
              <span style={{ whiteSpace: 'pre-wrap', wordBreak: 'break-all' }}>{line}</span>
            </div>
          );
        })}
      </div>
    </div>
  );
}

function tableHeader(t, isTerm, cols) {
  return {
    display: 'grid', gridTemplateColumns: cols, gap: 12,
    padding: isTerm ? '6px 16px' : '10px 20px', alignItems: 'center',
    fontSize: 11, color: t.fg3, fontWeight: 600,
    letterSpacing: isTerm ? 0 : '0.06em', textTransform: isTerm ? 'none' : 'uppercase',
    fontFamily: isTerm ? t.mono : 'inherit',
    borderBottom: `1px solid ${t.border}`, background: t.surface,
    position: 'sticky', top: 0, zIndex: 1,
  };
}
function tableRow(t, isTerm, cols) {
  return {
    display: 'grid', gridTemplateColumns: cols, gap: 12,
    padding: isTerm ? '6px 16px' : '12px 20px', alignItems: 'center',
    borderBottom: `1px solid ${t.border}`, fontSize: 13, color: t.fg1,
    cursor: 'pointer',
  };
}
function iconBtn(t) {
  return { background: 'transparent', border: 'none', padding: 5, borderRadius: 4, cursor: 'pointer', display: 'flex', alignItems: 'center', justifyContent: 'center' };
}
function pillBtn(t, accent) {
  return { display: 'inline-flex', alignItems: 'center', gap: 4, padding: '4px 10px', background: t.surfaceAlt, border: `1px solid ${t.border}`, borderRadius: 999, color: accent || t.fg2, fontSize: 11, fontFamily: 'inherit', cursor: 'pointer' };
}

window.cguiViews = { ContainersView, ImagesView, VolumesView, NetworksView, StacksView, LogsView, iconBtn, pillBtn };
