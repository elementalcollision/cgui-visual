// Stack dependency visualizer (B3).
//
// Renders a per-stack force-free DAG of services with `depends_on`
// edges. Layout is a topological-layer algorithm (Kahn's algorithm
// + per-layer slotting), rendered as plain SVG so we don't pay for
// react-flow / cytoscape just for a few nodes per stack.
//
// Edge direction: source.dependsOn(target) → arrow points source → target
// (i.e. "needs"). Cycles are detected and any unresolved nodes get
// dropped into a "?" layer at the end so they still render.

import { useMemo, useState } from 'react';
import type { ThemeTokens } from './theme';
import type { Stack, Service } from './types';
import { Icon, iconBtn } from './components';

interface PositionedNode {
  svc: Service;
  x: number;
  y: number;
  layer: number;
}

interface Edge {
  from: string;
  to: string;
}

const NODE_W = 200;
const NODE_H = 64;
const COL_GAP = 80;
const ROW_GAP = 24;
const PAD = 32;

// Kahn's algorithm grouped by topological depth. Nodes that participate
// in a cycle land in a final layer so they still render rather than
// silently disappearing.
function layout(services: Service[]): { nodes: PositionedNode[]; edges: Edge[]; width: number; height: number } {
  const byName = new Map<string, Service>();
  for (const s of services) byName.set(s.name, s);

  // Build the dependency graph. Filter dependsOn edges down to nodes
  // that actually exist in this stack — Compose lets you reference
  // things outside the stack but we don't want dangling arrows.
  const adj = new Map<string, string[]>(); // svc → things it depends on (in-edges)
  const inDeg = new Map<string, number>(); // svc → count of unresolved deps
  for (const s of services) {
    inDeg.set(s.name, 0);
    adj.set(s.name, []);
  }
  for (const s of services) {
    for (const dep of s.dependsOn ?? []) {
      if (!byName.has(dep) || dep === s.name) continue;
      adj.get(s.name)!.push(dep);
      inDeg.set(s.name, (inDeg.get(s.name) ?? 0) + 1);
    }
  }

  // Layer assignment: roots first (no dependencies), then peel.
  const layerOf = new Map<string, number>();
  let frontier = services.filter(s => (inDeg.get(s.name) ?? 0) === 0).map(s => s.name);
  let depth = 0;
  while (frontier.length > 0) {
    const next: string[] = [];
    for (const name of frontier) {
      layerOf.set(name, depth);
    }
    // Recompute in-degrees against the layer just placed.
    const placed = new Set(layerOf.keys());
    for (const s of services) {
      if (placed.has(s.name)) continue;
      const remaining = (s.dependsOn ?? []).filter(d => byName.has(d) && !placed.has(d)).length;
      if (remaining === 0) next.push(s.name);
    }
    depth++;
    frontier = next;
  }
  // Anything left over participates in a cycle — bucket at the deepest layer.
  for (const s of services) {
    if (!layerOf.has(s.name)) layerOf.set(s.name, depth);
  }
  const totalLayers = Math.max(...layerOf.values()) + 1;

  // Pack each layer's nodes into a column. Stable order = original
  // service order, so the visualisation is deterministic per stack.
  const layers: Service[][] = Array.from({ length: totalLayers }, () => []);
  for (const s of services) layers[layerOf.get(s.name)!].push(s);

  const colCount = layers.length;
  const tallestCol = Math.max(...layers.map(l => l.length), 1);
  const width = PAD * 2 + colCount * NODE_W + Math.max(0, colCount - 1) * COL_GAP;
  const height = PAD * 2 + tallestCol * NODE_H + Math.max(0, tallestCol - 1) * ROW_GAP;

  const nodes: PositionedNode[] = [];
  for (let li = 0; li < layers.length; li++) {
    const col = layers[li];
    const colHeight = col.length * NODE_H + Math.max(0, col.length - 1) * ROW_GAP;
    const yStart = PAD + (height - PAD * 2 - colHeight) / 2;
    for (let i = 0; i < col.length; i++) {
      nodes.push({
        svc: col[i],
        x: PAD + li * (NODE_W + COL_GAP),
        y: yStart + i * (NODE_H + ROW_GAP),
        layer: li,
      });
    }
  }

  const edges: Edge[] = [];
  for (const s of services) {
    for (const dep of s.dependsOn ?? []) {
      if (!byName.has(dep) || dep === s.name) continue;
      edges.push({ from: s.name, to: dep });
    }
  }

  return { nodes, edges, width, height };
}

// Cubic Bezier from a node's left edge to another's right edge — the
// reverse direction matches "depends on" reading right-to-left. Control
// points hug the midpoint x to avoid overshoots when layers stack tall.
function edgePath(from: PositionedNode, to: PositionedNode): string {
  const x1 = from.x;
  const y1 = from.y + NODE_H / 2;
  const x2 = to.x + NODE_W;
  const y2 = to.y + NODE_H / 2;
  const mx = (x1 + x2) / 2;
  return `M ${x1} ${y1} C ${mx} ${y1}, ${mx} ${y2}, ${x2} ${y2}`;
}

export default function StackGraphModal({ stack, t, onClose }: {
  stack: Stack;
  t: ThemeTokens;
  onClose: () => void;
}) {
  const { nodes, edges, width, height } = useMemo(() => layout(stack.services), [stack.services]);
  const [hover, setHover] = useState<string | null>(null);

  const stateColor = (state: string) => state === 'running' ? t.success
    : state === 'paused' ? t.warning
    : t.fg3;

  return (
    <div onClick={onClose} style={{
      position: 'absolute', inset: 0, background: 'rgba(0,0,0,0.55)',
      display: 'flex', alignItems: 'center', justifyContent: 'center',
      zIndex: 100, backdropFilter: 'blur(2px)',
    }}>
      <div onClick={e => e.stopPropagation()} style={{
        width: 'min(85vw, 1100px)', maxHeight: '85vh',
        background: t.surface, border: `1px solid ${t.border}`, borderRadius: 12,
        overflow: 'hidden', display: 'flex', flexDirection: 'column',
        boxShadow: '0 24px 80px rgba(0,0,0,0.45)',
      }}>
        <div style={{
          padding: '14px 22px', borderBottom: `1px solid ${t.border}`,
          display: 'flex', alignItems: 'center', gap: 12,
        }}>
          <Icon name="layers" size={20} color={t.accent} />
          <div style={{ flex: 1, minWidth: 0 }}>
            <div style={{ fontSize: 15, fontWeight: 600, color: t.fg1 }}>{stack.name}</div>
            <div style={{ fontSize: 11, color: t.fg3, fontFamily: t.mono, marginTop: 2 }}>
              {stack.services.length} service{stack.services.length === 1 ? '' : 's'}
              {' · '}{edges.length} dependenc{edges.length === 1 ? 'y' : 'ies'}
              {hover && ` · hovering ${hover}`}
            </div>
          </div>
          <button onClick={onClose} style={iconBtn()} title="Close">
            <Icon name="x" size={16} color={t.fg2} />
          </button>
        </div>
        <div style={{
          flex: 1, overflow: 'auto', background: t.bg, padding: 8,
        }}>
          <svg
            width={width} height={height}
            style={{ display: 'block', minWidth: '100%' }}
          >
            <defs>
              <marker id="arrow" viewBox="0 0 10 10" refX="9" refY="5"
                      markerWidth="6" markerHeight="6" orient="auto-start-reverse">
                <path d="M 0 0 L 10 5 L 0 10 z" fill={t.fg3} />
              </marker>
            </defs>
            {edges.map((e, i) => {
              const f = nodes.find(n => n.svc.name === e.from);
              const tg = nodes.find(n => n.svc.name === e.to);
              if (!f || !tg) return null;
              const lit = hover === e.from || hover === e.to;
              return (
                <path
                  key={i}
                  d={edgePath(f, tg)}
                  stroke={lit ? t.accent : t.border}
                  strokeWidth={lit ? 2 : 1.5}
                  fill="none"
                  markerEnd="url(#arrow)"
                  style={{ transition: 'stroke 120ms ease' }}
                />
              );
            })}
            {nodes.map(n => {
              const lit = hover === n.svc.name;
              return (
                <g
                  key={n.svc.name}
                  transform={`translate(${n.x}, ${n.y})`}
                  onMouseEnter={() => setHover(n.svc.name)}
                  onMouseLeave={() => setHover(prev => prev === n.svc.name ? null : prev)}
                  style={{ cursor: 'default' }}
                >
                  <rect
                    width={NODE_W} height={NODE_H} rx={8} ry={8}
                    fill={t.surface}
                    stroke={lit ? t.accent : t.border}
                    strokeWidth={lit ? 2 : 1}
                  />
                  <circle cx={14} cy={20} r={5} fill={stateColor(n.svc.state)} />
                  <text x={28} y={24} fill={t.fg1}
                        style={{ fontFamily: 'inherit', fontSize: 13, fontWeight: 600 }}>
                    {n.svc.name}
                  </text>
                  <text x={14} y={44} fill={t.fg3}
                        style={{ fontFamily: t.mono, fontSize: 10 }}>
                    {n.svc.image.length > 28 ? `${n.svc.image.slice(0, 25)}…` : n.svc.image}
                  </text>
                  <text x={14} y={58} fill={stateColor(n.svc.state)}
                        style={{ fontFamily: t.mono, fontSize: 10, textTransform: 'uppercase', letterSpacing: '0.05em' }}>
                    {n.svc.state} · {n.svc.health}
                  </text>
                </g>
              );
            })}
          </svg>
        </div>
        <div style={{
          padding: '10px 22px', borderTop: `1px solid ${t.border}`,
          background: t.surfaceAlt, fontFamily: t.mono, fontSize: 10, color: t.fg3,
          display: 'flex', alignItems: 'center', gap: 14,
        }}>
          <span>Arrow: A → B means A depends on B (B starts first).</span>
          <div style={{ flex: 1 }} />
          <span>● running</span>
          <span style={{ color: t.warning }}>● waiting</span>
          <span>● stopped</span>
        </div>
      </div>
    </div>
  );
}
