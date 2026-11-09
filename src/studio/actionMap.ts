// The action map (F6): a game declares inputs (`input NAME axis=x,z keys=KeyD,KeyA,KeyW,KeyS pad=stick:left label=Move`), the runtime exports the
// table (`game_input_map`), and this module turns what the devices are doing into the axes and presses the game reads. Pure: no DOM, no storage
// except the two small helpers at the end, so it is tested without a browser.
//
// What the game receives: for an axis input, an axis `NAME.AXIS` per axis (-1 to 1); for a press input, an axis `NAME` that is 1 while the key
// is down (so a jump can tell a hold from a tap) and one press on each rise. A player's rebinding replaces the keys of one action.

export type Row = { name: string; kind: 'axis' | 'press' | 'other'; axes: string[]; keys: string[]; pad: string[]; label: string };
export type PadState = { axes: number[]; buttons: { pressed: boolean; value: number }[] };
export type Device = { down: Set<string>; pad: PadState | null };
export type Overrides = Record<string, { keys?: string[]; pad?: string[] }>;

const DEAD_ZONE = 0.18;

/** The runtime's table, or an empty one for nothing or something that is not a table. */
export function parseTable(text: string): Row[] {
  try {
    const value: unknown = JSON.parse(text);
    return Array.isArray(value) ? (value as Row[]).filter(r => r && typeof r.name === 'string' && Array.isArray(r.keys) && Array.isArray(r.pad)) : [];
  } catch {
    return [];
  }
}

const DEFAULTS: Record<string, { keys: string[]; pad: string[] }> = {
  move: { keys: ['KeyD', 'KeyA', 'KeyW', 'KeyS'], pad: ['stick:left'] },
  jump: { keys: ['Space', 'KeyC'], pad: ['button0'] },
};

/** The table with the player's rebinding applied, and the standard bindings for `move` and `jump` where the game named none. */
export function withDefaults(rows: Row[], overrides: Overrides = {}): Row[] {
  return rows.map(row => {
    const over = overrides[row.name];
    let { keys, pad } = row;
    if (over?.keys) keys = over.keys;
    if (over?.pad) pad = over.pad;
    if (!over && keys.length === 0 && pad.length === 0) {
      const standard = DEFAULTS[row.name];
      // the standard move binding is for two axes; other shapes are left to the game
      if (standard && (row.name !== 'move' || row.axes.length === 2)) ({ keys, pad } = standard);
    }
    return { ...row, keys, pad };
  });
}

const dead = (v: number) => (Math.abs(v) < DEAD_ZONE ? 0 : v);
const clean = (v: number) => (v === 0 ? 0 : Math.max(-1, Math.min(1, v)));

function padValue(controls: string[], pad: PadState, index: number): number {
  let value = 0;
  for (const control of controls) {
    if (control === 'stick:left') value = value || (index === 0 ? dead(pad.axes[0] ?? 0) : index === 1 ? -dead(pad.axes[1] ?? 0) : 0);
    else if (control === 'stick:right') value = value || (index === 0 ? dead(pad.axes[2] ?? 0) : index === 1 ? -dead(pad.axes[3] ?? 0) : 0);
    else if (/^axis\d+[+-]$/.test(control)) {
      const raw = dead(pad.axes[Number(control.slice(4, -1))] ?? 0);
      value = value || (control.endsWith('+') ? Math.max(0, raw) : Math.min(0, raw));
    }
  }
  return value;
}

/** The axes to send and which press inputs are held, from what the devices are doing now. */
export function resolve(rows: Row[], device: Device): { axes: Record<string, number>; held: Record<string, boolean> } {
  const axes: Record<string, number> = {};
  const held: Record<string, boolean> = {};
  for (const row of rows) {
    if (row.kind === 'axis') {
      row.axes.forEach((axis, i) => {
        const keyed = (device.down.has(row.keys[2 * i] ?? '') ? 1 : 0) - (device.down.has(row.keys[2 * i + 1] ?? '') ? 1 : 0);
        const value = keyed !== 0 ? keyed : device.pad ? padValue(row.pad, device.pad, i) : 0;
        axes[`${row.name}.${axis}`] = clean(value);
      });
    } else if (row.kind === 'press') {
      const keyDown = row.keys.some(key => device.down.has(key));
      const buttonDown = device.pad ? row.pad.some(control => /^button\d+$/.test(control) && !!device.pad?.buttons[Number(control.slice(6))]?.pressed) : false;
      held[row.name] = keyDown || buttonDown;
      axes[row.name] = held[row.name] ? 1 : 0;
    }
  }
  return { axes, held };
}

/** The names that went from up to down between two readings. */
export function edges(before: Record<string, boolean>, now: Record<string, boolean>): string[] {
  return Object.keys(now).filter(name => now[name] && !before[name]);
}

/** A player's new keys for an action (null restores the game's own). An axis needs a key for each direction, or nothing changes. */
export function rebind(overrides: Overrides, name: string, keys: string[] | null, row?: Row): Overrides {
  if (keys === null) {
    const { [name]: _removed, ...rest } = overrides;
    return rest;
  }
  if (row?.kind === 'axis' && keys.length !== row.axes.length * 2) return overrides;
  return { ...overrides, [name]: { ...overrides[name], keys } };
}

const NAMES: Record<string, string> = {
  Space: 'Space', Enter: 'Enter', Escape: 'Esc', Tab: 'Tab', Backspace: 'Backspace', ShiftLeft: 'Shift', ShiftRight: 'Shift',
  ControlLeft: 'Ctrl', ControlRight: 'Ctrl', AltLeft: 'Alt', AltRight: 'Alt', ArrowUp: '↑', ArrowDown: '↓', ArrowLeft: '←', ArrowRight: '→',
  Mouse0: 'Left click', Mouse1: 'Middle click', Mouse2: 'Right click', 'stick:left': 'Left stick', 'stick:right': 'Right stick',
};
const PAD_BUTTONS = ['A', 'B', 'X', 'Y', 'LB', 'RB', 'LT', 'RT', 'Back', 'Start', 'L3', 'R3', 'D-pad ↑', 'D-pad ↓', 'D-pad ←', 'D-pad →'];

/** What a key or control is called on a keycap or a controller. */
export function glyph(code: string): string {
  if (NAMES[code]) return NAMES[code];
  if (/^Key[A-Z]$/.test(code)) return code.slice(3);
  if (/^Digit\d$/.test(code)) return code.slice(5);
  if (/^Numpad\d$/.test(code)) return `Num ${code.slice(6)}`;
  const button = /^button(\d+)$/.exec(code);
  if (button) return PAD_BUTTONS[Number(button[1])] ?? `Button ${button[1]}`;
  const axis = /^axis(\d+)([+-])$/.exec(code);
  if (axis) return `Axis ${axis[1]}${axis[2]}`;
  return code;
}

/** The text of an on-screen prompt for an action on the keyboard or on a pad (empty when nothing is bound there). */
export function prompt(row: Row, device: 'keyboard' | 'pad'): string {
  if (device === 'pad') return [...new Set(row.pad.map(glyph))].join(' / ');
  if (row.kind !== 'axis') return row.keys.map(glyph).join(' / ');
  // the keys of an axis input read like the keys of a keyboard cluster: up, left, down, right
  const order = row.axes.length === 2 ? [2, 1, 3, 0] : row.axes.length === 1 ? [1, 0] : row.keys.map((_, i) => i);
  return order.map(i => row.keys[i]).filter(Boolean).map(glyph).join(' ');
}

/** The menu keys every game has: the arrows and d-pad move the focus between buttons, Enter and A accept (see ADR 0074). */
export const UI_ROWS: Row[] = [
  { name: 'ui_next', kind: 'press', axes: [], keys: ['ArrowDown', 'ArrowRight'], pad: ['button13', 'button15'], label: 'Next' },
  { name: 'ui_prev', kind: 'press', axes: [], keys: ['ArrowUp', 'ArrowLeft'], pad: ['button12', 'button14'], label: 'Previous' },
  { name: 'ui_accept', kind: 'press', axes: [], keys: ['Enter', 'NumpadEnter'], pad: ['button0'], label: 'Accept' },
];

/** What each declared action is called on a device, for `{key:ACTION}` in the game's texts; actions with nothing bound there are left out. */
export function labelsFor(rows: Row[], device: 'keyboard' | 'pad'): Record<string, string> {
  const labels: Record<string, string> = {};
  for (const row of rows) {
    const text = prompt(row, device);
    if (text) labels[row.name] = text;
  }
  return labels;
}

export type ButtonBox = { id: string; x: number; y: number; w: number; h: number };

/** The button under a point given in pixels from the canvas's top-left; `unit` is the canvas height in pixels (one screen height). */
export function hitButton(buttons: ButtonBox[], px: number, py: number, unit: number): string | undefined {
  const [x, y] = [px / unit, py / unit];
  return [...buttons].reverse().find(b => x >= b.x && x <= b.x + b.w && y >= b.y && y <= b.y + b.h)?.id;
}

const STORE = 've.bindings';

/** The player's saved rebinding. Storage can be missing or blocked: then there is none. */
export function loadOverrides(): Overrides {
  try {
    const value: unknown = JSON.parse(localStorage.getItem(STORE) ?? '{}');
    return value && typeof value === 'object' && !Array.isArray(value) ? (value as Overrides) : {};
  } catch {
    return {};
  }
}

export function saveOverrides(overrides: Overrides): void {
  try {
    localStorage.setItem(STORE, JSON.stringify(overrides));
  } catch {
    /* private window or blocked storage: the rebinding lasts until the page closes */
  }
}
