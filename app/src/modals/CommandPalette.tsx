import { useEffect, useMemo, useRef, useState } from 'react';
import { Icon } from '../components';
import type { ThemeTokens } from '../theme';
import type { Container, Image, Stack, Tab } from '../types';
import { Backdrop } from './shared';

// ─── Command palette ──────────────────────────────────────────────────
//
// Cmd-K opens a floating fuzzy-search box with three result classes:
//
//   • Tab    — switch the main view (containers / images / volumes / …)
//   • Action — global affordances (open Settings, Doctor, Pull, ...)
//   • Entity — a specific Container / Image / Stack to inspect
//
// Selecting an entity routes through `onPick` which dispatches the
// right modal/tab change in App.tsx. Match scoring is a tiny subsequence
// fuzzy match — fast enough for thousands of rows and zero deps.

export type CommandKind = 'tab' | 'action' | 'container' | 'image' | 'stack';

export type CommandResult =
  | { kind: 'tab'; tab: Tab; label: string }
  | { kind: 'action'; id: string; label: string }
  | { kind: 'container'; container: Container; label: string }
  | { kind: 'image'; image: Image; label: string }
  | { kind: 'stack'; stack: Stack; label: string };

// Lightweight subsequence scorer: returns Infinity on miss, a smaller
// number for a tighter match. Letters in `q` must appear in `text` in
// order; matches at word starts cost less than mid-word matches.
export function fuzzyScore(text: string, q: string): number {
  if (!q) return 0;
  const lt = text.toLowerCase();
  const lq = q.toLowerCase();
  let i = 0, j = 0, score = 0, lastMatch = -1;
  while (i < lt.length && j < lq.length) {
    if (lt[i] === lq[j]) {
      // Word-start bonus: matches at index 0 or just after non-alnum
      // are cheap. Consecutive matches stay cheap; gaps add cost.
      const wordStart = i === 0 || !/[a-z0-9]/i.test(lt[i - 1]);
      score += wordStart ? 0 : (lastMatch === i - 1 ? 1 : 4);
      lastMatch = i;
      j++;
    }
    i++;
  }
  return j === lq.length ? score : Infinity;
}

export function CommandPaletteModal({
  t, onClose,
  containers, images, stacks,
  onTab, onAction, onContainer, onImage, onStack,
}: {
  t: ThemeTokens; onClose: () => void;
  containers: Container[]; images: Image[]; stacks: Stack[];
  onTab: (tab: Tab) => void;
  onAction: (id: string) => void;
  onContainer: (c: Container) => void;
  onImage: (img: Image) => void;
  onStack: (s: Stack) => void;
}) {
  const [q, setQ] = useState('');
  const [active, setActive] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  useEffect(() => { inputRef.current?.focus(); }, []);

  // Build the universe of command targets, then filter + score against q.
  // Limit to top 12 results so navigation stays one-screen.
  const all: CommandResult[] = useMemo(() => {
    const tabs: CommandResult[] = [
      { kind: 'tab', tab: 'containers', label: 'Tab: Containers' },
      { kind: 'tab', tab: 'images', label: 'Tab: Images' },
      { kind: 'tab', tab: 'volumes', label: 'Tab: Volumes' },
      { kind: 'tab', tab: 'networks', label: 'Tab: Networks' },
      { kind: 'tab', tab: 'stacks', label: 'Tab: Stacks' },
      { kind: 'tab', tab: 'logs', label: 'Tab: Logs' },
    ];
    const actions: CommandResult[] = [
      { kind: 'action', id: 'settings', label: 'Open Settings…' },
      { kind: 'action', id: 'doctor', label: 'Run Doctor checks' },
      { kind: 'action', id: 'pull', label: 'Pull image…' },
    ];
    const cs: CommandResult[] = containers.map(c => ({
      kind: 'container', container: c, label: `${c.name} · ${c.image}`,
    }));
    const ims: CommandResult[] = images.map(i => ({
      kind: 'image', image: i, label: `${i.ref}`,
    }));
    const ss: CommandResult[] = stacks.map(s => ({
      kind: 'stack', stack: s, label: `${s.name} · ${s.services.length} svc`,
    }));
    return [...tabs, ...actions, ...cs, ...ims, ...ss];
  }, [containers, images, stacks]);

  const ranked = useMemo(() => {
    const trimmed = q.trim();
    const scored = all
      .map(r => ({ r, score: fuzzyScore(r.label, trimmed) }))
      .filter(x => x.score !== Infinity);
    scored.sort((a, b) => a.score - b.score);
    return scored.slice(0, 12).map(x => x.r);
  }, [all, q]);

  // Keep the active index in range when results shrink under the cursor.
  useEffect(() => {
    if (active >= ranked.length) setActive(0);
  }, [ranked.length, active]);

  const dispatch = (r: CommandResult) => {
    onClose();
    if (r.kind === 'tab') onTab(r.tab);
    else if (r.kind === 'action') onAction(r.id);
    else if (r.kind === 'container') onContainer(r.container);
    else if (r.kind === 'image') onImage(r.image);
    else if (r.kind === 'stack') onStack(r.stack);
  };

  const onKey = (e: React.KeyboardEvent) => {
    if (e.key === 'ArrowDown') { e.preventDefault(); setActive(i => Math.min(ranked.length - 1, i + 1)); }
    else if (e.key === 'ArrowUp') { e.preventDefault(); setActive(i => Math.max(0, i - 1)); }
    else if (e.key === 'Enter') { e.preventDefault(); if (ranked[active]) dispatch(ranked[active]); }
  };

  const kindBadge: Record<CommandKind, { label: string; color: string }> = {
    tab:       { label: 'Tab',       color: t.fg3 },
    action:    { label: 'Action',    color: t.accent },
    container: { label: 'Container', color: t.success },
    image:     { label: 'Image',     color: t.warning },
    stack:     { label: 'Stack',     color: t.accent },
  };

  return (
    <Backdrop title="Command palette" onClose={onClose}>
      <div style={{ width: 580, background: t.surface, border: `1px solid ${t.border}`, borderRadius: 12, overflow: 'hidden', boxShadow: '0 24px 80px rgba(0,0,0,0.45)' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '12px 16px', borderBottom: `1px solid ${t.border}` }}>
          <Icon name="info" size={16} color={t.fg3} />
          <input
            ref={inputRef}
            value={q}
            onChange={e => setQ(e.target.value)}
            onKeyDown={onKey}
            placeholder="Type a tab, action, container, image, or stack…"
            spellCheck={false}
            style={{
              flex: 1, background: 'transparent', border: 'none', outline: 'none',
              fontSize: 14, color: t.fg1, fontFamily: 'inherit',
            }}
          />
          <span style={{ fontSize: 10, color: t.fg3, fontFamily: t.mono, padding: '2px 6px', background: t.surfaceAlt, border: `1px solid ${t.border}`, borderRadius: 4 }}>esc</span>
        </div>
        <div style={{ maxHeight: 360, overflow: 'auto' }}>
          {ranked.length === 0 ? (
            <div style={{ padding: '20px 16px', fontSize: 12, color: t.fg3, fontStyle: 'italic' }}>No matches.</div>
          ) : ranked.map((r, i) => {
            const k = kindBadge[r.kind];
            const sel = i === active;
            return (
              <button
                key={`${r.kind}-${i}`}
                onMouseEnter={() => setActive(i)}
                onClick={() => dispatch(r)}
                style={{
                  width: '100%', textAlign: 'left',
                  display: 'flex', alignItems: 'center', gap: 10,
                  padding: '10px 16px',
                  background: sel ? t.selected : 'transparent',
                  border: 'none', borderLeft: `2px solid ${sel ? t.accent : 'transparent'}`,
                  cursor: 'pointer', color: 'inherit',
                }}
              >
                <span style={{
                  fontFamily: t.mono, fontSize: 10, fontWeight: 700,
                  color: k.color,
                  letterSpacing: '0.05em', textTransform: 'uppercase',
                  width: 64, flexShrink: 0,
                }}>{k.label}</span>
                <span style={{ flex: 1, fontSize: 13, color: t.fg1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{r.label}</span>
                {sel && <span style={{ fontSize: 10, color: t.fg3, fontFamily: t.mono }}>↵</span>}
              </button>
            );
          })}
        </div>
        <div style={{ padding: '8px 14px', borderTop: `1px solid ${t.border}`, background: t.surfaceAlt, fontSize: 10, fontFamily: t.mono, color: t.fg3, display: 'flex', gap: 12 }}>
          <span>↑↓ navigate</span><span>↵ open</span><span>esc close</span>
          <div style={{ flex: 1 }} />
          <span>{ranked.length} result{ranked.length === 1 ? '' : 's'}</span>
        </div>
      </div>
    </Backdrop>
  );
}
