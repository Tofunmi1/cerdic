import { expect, test } from 'bun:test';
import { RD_PRESETS, grayScott, refusal, toPixels } from '../src/engine/texture/simulate';

const base = { size: 64, steps: 1500, ...RD_PRESETS.spots, seed: 1 };

test('the same settings give the same picture bit for bit, and another seed gives another', () => {
  const a = grayScott(base), b = grayScott(base), c = grayScott({ ...base, seed: 2 });
  expect(Array.from(a)).toEqual(Array.from(b));
  expect(Array.from(a)).not.toEqual(Array.from(c));
}, 60000);

test('a pattern grows from the specks: contrast, both high and low cells, not a blank or a noise', () => {
  const field = grayScott(base);
  let high = 0, low = 0;
  for (const v of field) { if (v > 0.7) high++; if (v < 0.1) low++; }
  expect(field.length).toBe(64 * 64);
  expect(high).toBeGreaterThan(40);
  expect(low).toBeGreaterThan(400);
  // Smooth like a chemical, not white noise: neighbouring cells differ little on average.
  let jump = 0;
  for (let y = 0; y < 64; y++) for (let x = 0; x < 63; x++) jump += Math.abs(field[y * 64 + x] - field[y * 64 + x + 1]);
  expect(jump / (64 * 63)).toBeLessThan(0.12);
});

test('the named patterns differ: stripes and mazes are not spots', () => {
  const fields = Object.fromEntries(['spots', 'stripes', 'maze'].map((name) => [name, grayScott({ ...base, steps: 3000, ...RD_PRESETS[name] })]));
  const area = (f: Float32Array) => f.filter((v) => v > 0.5).length / f.length;
  const [spots, stripes, maze] = [area(fields.spots), area(fields.stripes), area(fields.maze)];
  expect(new Set([spots.toFixed(2), stripes.toFixed(2), maze.toFixed(2)]).size).toBeGreaterThan(1);
  expect(Array.from(fields.spots)).not.toEqual(Array.from(fields.stripes));
}, 60000);

test('every named preset keeps a pattern alive (not a blank, not a flood) after a few thousand steps', () => {
  const dead: string[] = [];
  for (const [name, rates] of Object.entries(RD_PRESETS)) {
    const n = 64;
    const field = grayScott({ size: n, steps: 4000, ...rates, seed: 3 });
    let high = 0, edges = 0;
    for (let y = 0; y < n; y++) for (let x = 0; x < n; x++) {
      const v = field[y * n + x];
      if (v > 0.5) high++;
      if ((v > 0.5) !== (field[y * n + ((x + 1) % n)] > 0.5)) edges++;
      if ((v > 0.5) !== (field[((y + 1) % n) * n + x] > 0.5)) edges++;
    }
    const area = high / (n * n);
    if (!(area > 0.04 && area < 0.97 && edges > 150)) dead.push(`${name}: area ${area.toFixed(2)}, edges ${edges}`);
  }
  expect(dead).toEqual([]);
}, 60000);

test('the grid wraps, so the picture tiles: a speck on the edge grows across it', () => {
  const field = grayScott({ ...base, size: 32, steps: 400, seed: 7 });
  // Wrapped neighbours (column 31 and column 0) are about as alike as ordinary neighbours.
  let across = 0, inside = 0;
  for (let y = 0; y < 32; y++) { across += Math.abs(field[y * 32 + 31] - field[y * 32]); inside += Math.abs(field[y * 32 + 15] - field[y * 32 + 16]); }
  expect(across).toBeLessThan(inside * 3 + 1);
});

test('settings that cost too much or make no sense are refused with the reason', () => {
  expect(refusal({ ...base, size: 600 })).toContain('size');
  expect(refusal({ ...base, steps: 0 })).toContain('steps');
  expect(refusal({ ...base, size: 512, steps: 5000 })).toContain('million cell updates');
  expect(refusal({ ...base, feed: 0 })).toContain('feed');
  expect(refusal({ ...base, kill: 0.5 })).toContain('kill');
  expect(refusal(base)).toBeUndefined();
  expect(() => grayScott({ ...base, size: 7 })).toThrow('reaction_diffusion');
});

test('pixels are opaque grey from the field', () => {
  const pixels = toPixels(new Float32Array([0, 0.5, 1]));
  expect(Array.from(pixels)).toEqual([0, 0, 0, 255, 128, 128, 128, 255, 255, 255, 255, 255]);
});
