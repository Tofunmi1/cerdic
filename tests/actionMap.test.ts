// The action map (F6): a game declares inputs with keys and gamepad controls; the page turns the devices into axes and presses from that table.
import { describe, expect, test } from 'bun:test';
import { type Device, type Row, UI_ROWS, edges, glyph, hitButton, labelsFor, parseTable, prompt, rebind, resolve, withDefaults } from '../src/studio/actionMap';

const move: Row = { name: 'move', kind: 'axis', axes: ['x', 'z'], keys: ['KeyD', 'KeyA', 'KeyW', 'KeyS'], pad: ['stick:left'], label: 'Move' };
const jump: Row = { name: 'jump', kind: 'press', axes: [], keys: ['Space', 'KeyC'], pad: ['button0'], label: 'Jump' };
const idle: Device = { down: new Set(), pad: null };
const pad = (axes: number[], pressed: number[] = []): Device => ({
  down: new Set(),
  pad: { axes, buttons: Array.from({ length: 16 }, (_, i) => ({ pressed: pressed.includes(i), value: pressed.includes(i) ? 1 : 0 })) },
});

describe('resolve', () => {
  test('keys give -1, 0 or 1 on each axis, and opposite keys cancel', () => {
    const out = (keys: string[]) => resolve([move], { down: new Set(keys), pad: null }).axes;
    expect(out(['KeyD'])).toEqual({ 'move.x': 1, 'move.z': 0 });
    expect(out(['KeyA', 'KeyW'])).toEqual({ 'move.x': -1, 'move.z': 1 });
    expect(out(['KeyS'])['move.z']).toBe(-1);
    expect(out(['KeyD', 'KeyA'])['move.x']).toBe(0);
  });
  test('a stick moves the axes past a dead zone, and up is +z', () => {
    const axes = resolve([move], pad([0.8, -0.9])).axes;
    expect(axes['move.x']).toBeCloseTo(0.8);
    expect(axes['move.z']).toBeCloseTo(0.9);
    expect(resolve([move], pad([0.1, -0.1])).axes).toEqual({ 'move.x': 0, 'move.z': 0 });
  });
  test('keys win over a stick that is at rest, and a stick fills in when no key is down', () => {
    expect(resolve([move], { down: new Set(['KeyA']), pad: pad([0, 0]).pad }).axes['move.x']).toBe(-1);
  });
  test('a press input is held while a key or a button is down', () => {
    expect(resolve([jump], idle).held.jump).toBe(false);
    expect(resolve([jump], { down: new Set(['Space']), pad: null }).held.jump).toBe(true);
    expect(resolve([jump], { down: new Set(['KeyC']), pad: null }).axes.jump).toBe(1);
    expect(resolve([jump], pad([0, 0, 0, 0], [0])).held.jump).toBe(true);
    expect(resolve([jump], pad([0, 0, 0, 0], [1])).held.jump).toBe(false);
  });
  test('one-axis pad controls give each direction', () => {
    const steer: Row = { name: 'steer', kind: 'axis', axes: ['x'], keys: [], pad: ['axis0+', 'axis0-'], label: 'Steer' };
    expect(resolve([steer], pad([0.7])).axes['steer.x']).toBeCloseTo(0.7);
    expect(resolve([steer], pad([-0.6])).axes['steer.x']).toBeCloseTo(-0.6);
  });
});

describe('edges', () => {
  test('only a rise from up to down is a press', () => {
    expect(edges({ jump: false }, { jump: true })).toEqual(['jump']);
    expect(edges({ jump: true }, { jump: true })).toEqual([]);
    expect(edges({ jump: true }, { jump: false })).toEqual([]);
    expect(edges({}, { jump: true, fire: true }).sort()).toEqual(['fire', 'jump']);
  });
});

describe('table', () => {
  test('the runtime table parses, and nothing or garbage is an empty table', () => {
    expect(parseTable(JSON.stringify([move, jump]))).toHaveLength(2);
    expect(parseTable('')).toEqual([]);
    expect(parseTable('not json')).toEqual([]);
    expect(parseTable('{"a":1}')).toEqual([]);
  });
  test('inputs with no bindings get the defaults for move and jump, and nothing else is invented', () => {
    const bare = (name: string, kind: Row['kind'], axes: string[] = []): Row => ({ name, kind, axes, keys: [], pad: [], label: name });
    const [m, j, other] = withDefaults([bare('move', 'axis', ['x', 'z']), bare('jump', 'press'), bare('fire', 'press')]);
    expect(m.keys).toEqual(['KeyD', 'KeyA', 'KeyW', 'KeyS']);
    expect(m.pad).toEqual(['stick:left']);
    expect(j.keys).toContain('Space');
    expect(j.pad).toEqual(['button0']);
    expect(other.keys).toEqual([]);
    expect(withDefaults([move])[0]).toEqual(move);
  });
});

describe('rebinding', () => {
  test('an override replaces the keys of one action and leaves the rest', () => {
    const rows = withDefaults([move, jump], rebind({}, 'jump', ['KeyZ']));
    expect(rows[1].keys).toEqual(['KeyZ']);
    expect(rows[0].keys).toEqual(move.keys);
  });
  test('an axis override must have a key for each direction', () => {
    expect(rebind({}, 'move', ['KeyL', 'KeyJ'], move)).toEqual({});
    expect(rebind({}, 'move', ['ArrowRight', 'ArrowLeft', 'ArrowUp', 'ArrowDown'], move).move?.keys).toHaveLength(4);
  });
  test('clearing an override restores the game defaults', () => {
    const changed = rebind({}, 'jump', ['KeyZ']);
    expect(rebind(changed, 'jump', null)).toEqual({});
  });
});

describe('menus', () => {
  test('the menu keys are always there: arrows and d-pad move, Enter and A accept', () => {
    const names = UI_ROWS.map(r => r.name);
    expect(names).toEqual(['ui_next', 'ui_prev', 'ui_accept']);
    const held = resolve(UI_ROWS, { down: new Set(['ArrowDown']), pad: null }).held;
    expect(held).toEqual({ ui_next: true, ui_prev: false, ui_accept: false });
    expect(resolve(UI_ROWS, pad([0, 0, 0, 0], [0])).held.ui_accept).toBe(true);
    expect(resolve(UI_ROWS, pad([0, 0, 0, 0], [12])).held.ui_prev).toBe(true);
  });
  test('labels name each declared action for the device in use', () => {
    expect(labelsFor([move, jump], 'keyboard')).toEqual({ move: 'W A S D', jump: 'Space / C' });
    expect(labelsFor([move, jump], 'pad')).toEqual({ move: 'Left stick', jump: 'A' });
    expect(labelsFor([{ ...jump, keys: [], pad: [] }], 'keyboard')).toEqual({});
  });
  test('a click lands on the button under it, in screen heights', () => {
    const buttons = [{ id: 'play', x: 0.4, y: 0.4, w: 0.3, h: 0.06 }, { id: 'quit', x: 0.4, y: 0.5, w: 0.3, h: 0.06 }];
    // a 1600 x 800 canvas: one screen height is 800 px
    expect(hitButton(buttons, 0.55 * 800, 0.43 * 800, 800)).toBe('play');
    expect(hitButton(buttons, 0.55 * 800, 0.53 * 800, 800)).toBe('quit');
    expect(hitButton(buttons, 0.55 * 800, 0.47 * 800, 800)).toBeUndefined();
    expect(hitButton([], 100, 100, 800)).toBeUndefined();
  });
});

describe('prompts', () => {
  test('glyphs read like keycaps', () => {
    expect(glyph('KeyW')).toBe('W');
    expect(glyph('Digit3')).toBe('3');
    expect(glyph('ArrowUp')).toBe('↑');
    expect(glyph('Space')).toBe('Space');
    expect(glyph('ShiftLeft')).toBe('Shift');
    expect(glyph('Mouse0')).toBe('Left click');
    expect(glyph('button0')).toBe('A');
    expect(glyph('button1')).toBe('B');
    expect(glyph('stick:left')).toBe('Left stick');
  });
  test('a prompt names the keys on the keyboard and the control on a pad', () => {
    expect(prompt(jump, 'keyboard')).toBe('Space / C');
    expect(prompt(jump, 'pad')).toBe('A');
    expect(prompt(move, 'keyboard')).toBe('W A S D');
    expect(prompt(move, 'pad')).toBe('Left stick');
    expect(prompt({ ...jump, keys: [], pad: [] }, 'keyboard')).toBe('');
  });
});
