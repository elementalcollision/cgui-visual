import { describe, it, expect } from 'vitest';
import { render } from '@testing-library/react';
import { fmtBytes, Sparkline, StatusDot, Bar, fmtAgo, fmtCreated } from './components';
import { getTheme } from './theme';

const t = getTheme(true);

describe('fmtBytes', () => {
  it('uses MiB for sub-GiB values', () => {
    expect(fmtBytes(0.001)).toMatch(/MiB$/);
    expect(fmtBytes(0.5)).toMatch(/MiB$/);
  });
  it('uses GiB for >= 1', () => {
    expect(fmtBytes(1.5)).toBe('1.5 GiB');
    expect(fmtBytes(64.0)).toBe('64.0 GiB');
  });
  it('drops decimals for values >= 100', () => {
    expect(fmtBytes(142.7)).toBe('143 GiB');
  });
});

describe('Sparkline', () => {
  it('emits a polyline with one point per data sample', () => {
    const { container } = render(<Sparkline data={[1, 2, 3, 4]} w={40} h={10} color="#000" max={4} />);
    const poly = container.querySelector('polyline');
    expect(poly).toBeTruthy();
    const points = poly!.getAttribute('points')!;
    expect(points.split(' ')).toHaveLength(4);
  });

  it('places the last point at the right edge', () => {
    const { container } = render(<Sparkline data={[5, 5, 5, 5]} w={60} h={20} color="#000" max={10} />);
    const points = container.querySelector('polyline')!.getAttribute('points')!.split(' ');
    expect(points[points.length - 1].split(',')[0]).toBe('60');
  });
});

describe('StatusDot', () => {
  it('uses success color for running', () => {
    const { container } = render(<StatusDot status="running" t={t} />);
    const dot = container.querySelector('span') as HTMLSpanElement;
    expect(dot.style.background).toBe('rgb(63, 185, 80)'); // dark theme success
  });
  it('uses warning color for paused', () => {
    const { container } = render(<StatusDot status="paused" t={t} />);
    const dot = container.querySelector('span') as HTMLSpanElement;
    expect(dot.style.background).toBe('rgb(210, 153, 34)');
  });
  it('uses muted color for exited', () => {
    const { container } = render(<StatusDot status="exited" t={t} />);
    const dot = container.querySelector('span') as HTMLSpanElement;
    expect(dot.style.background).toBe('rgb(107, 115, 130)');
  });
});

describe('fmtAgo', () => {
  it("returns '—' when no tick has been received", () => {
    expect(fmtAgo(0)).toBe('—');
  });
  it("uses 'just now' inside a tick window", () => {
    const now = 100_000;
    expect(fmtAgo(now - 500, now)).toBe('just now');
    expect(fmtAgo(now - 1500, now)).toBe('just now');
  });
  it("uses seconds, minutes, hours buckets above the floor", () => {
    const now = 10_000_000;
    expect(fmtAgo(now - 30_000, now)).toBe('30s ago');
    expect(fmtAgo(now - 90_000, now)).toBe('1m ago');
    expect(fmtAgo(now - 3 * 3600_000, now)).toBe('3h ago');
  });
});

describe('fmtCreated', () => {
  const now = Date.parse('2026-06-16T00:00:00Z');
  it("returns '—' for missing / unparseable input", () => {
    expect(fmtCreated('', now)).toBe('—');
    expect(fmtCreated('not a date', now)).toBe('—');
  });
  it("returns '—' for an epoch-zero stamp (the 1970 bug)", () => {
    // Date.parse here is 0, not NaN — the case the old `ago` helper missed.
    expect(fmtCreated('1970-01-01T00:00:00Z', now)).toBe('—');
  });
  it('formats a real timestamp as a relative age', () => {
    // Hours bucket runs up to 48h, so 24h ago reads as "24h ago".
    expect(fmtCreated('2026-06-15T00:00:00Z', now)).toBe('24h ago');
    expect(fmtCreated('2026-06-13T00:00:00Z', now)).toBe('3d ago');
    expect(fmtCreated('2026-04-17T00:00:00Z', now)).toBe('2mo ago');
  });
  it('accepts a date-only string (fixture shape)', () => {
    expect(fmtCreated('2026-06-13', now)).toBe('3d ago');
  });
});

describe('Bar', () => {
  it('clamps fill to 100% even when pct exceeds it', () => {
    const { container } = render(<Bar pct={250} color="#f00" bg="#000" />);
    const fill = container.querySelector('div > div') as HTMLDivElement;
    expect(fill.style.width).toBe('100%');
  });
});
