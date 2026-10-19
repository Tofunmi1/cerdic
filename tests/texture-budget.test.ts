import { expect, test } from 'bun:test';
import { fitToLayerBudget, VIEWPORT_LAYER_LIMIT, type MapName } from '../src/engine/texture/budget';

const full = () => ({ albedo: {}, normal: {}, metallicRoughness: {}, emissive: {} });
const mapsOf = (item: Partial<Record<MapName, unknown>>) => Object.keys(item).sort();

test('a scene inside the limit keeps every map', () => {
  const { kept, dropped } = fitToLayerBudget(Array.from({ length: VIEWPORT_LAYER_LIMIT / 4 }, full));
  expect(kept.every(item => mapsOf(item).length === 4)).toBe(true);
  expect(dropped.every(list => list.length === 0)).toBe(true);
});

test('too many fully textured parts: the later ones lose glow, shininess and bumps before any loses colour', () => {
  const parts = VIEWPORT_LAYER_LIMIT / 4 + 2; // two over: only glow maps of the last eight parts go, the first two are untouched
  const { kept, dropped } = fitToLayerBudget(Array.from({ length: parts }, full));
  const total = kept.reduce((sum, item) => sum + mapsOf(item).length, 0);
  expect(total).toBeLessThanOrEqual(VIEWPORT_LAYER_LIMIT);
  expect(total).toBeGreaterThan(VIEWPORT_LAYER_LIMIT - 4);
  expect(kept.every(item => item.albedo !== undefined)).toBe(true);
  // The first part is untouched; the last lost the most.
  expect(mapsOf(kept[0])).toHaveLength(4);
  expect(dropped[parts - 1].length).toBeGreaterThan(0);
  expect(dropped[0]).toEqual([]);
  // Emissive goes first: nobody loses normals while another part still has an emissive map.
  const lostNormals = dropped.some(list => list.includes('normal'));
  const keptEmissive = kept.some(item => item.emissive !== undefined);
  expect(lostNormals && keptEmissive).toBe(false);
});

test('colour is dropped only as a last resort, from the last part backwards', () => {
  const { kept, dropped } = fitToLayerBudget(Array.from({ length: 10 }, () => ({ albedo: {} })), 6);
  expect(kept.filter(item => item.albedo !== undefined)).toHaveLength(6);
  expect(dropped.slice(0, 6).every(list => list.length === 0)).toBe(true);
  expect(dropped.slice(6).every(list => list[0] === 'albedo')).toBe(true);
});

test('parts that share the same maps share their layers, so they count once', () => {
  const shared = full();
  const { kept, dropped } = fitToLayerBudget(Array.from({ length: 40 }, () => ({ ...shared })), 4);
  expect(kept.every(item => mapsOf(item).length === 4)).toBe(true);
  expect(dropped.every(list => list.length === 0)).toBe(true);
});

test('the inputs are not changed', () => {
  const items = [full(), full()];
  fitToLayerBudget(items, 3);
  expect(items.every(item => mapsOf(item).length === 4)).toBe(true);
});
