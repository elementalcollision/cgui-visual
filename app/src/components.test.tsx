import { describe, it, expect } from 'vitest';
import { render } from '@testing-library/react';
import { fmtBytes, Sparkline, StatusDot, Bar } from './components';
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

describe('Bar', () => {
  it('clamps fill to 100% even when pct exceeds it', () => {
    const { container } = render(<Bar pct={250} color="#f00" bg="#000" />);
    const fill = container.querySelector('div > div') as HTMLDivElement;
    expect(fill.style.width).toBe('100%');
  });
});
