import { useEffect, useRef, useState } from 'react';
import { api } from '../api';
import { Icon, iconBtn, pillBtn } from '../components';
import { errMessage } from '../loadState';
import type { ThemeTokens } from '../theme';
import { Backdrop } from './shared';

// One-shot `container machine logs` viewer: machine output (stdio) or the VM
// boot log, last N lines, with Refresh. Not a live follow.
export function MachineLogsModal({ t, name, onClose }: {
  t: ThemeTokens; name: string; onClose: () => void;
}) {
  const [boot, setBoot] = useState(false);
  const [tail, setTail] = useState(200);
  const [tick, setTick] = useState(0);
  const [state, setState] = useState<{ text: string; error: string | null; loading: boolean }>({ text: '', error: null, loading: true });
  const preRef = useRef<HTMLPreElement>(null);
  useEffect(() => {
    let cancelled = false;
    setState(s => ({ ...s, loading: true }));
    api.machineLogs(name, { boot, tail })
      .then(text => { if (!cancelled) setState({ text, error: null, loading: false }); })
      .catch(e => { if (!cancelled) setState({ text: '', error: errMessage(e), loading: false }); });
    return () => { cancelled = true; };
  }, [name, boot, tail, tick]);
  useEffect(() => {
    const el = preRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [state.text]);
  // A machine that has never booted has no stdio log file; say that plainly.
  const friendly = state.error && /doesn.t exist|No such file/i.test(state.error)
    ? `No ${boot ? 'boot ' : ''}log yet: ${name} hasn't booted. Start it (run a command in it) and refresh.`
    : state.error;
  const seg = (active: boolean) => ({
    ...pillBtn(t), background: active ? t.accentSoft : 'transparent', color: active ? t.accent : t.fg2,
  });
  return (
    <Backdrop onClose={onClose} title={`Machine logs — ${name}`}>
      <div style={{ width: 760, maxHeight: '85vh', background: t.surface, border: `1px solid ${t.border}`, borderRadius: 12, overflow: 'hidden', display: 'flex', flexDirection: 'column', boxShadow: '0 24px 80px rgba(0,0,0,0.4)' }}>
        <div style={{ padding: '18px 22px', borderBottom: `1px solid ${t.border}`, display: 'flex', alignItems: 'flex-start', gap: 12 }}>
          <Icon name="logs" size={20} color={t.accent} />
          <div style={{ flex: 1 }}>
            <div style={{ fontSize: 18, fontWeight: 600, color: t.fg1 }}>Machine logs</div>
            <div style={{ fontSize: 12, color: t.fg3, fontFamily: t.mono, marginTop: 4 }}>container machine logs{boot ? ' --boot' : ''} {name}</div>
          </div>
          <button onClick={onClose} style={iconBtn()} aria-label="Close" title="Close"><Icon name="x" size={16} color={t.fg2} /></button>
        </div>
        <div style={{ padding: '10px 22px', borderBottom: `1px solid ${t.border}`, display: 'flex', alignItems: 'center', gap: 8 }}>
          <button style={seg(!boot)} aria-pressed={!boot} onClick={() => setBoot(false)}>Output</button>
          <button style={seg(boot)} aria-pressed={boot} onClick={() => setBoot(true)}>Boot log</button>
          <div style={{ flex: 1 }} />
          <label style={{ fontSize: 11, color: t.fg3, display: 'flex', alignItems: 'center', gap: 6 }}>
            Last
            <select aria-label="Lines to show" value={tail} onChange={e => setTail(Number(e.target.value))}
                    style={{ background: t.surfaceAlt, color: t.fg1, border: `1px solid ${t.border}`, borderRadius: 4, fontSize: 12, padding: '3px 6px' }}>
              {[100, 200, 1000, 5000].map(n => <option key={n} value={n}>{n}</option>)}
            </select>
            lines
          </label>
          <button style={pillBtn(t)} onClick={() => setTick(n => n + 1)} disabled={state.loading}>Refresh</button>
        </div>
        <pre ref={preRef} style={{ flex: 1, overflow: 'auto', margin: 0, padding: '14px 22px', minHeight: 240, fontFamily: t.mono, fontSize: 11, lineHeight: 1.6, color: friendly ? t.fg3 : t.fg2, whiteSpace: 'pre-wrap', wordBreak: 'break-all' }}>
          {state.loading && !state.text ? 'Loading…' : friendly ?? (state.text.trim() ? state.text : '(empty)')}
        </pre>
        <div style={{ padding: '12px 22px', borderTop: `1px solid ${t.border}`, display: 'flex', gap: 8, justifyContent: 'flex-end', background: t.surfaceAlt }}>
          <button style={pillBtn(t)} onClick={() => navigator.clipboard?.writeText(state.text)} disabled={!state.text}>Copy</button>
          <button style={pillBtn(t)} onClick={onClose}>Close</button>
        </div>
      </div>
    </Backdrop>
  );
}
