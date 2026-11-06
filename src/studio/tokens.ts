// The design tokens of the studio: the only place a colour, radius, blur, size or font is chosen. `bun scripts/gen-css.ts` turns them into custom
// properties at the top of app.css; every component stylesheet uses `var(--…)` and nothing else, so one edit here restyles the whole app.
//
// Inspiration: Apple's system look (SF blue, system fonts, large radii, continuous hairlines) and its glass material (a translucent, blurred, saturated
// surface with a bright inner edge), kept readable by giving each glass surface enough fill to hold text contrast.

/** Colours that change with the theme. Glass fills are translucent; `*-solid` is the fallback where backdrop blur is unavailable or reduced. */
export const themes = {
  dark: {
    bg: '#0b0c10',
    glass: 'rgba(30, 31, 38, 0.60)',
    'glass-strong': 'rgba(24, 25, 31, 0.78)',
    'glass-solid': '#1c1d24',
    'glass-edge': 'rgba(255, 255, 255, 0.16)',
    'glass-edge-soft': 'rgba(255, 255, 255, 0.07)',
    'glass-shine': 'rgba(255, 255, 255, 0.10)',
    hover: 'rgba(255, 255, 255, 0.08)',
    press: 'rgba(255, 255, 255, 0.14)',
    text: '#f5f5f7',
    'text-2': '#a1a1a8',
    'text-3': '#6e6e76',
    accent: '#0a84ff',
    'accent-text': '#ffffff',
    'accent-soft': 'rgba(10, 132, 255, 0.20)',
    ok: '#30d158',
    warn: '#ff9f0a',
    bad: '#ff453a',
    'code-bg': 'rgba(0, 0, 0, 0.35)',
    shadow: '0 18px 50px rgba(0, 0, 0, 0.45), 0 2px 8px rgba(0, 0, 0, 0.3)',
    'user-bubble': 'rgba(255, 255, 255, 0.10)',
  },
  light: {
    bg: '#e8eaf0',
    glass: 'rgba(255, 255, 255, 0.62)',
    'glass-strong': 'rgba(255, 255, 255, 0.82)',
    'glass-solid': '#f7f7fa',
    'glass-edge': 'rgba(255, 255, 255, 0.85)',
    'glass-edge-soft': 'rgba(0, 0, 0, 0.07)',
    'glass-shine': 'rgba(255, 255, 255, 0.6)',
    hover: 'rgba(0, 0, 0, 0.05)',
    press: 'rgba(0, 0, 0, 0.09)',
    text: '#1d1d1f',
    'text-2': '#5f5f66',
    'text-3': '#8e8e95',
    accent: '#007aff',
    'accent-text': '#ffffff',
    'accent-soft': 'rgba(0, 122, 255, 0.14)',
    ok: '#1f9d45',
    warn: '#c77700',
    bad: '#d70015',
    'code-bg': 'rgba(0, 0, 0, 0.06)',
    shadow: '0 18px 50px rgba(30, 40, 70, 0.18), 0 2px 8px rgba(30, 40, 70, 0.10)',
    'user-bubble': 'rgba(0, 0, 0, 0.06)',
  },
} as const;

/** Values shared by both themes. */
export const shared = {
  // Type. System UI first (that is SF Pro on Apple devices, with no licence to carry); Inter Variable elsewhere; JetBrains Mono for code, ids and numbers.
  'font-sans': "system-ui, -apple-system, 'SF Pro Text', 'Inter Variable', Inter, 'Segoe UI', Roboto, sans-serif",
  'font-mono': "'JetBrains Mono Variable', ui-monospace, 'SF Mono', Menlo, monospace",
  'text-xs': '11px',
  'text-sm': '12.5px',
  'text-md': '14px',
  'text-lg': '17px',
  'leading': '1.5',
  'tracking': '-0.011em',
  // Shape.
  'radius-sm': '8px',
  'radius-md': '12px',
  'radius-lg': '20px',
  'radius-xl': '28px',
  'radius-pill': '999px',
  // Glass.
  'blur': '28px',
  'saturate': '1.8',
  // Space.
  'gap-1': '4px',
  'gap-2': '8px',
  'gap-3': '12px',
  'gap-4': '16px',
  'gap-5': '24px',
  // Layout.
  'chat-width': '400px',
  'topbar-height': '52px',
  // Motion (Apple's standard ease-out curve).
  'ease': 'cubic-bezier(0.22, 1, 0.36, 1)',
  'speed': '0.22s',
} as const;

/** One hue per operator family, for the headers of the node graph. Unlisted families fall back to `other`. */
export const families = {
  solid: '#5e9bff',
  field: '#b07cff',
  poly: '#3ecfb2',
  xform: '#8e9aaf',
  anim: '#ffb340',
  rig: '#ff7a59',
  bake: '#e0a04a',
  light: '#ffd84d',
  scene: '#6fd3ff',
  street: '#9aa6b8',
  city: '#9aa6b8',
  building: '#c49a6c',
  terrain: '#7fcf6a',
  body: '#ff8fb4',
  curve: '#58d68d',
  physics: '#ff6b6b',
  other: '#9a9aa5',
} as const;
