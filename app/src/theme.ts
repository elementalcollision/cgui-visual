// Workbench theme tokens (locked variation per design handoff).
// Source: design_handoff_cgui/prototype/theme.js — `workbench` block.

export type ThemeTokens = {
  bg: string; surface: string; surfaceAlt: string;
  border: string; borderStrong: string;
  fg1: string; fg2: string; fg3: string;
  accent: string; accentInk: string; accentSoft: string;
  success: string; warning: string; danger: string;
  hover: string; selected: string;
  headerBar: string; tabActive: string;
  sparkline: string;
  mono: string;
  radius: number;
  density: 'comfortable' | 'spacious' | 'compact';
};

const MONO = 'ui-monospace, "SF Mono", Menlo, Consolas, monospace';

const light = {
  bg: '#F7F8FA', surface: '#FFFFFF', surfaceAlt: '#F1F3F7',
  border: '#E4E8EF', borderStrong: '#CFD5E0',
  fg1: '#11141F', fg2: '#3D455A', fg3: '#6B7388',
  accent: '#2563EB', accentInk: '#FFFFFF', accentSoft: '#DBEAFE',
  success: '#059669', warning: '#D97706', danger: '#DC2626',
  hover: 'rgba(17,20,31,0.04)', selected: 'rgba(37,99,235,0.08)',
  headerBar: '#FFFFFF', tabActive: '#2563EB',
  sparkline: '#2563EB',
  mono: MONO,
};

const dark = {
  bg: '#0E1117', surface: '#161B22', surfaceAlt: '#1C232E',
  border: '#262C36', borderStrong: '#383F4B',
  fg1: '#E6EDF3', fg2: '#9DA7B3', fg3: '#6B7382',
  accent: '#58A6FF', accentInk: '#0E1117', accentSoft: 'rgba(88,166,255,0.15)',
  success: '#3FB950', warning: '#D29922', danger: '#F85149',
  hover: 'rgba(255,255,255,0.04)', selected: 'rgba(88,166,255,0.10)',
  headerBar: '#161B22', tabActive: '#58A6FF',
  sparkline: '#58A6FF',
  mono: MONO,
};

export function getTheme(isDark: boolean): ThemeTokens {
  return { ...(isDark ? dark : light), radius: 8, density: 'comfortable' };
}
