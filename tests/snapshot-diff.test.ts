import { expect, test } from 'bun:test';
import { describeDiff, diffImages } from '../src/engine/snapshot-diff';

const solid = (w: number, h: number, rgb: [number, number, number]) => {
  const out = new Uint8ClampedArray(w * h * 4);
  for (let i = 0; i < w * h; i++) out.set([...rgb, 255], i * 4);
  return out;
};
const paint = (image: Uint8ClampedArray, w: number, x0: number, y0: number, x1: number, y1: number, rgb: [number, number, number]) => {
  for (let y = y0; y < y1; y++) for (let x = x0; x < x1; x++) image.set([...rgb, 255], (y * w + x) * 4);
};

test('identical images have no change', () => {
  const a = solid(30, 30, [10, 20, 30]);
  const diff = diffImages(a, a.slice(), 30, 30);
  expect(diff.changedFraction).toBe(0);
  expect(diff.bbox).toBeNull();
  expect(diff.meanShift).toBe(0);
  expect(describeDiff(diff)).toBe('the view is unchanged');
});

test('a changed block reports its fraction, its box and where it is', () => {
  const a = solid(30, 30, [0, 0, 0]);
  const b = a.slice();
  paint(b, 30, 20, 0, 30, 10, [255, 0, 0]); // the top-right cell, 100 of 900 pixels
  const diff = diffImages(a, b, 30, 30);
  expect(diff.changedFraction).toBeCloseTo(100 / 900, 9);
  expect(diff.bbox).toEqual([20 / 30, 0, 1, 10 / 30]);
  expect(diff.grid[2]).toBe(1);
  expect(diff.grid.reduce((s, v, i) => (i === 2 ? s : s + v), 0)).toBe(0);
  expect(diff.meanShift).toBeCloseTo((100 * 255) / (900 * 3 * 255), 9);
  const text = describeDiff(diff);
  expect(text).toContain('11.1%');
  expect(text).toContain('top right');
  expect(text).toContain('[67, 0, 100, 33]');
});

test('small colour noise below the threshold is not a change', () => {
  const a = solid(20, 20, [100, 100, 100]);
  const b = solid(20, 20, [110, 95, 100]);
  expect(diffImages(a, b, 20, 20).changedFraction).toBe(0);
  expect(diffImages(a, b, 20, 20, 4).changedFraction).toBe(1);
});

test('the grid names the busiest cells, most changed first', () => {
  const a = solid(30, 30, [0, 0, 0]);
  const b = a.slice();
  paint(b, 30, 0, 20, 10, 30, [255, 255, 255]);
  paint(b, 30, 10, 10, 20, 15, [255, 255, 255]);
  const text = describeDiff(diffImages(a, b, 30, 30));
  expect(text).toContain('mostly bottom left, centre');
});

test('images of different sizes are refused, not compared', () => {
  expect(() => diffImages(solid(4, 4, [0, 0, 0]), solid(5, 4, [0, 0, 0]), 4, 4)).toThrow('RGBA');
  expect(() => diffImages(new Uint8ClampedArray(0), new Uint8ClampedArray(0), 0, 0)).toThrow('RGBA');
});
