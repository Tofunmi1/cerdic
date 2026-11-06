import { expect, test } from 'bun:test';
// @ts-expect-error plain module with no declarations
import { firstPad, readPad, startGamepad, stick } from '../engine/web/gamepad.js';

const pad = (axes: number[], pressed: number[] = [], mapping = 'standard') => ({
  connected: true,
  mapping,
  axes,
  buttons: Array.from({ length: 17 }, (_, i) => ({ pressed: pressed.includes(i) })),
});

test('a stick has a round dead zone and rescales past it', () => {
  expect(stick(0.1, 0.1)).toEqual([0, 0]);
  const [x, y] = stick(1, 0);
  expect(x).toBeCloseTo(1, 6);
  expect(y).toBe(0);
  const [a] = stick(0.2, 0); // just past the 0.18 zone: a small motion, not a jump
  expect(a).toBeGreaterThan(0);
  expect(a).toBeLessThan(0.05);
  const [dx, dy] = stick(0.7, 0.7);
  expect(Math.hypot(dx, dy)).toBeLessThanOrEqual(1);
});

test('the left stick walks, A and B rise and sink, the stick press sprints', () => {
  const r = readPad(pad([0, -1, 0, 0], [0, 10]), 0.016);
  expect(r.move).toEqual({ right: 0, up: 1, forward: 1, sprint: true });
  const s = readPad(pad([1, 0, 0, 0], [1]), 0.016);
  expect(s.move.right).toBeCloseTo(1, 6);
  expect(s.move.up).toBe(-1);
  expect(s.move.sprint).toBe(false);
});

test('the right stick looks, in mouse pixels scaled by the frame time', () => {
  const r = readPad(pad([0, 0, 1, 0]), 0.1);
  expect(r.look.dx).toBeCloseTo(90, 5);
  expect(r.look.dy).toBe(0);
  expect(readPad(pad([0, 0, 0, 0]), 0.1).look).toEqual({ dx: 0, dy: 0 });
});

test('triggers drive the desktop hand: right trigger pulls, a bumper or left trigger grips', () => {
  expect(readPad(pad([0, 0, 0, 0], [7]), 0.016).hand).toEqual({ trigger: true, grip: false });
  expect(readPad(pad([0, 0, 0, 0], [5]), 0.016).hand).toEqual({ trigger: false, grip: true });
  expect(readPad(pad([0, 0, 0, 0], [6]), 0.016).hand.grip).toBe(true);
  expect(readPad(pad([0, 0, 0, 0]), 0.016).hand).toEqual({ trigger: false, grip: false });
});

test('only a connected controller with the standard mapping is used', () => {
  expect(firstPad([null, pad([], [], '')])).toBeNull();
  expect(firstPad([{ ...pad([]), connected: false }])).toBeNull();
  const good = pad([]);
  expect(firstPad([null, good])).toBe(good);
  expect(firstPad(null)).toBeNull();
});

test('startGamepad sends motion while wanted, stops it once, and sends the hand only when it changes', () => {
  const calls: string[] = [];
  const api = {
    viewer_move: (...a: unknown[]) => calls.push(`move ${a.join(',')}`),
    viewer_look: (...a: unknown[]) => calls.push(`look ${(a as number[]).map((v) => v.toFixed(1)).join(',')}`),
    game_desktop_hand: (...a: unknown[]) => calls.push(`hand ${a.join(',')}`),
  };
  let current = pad([0, -1, 0, 0], [7]);
  let clock = 0;
  const frames: Array<() => void> = [];
  const stop = startGamepad(api, { getPads: () => [current], raf: (f: () => void) => { frames.push(f); return frames.length; }, now: () => (clock += 16) });
  frames.shift()!(); // frame 1: walking forward and pulling the trigger
  frames.shift()!(); // frame 2: same
  current = pad([0, 0, 0, 0], []); // released everything
  frames.shift()!(); // frame 3: stop the motion once, release the trigger
  frames.shift()!(); // frame 4: nothing more
  expect(calls).toEqual(['move 0,0,1,false', 'hand false,true', 'move 0,0,1,false', 'move 0,0,0,false', 'hand false,false']);
  stop();
});
