import { describe, it, expect } from 'vitest';
import { getTheme } from './theme';

describe('theme', () => {
  it('returns dark tokens by default', () => {
    const t = getTheme(true);
    expect(t.bg).toBe('#0E1117');
    expect(t.fg1).toBe('#E6EDF3');
    expect(t.density).toBe('comfortable');
  });

  it('switches surface colors between dark and light', () => {
    expect(getTheme(true).surface).not.toBe(getTheme(false).surface);
    expect(getTheme(false).bg).toBe('#F7F8FA');
  });

  it('keeps shared scales stable across modes', () => {
    expect(getTheme(true).radius).toBe(getTheme(false).radius);
    expect(getTheme(true).density).toBe(getTheme(false).density);
  });
});
