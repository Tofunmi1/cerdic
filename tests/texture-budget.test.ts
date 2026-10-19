import { expect, test } from 'bun:test';
import {
  fitToBudget, halveRgba, mapBytes, mapSide, snapMapSize, VIEWPORT_BUDGET_BYTES, type MapName,
} from '../src/engine/texture/budget';

const map = (side: number, fill = 0) => new Uint8Array(side * side * 4).fill(fill);
const full = (side: number) => ({ albedo: map(side), normal: map(side), metallicRoughness: map(side), emissive: map(side) });
const NAMES: MapName[] = ['albedo', 'normal', 'metallicRoughness', 'emissive'];
const sides = (item: Partial<Record<MapName, Uint8Array>>) => NAMES.map(name => (item[name] ? mapSide(item[name]) : 0));
const costOf = (items: Partial<Record<MapName, Uint8Array>>[]) => {
  const distinct = new Set<Uint8Array>();
  for (const item of items) for (const name of NAMES) if (item[name]) distinct.add(item[name]);
  return [...distinct].reduce((sum, m) => sum + mapBytes(mapSide(m)), 0);
};

test('the byte numbers match the renderer: a mip chain is a third more than the top level', () => {
  expect(mapBytes(1024)).toBe((4 ** 11 - 1) / 3 * 4);
  expect(mapBytes(256) * 4).toBeGreaterThan(mapBytes(512) - 16);
  expect(mapBytes(256) * 4).toBeLessThan(mapBytes(512) + 16);
  expect(VIEWPORT_BUDGET_BYTES).toBe(192 * 1024 * 1024);
  // The renderer's old limit of 32 large maps still fits.
  expect(32 * mapBytes(1024)).toBeLessThan(VIEWPORT_BUDGET_BYTES);
});

test('a requested size snaps down to one the renderer holds', () => {
  expect([2048, 1024, 800, 512, 400, 256, 100, 16].map(snapMapSize)).toEqual([1024, 1024, 512, 512, 256, 256, 256, 256]);
  expect(snapMapSize(Number.NaN)).toBe(1024);
});

test('halving averages two by two blocks and keeps alpha', () => {
  const source = new Uint8Array([
    0, 0, 0, 255, 100, 100, 100, 255,
    200, 200, 200, 255, 100, 100, 100, 255,
  ]);
  expect([...halveRgba(source, 2)]).toEqual([100, 100, 100, 255]);
  const half = halveRgba(map(8, 40), 8);
  expect(mapSide(half)).toBe(4);
  expect(half.every(v => v === 40)).toBe(true);
});

test('a scene inside the budget keeps every map, untouched', () => {
  const items = Array.from({ length: 4 }, () => full(1024));
  const { kept, dropped, shrunk } = fitToBudget(items, 100 * mapBytes(1024));
  expect(kept.every((item, i) => NAMES.every(name => item[name] === items[i][name]))).toBe(true);
  expect(dropped.flat()).toEqual([]);
  expect(shrunk.flat()).toEqual([]);
});

test('over budget, maps shrink before any is dropped, last surfaces and least useful maps first', () => {
  const items = Array.from({ length: 6 }, () => full(1024));
  // 24 large maps against room for 9 of them.
  const budget = 9 * mapBytes(1024);
  const { kept, dropped, shrunk } = fitToBudget(items, budget);
  expect(costOf(kept)).toBeLessThanOrEqual(budget);
  expect(dropped.flat()).toEqual([]);
  // Nothing is dropped, every map is still there, and the first surface is the sharpest.
  expect(kept.every(item => NAMES.every(name => item[name] !== undefined))).toBe(true);
  const total = (item: Partial<Record<MapName, Uint8Array>>) => sides(item).reduce((a, b) => a + b, 0);
  expect(total(kept[0])).toBeGreaterThanOrEqual(total(kept[5]));
  expect(shrunk[5].length).toBeGreaterThan(0);
  // Within one surface, glow shrinks before colour does.
  const first = kept.find((_, i) => shrunk[i].length > 0 && shrunk[i].length < 4);
  if (first) {
    expect(mapSide(first.emissive!)).toBeLessThanOrEqual(mapSide(first.albedo!));
  }
  // What a shrunk map reports is true.
  for (const [i, list] of shrunk.entries()) for (const { name, from, to } of list) {
    expect(from).toBe(1024);
    expect(mapSide(kept[i][name]!)).toBe(to);
    expect(to).toBeLessThan(from);
  }
});

test('maps are never shrunk below 256, and only then are maps dropped, from the last surface backwards', () => {
  const items = Array.from({ length: 10 }, () => ({ albedo: map(256) }));
  const { kept, dropped } = fitToBudget(items, 6 * mapBytes(256));
  expect(kept.filter(item => item.albedo !== undefined)).toHaveLength(6);
  expect(dropped.slice(0, 6).every(list => list.length === 0)).toBe(true);
  expect(dropped.slice(6).every(list => list[0] === 'albedo')).toBe(true);
  // A smaller map stays as it is: it does not grow back, and it is not shrunk again.
  const small = fitToBudget([{ albedo: map(256) }], 10 * mapBytes(256));
  expect(mapSide(small.kept[0].albedo!)).toBe(256);
});

test('colour is dropped last: glow, shininess and bumps go before it', () => {
  const items = Array.from({ length: 4 }, () => full(256));
  const { kept, dropped } = fitToBudget(items, 6 * mapBytes(256));
  expect(costOf(kept)).toBeLessThanOrEqual(6 * mapBytes(256));
  expect(kept.every(item => item.albedo !== undefined)).toBe(true);
  const lostNormals = dropped.some(list => list.includes('normal'));
  const keptEmissive = kept.some(item => item.emissive !== undefined);
  expect(lostNormals && keptEmissive).toBe(false);
  // Channels go one kind at a time across the scene, last surface first, so later surfaces never keep more than earlier ones.
  expect(dropped[3].length).toBeGreaterThanOrEqual(dropped[0].length);
  expect(kept[0].normal !== undefined || kept[3].normal === undefined).toBe(true);
});

test('parts that share a map count it once and shrink it together', () => {
  const shared = full(1024);
  const items = Array.from({ length: 40 }, () => ({ ...shared }));
  // Four maps fit at full size: forty parts sharing them cost no more.
  const same = fitToBudget(items, 4 * mapBytes(1024));
  expect(same.kept.every(item => item.albedo === shared.albedo)).toBe(true);
  expect(same.shrunk.flat()).toEqual([]);
  // With room for only two large maps' worth, the shared maps shrink, and stay shared.
  const tight = fitToBudget(items, 2 * mapBytes(1024));
  expect(costOf(tight.kept)).toBeLessThanOrEqual(2 * mapBytes(1024));
  expect(new Set(tight.kept.map(item => item.albedo)).size).toBe(1);
  expect(new Set(tight.kept.map(item => item.emissive)).size).toBe(1);
  expect(tight.dropped.flat()).toEqual([]);
});

test('the inputs are not changed', () => {
  const items = [full(1024), full(1024)];
  const before = items.map(item => ({ ...item }));
  fitToBudget(items, 3 * mapBytes(1024));
  expect(items.every((item, i) => NAMES.every(name => item[name] === before[i][name] && mapSide(item[name]) === 1024))).toBe(true);
});
