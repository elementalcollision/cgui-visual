import { useEffect, useMemo, useState } from 'react';
import { api } from '../api';
import { Icon, iconBtn, pillBtn, fmtCreated } from '../components';
import type { ThemeTokens } from '../theme';
import { Backdrop } from './shared';
import { parseImageInspect, prettyCmd } from './inspect';

export function ImageInspectModal({ t, reference, onClose }: {
  t: ThemeTokens; reference: string; onClose: () => void;
}) {
  const [json, setJson] = useState<string>('Loading…');
  const [tab, setTab] = useState<'layers' | 'raw'>('layers');
  useEffect(() => {
    let cancelled = false;
    api.inspectImage(reference)
      .then(s => { if (!cancelled) setJson(s); })
      .catch(e => { if (!cancelled) setJson(`error: ${e}`); });
    return () => { cancelled = true; };
  }, [reference]);

  const parsed = useMemo(() => parseImageInspect(json), [json]);
  const layers = parsed?.layers ?? [];
  const realLayers = layers.filter(l => !l.emptyLayer).length;

  return (
    <Backdrop title={`Image ${reference}`} onClose={onClose}>
      <div style={{ width: 820, maxHeight: '85vh', background: t.surface, border: `1px solid ${t.border}`, borderRadius: 12, overflow: 'hidden', display: 'flex', flexDirection: 'column', boxShadow: '0 24px 80px rgba(0,0,0,0.4)' }}>
        <div style={{ padding: '18px 22px', borderBottom: `1px solid ${t.border}`, display: 'flex', alignItems: 'flex-start', gap: 12 }}>
          <Icon name="image" size={20} color={t.accent} />
          <div style={{ flex: 1, minWidth: 0 }}>
            <div style={{ fontSize: 16, fontWeight: 600, color: t.fg1, fontFamily: t.mono, wordBreak: 'break-all' }}>{reference}</div>
            <div style={{ fontSize: 12, color: t.fg3, fontFamily: t.mono, marginTop: 4 }}>
              container image inspect
              {parsed && (
                <span style={{ marginLeft: 8 }}>
                  · {realLayers} layer{realLayers === 1 ? '' : 's'}
                  {parsed.architecture && ` · ${parsed.os ?? 'linux'}/${parsed.architecture}`}
                </span>
              )}
            </div>
          </div>
          <button onClick={onClose} style={iconBtn()} aria-label="Close"><Icon name="x" size={16} color={t.fg2} /></button>
        </div>
        <div style={{ display: 'flex', gap: 4, padding: '8px 16px 0', borderBottom: `1px solid ${t.border}` }}>
          {([
            { id: 'layers' as const, label: 'Layers', count: layers.length, available: layers.length > 0 },
            { id: 'raw'    as const, label: 'Raw',    count: 0,             available: true },
          ]).map(item => {
            const active = tab === item.id;
            const dim = !item.available && item.id !== 'raw';
            return (
              <button
                key={item.id}
                disabled={dim}
                onClick={() => setTab(item.id)}
                style={{
                  padding: '8px 12px',
                  fontSize: 12, fontWeight: 500, fontFamily: t.mono,
                  color: active ? t.fg1 : (dim ? t.fg3 : t.fg2),
                  background: 'transparent', border: 'none',
                  borderBottom: `2px solid ${active ? t.accent : 'transparent'}`,
                  cursor: dim ? 'default' : 'pointer',
                  opacity: dim ? 0.5 : 1, marginBottom: -1,
                }}
              >
                {item.label}{item.count > 0 && <span style={{ marginLeft: 6, color: t.fg3, fontSize: 10 }}>{item.count}</span>}
              </button>
            );
          })}
        </div>
        <div style={{ flex: 1, overflow: 'auto', padding: '12px 22px', minHeight: 240 }}>
          {tab === 'raw' || !parsed ? (
            <pre style={{ margin: 0, fontFamily: t.mono, fontSize: 11, lineHeight: 1.6, color: t.fg2, whiteSpace: 'pre-wrap', wordBreak: 'break-all' }}>{json}</pre>
          ) : layers.length === 0 ? (
            <div style={{ fontSize: 12, color: t.fg3, fontStyle: 'italic' }}>No layer history reported by the runtime.</div>
          ) : (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
              {layers.map((l, i) => (
                <div key={i} style={{
                  padding: '8px 10px',
                  background: l.emptyLayer ? 'transparent' : t.surfaceAlt,
                  border: `1px solid ${t.border}`,
                  borderRadius: 6,
                  opacity: l.emptyLayer ? 0.65 : 1,
                  display: 'grid', gridTemplateColumns: '40px 1fr auto', gap: 10, alignItems: 'baseline',
                }}>
                  <span style={{ fontFamily: t.mono, fontSize: 10, color: t.fg3, fontVariantNumeric: 'tabular-nums' }}>
                    #{i + 1}
                  </span>
                  <div style={{ minWidth: 0 }}>
                    <div style={{ fontFamily: t.mono, fontSize: 12, color: t.fg1, wordBreak: 'break-all', whiteSpace: 'pre-wrap' }}>
                      {prettyCmd(l.createdBy) || <span style={{ color: t.fg3 }}>(no command)</span>}
                    </div>
                    {(l.comment || l.diffId) && (
                      <div style={{ fontFamily: t.mono, fontSize: 10, color: t.fg3, marginTop: 4, wordBreak: 'break-all' }}>
                        {l.comment && <span>{l.comment}{l.diffId ? ' · ' : ''}</span>}
                        {l.diffId && <span>{l.diffId}</span>}
                      </div>
                    )}
                  </div>
                  <div style={{ textAlign: 'right' }}>
                    <div style={{ fontFamily: t.mono, fontSize: 10, color: t.fg3, whiteSpace: 'nowrap' }}>{fmtCreated(l.created)}</div>
                    {l.emptyLayer && (
                      <div style={{ fontFamily: t.mono, fontSize: 9, color: t.fg3, marginTop: 2, letterSpacing: '0.04em', textTransform: 'uppercase' }}>empty</div>
                    )}
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
        <div style={{ padding: '12px 22px', borderTop: `1px solid ${t.border}`, display: 'flex', gap: 8, justifyContent: 'flex-end', background: t.surfaceAlt }}>
          <button style={pillBtn(t)} onClick={() => navigator.clipboard?.writeText(json)}>Copy JSON</button>
          <button style={pillBtn(t)} onClick={onClose}>Close</button>
        </div>
      </div>
    </Backdrop>
  );
}
