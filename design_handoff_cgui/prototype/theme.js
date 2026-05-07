/* Three visual variations for cgui.
   Each defines color tokens for light + dark, plus stylistic flags.
   No MLC brand colors — neutral devtool palette per spec. */

window.VARIATIONS = {
  workbench: {
    name: 'Workbench',
    blurb: 'Familiar devtool. Dense tables, semantic colors, OrbStack-adjacent.',
    light: {
      bg: '#F7F8FA',     surface: '#FFFFFF',  surfaceAlt: '#F1F3F7',
      border: '#E4E8EF', borderStrong: '#CFD5E0',
      fg1: '#11141F',    fg2: '#3D455A',      fg3: '#6B7388',
      accent: '#2563EB', accentInk: '#FFFFFF', accentSoft: '#DBEAFE',
      success: '#059669', warning: '#D97706', danger: '#DC2626',
      hover: 'rgba(17,20,31,0.04)', selected: 'rgba(37,99,235,0.08)',
      headerBar: '#FFFFFF', tabActive: '#2563EB',
      sparkline: '#2563EB',
      mono: 'ui-monospace, "SF Mono", Menlo, Consolas, monospace',
    },
    dark: {
      bg: '#0E1117',     surface: '#161B22',  surfaceAlt: '#1C232E',
      border: '#262C36', borderStrong: '#383F4B',
      fg1: '#E6EDF3',    fg2: '#9DA7B3',      fg3: '#6B7382',
      accent: '#58A6FF', accentInk: '#0E1117', accentSoft: 'rgba(88,166,255,0.15)',
      success: '#3FB950', warning: '#D29922', danger: '#F85149',
      hover: 'rgba(255,255,255,0.04)', selected: 'rgba(88,166,255,0.10)',
      headerBar: '#161B22', tabActive: '#58A6FF',
      sparkline: '#58A6FF',
      mono: 'ui-monospace, "SF Mono", Menlo, Consolas, monospace',
    },
    radius: 8, density: 'comfortable', font: 'sans',
  },

  editorial: {
    name: 'Editorial',
    blurb: 'Calm, generous whitespace. Serif numerics. Magazine-style detail panes.',
    light: {
      bg: '#FBFAF7',     surface: '#FFFFFF',  surfaceAlt: '#F4F2EE',
      border: '#E8E4DB', borderStrong: '#D4CFC2',
      fg1: '#1A1814',    fg2: '#4A4640',      fg3: '#7A746A',
      accent: '#1A1814', accentInk: '#FBFAF7', accentSoft: '#EBE6DA',
      success: '#3F6B40', warning: '#A56B1F', danger: '#8B2F2F',
      hover: 'rgba(26,24,20,0.03)', selected: 'rgba(26,24,20,0.06)',
      headerBar: '#FBFAF7', tabActive: '#1A1814',
      sparkline: '#1A1814',
      mono: 'ui-monospace, "SF Mono", Menlo, Consolas, monospace',
    },
    dark: {
      bg: '#1A1814',     surface: '#22201B',  surfaceAlt: '#2A2722',
      border: '#34302A', borderStrong: '#4A4640',
      fg1: '#F5F1E8',    fg2: '#B8B0A0',      fg3: '#7A746A',
      accent: '#F5F1E8', accentInk: '#1A1814', accentSoft: 'rgba(245,241,232,0.10)',
      success: '#7FA87F', warning: '#D4A856', danger: '#D67878',
      hover: 'rgba(245,241,232,0.04)', selected: 'rgba(245,241,232,0.07)',
      headerBar: '#22201B', tabActive: '#F5F1E8',
      sparkline: '#D4A856',
      mono: 'ui-monospace, "SF Mono", Menlo, Consolas, monospace',
    },
    radius: 4, density: 'spacious', font: 'serif-numerics',
  },

  terminal: {
    name: 'Terminal',
    blurb: 'Monospace-leaning. Dense grid, ASCII rules, true-to-TUI roots.',
    light: {
      bg: '#F4F4F0',     surface: '#FBFBF7',  surfaceAlt: '#EDEDE6',
      border: '#1C1C18', borderStrong: '#1C1C18',
      fg1: '#1C1C18',    fg2: '#3A3A33',      fg3: '#6E6E62',
      accent: '#1C1C18', accentInk: '#F4F4F0', accentSoft: '#E0E0D6',
      success: '#3F7A3F', warning: '#A56B0F', danger: '#A52A2A',
      hover: 'rgba(28,28,24,0.05)', selected: 'rgba(28,28,24,0.10)',
      headerBar: '#1C1C18', tabActive: '#F4F4F0',
      sparkline: '#1C1C18',
      mono: '"JetBrains Mono", ui-monospace, "SF Mono", Menlo, monospace',
    },
    dark: {
      bg: '#0A0A08',     surface: '#0F0F0C',  surfaceAlt: '#161613',
      border: '#3FA34F', borderStrong: '#3FA34F',
      fg1: '#D4D4C8',    fg2: '#9C9C8E',      fg3: '#6E6E62',
      accent: '#3FA34F', accentInk: '#0A0A08', accentSoft: 'rgba(63,163,79,0.15)',
      success: '#3FA34F', warning: '#D4A800', danger: '#E04848',
      hover: 'rgba(212,212,200,0.04)', selected: 'rgba(63,163,79,0.12)',
      headerBar: '#0F0F0C', tabActive: '#3FA34F',
      sparkline: '#3FA34F',
      mono: '"JetBrains Mono", ui-monospace, "SF Mono", Menlo, monospace',
    },
    radius: 0, density: 'compact', font: 'mono',
  },
};
