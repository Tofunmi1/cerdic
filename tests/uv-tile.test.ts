import { expect, test } from 'bun:test';
import { worldTileUvs } from '../src/engine/texture/uv';

// A 4 x 4 floor at y = 0 and a 6 x 3 wall at z = 0, each two triangles.
const floor = { positions: new Float32Array([0, 0, 0, 4, 0, 0, 4, 0, 4, 0, 0, 4]), indices: new Uint32Array([0, 2, 1, 0, 3, 2]) };
const wall = { positions: new Float32Array([0, 0, 0, 6, 0, 0, 6, 3, 0, 0, 3, 0]), indices: new Uint32Array([0, 1, 2, 0, 2, 3]) };

test('a floor tiles by metres along X and Z', () => {
  const uvs = worldTileUvs(floor.positions, floor.indices, 2);
  // Corner order is 0, 2, 1 for the first triangle: (0,0), (4,4), (4,0) metres -> (0,0), (2,2), (2,0) tiles.
  expect(Array.from(uvs.slice(0, 6))).toEqual([0, 0, 2, 2, 2, 0]);
});

test('a wall tiles by metres along its length and up', () => {
  const uvs = worldTileUvs(wall.positions, wall.indices, 1.5);
  // First triangle: (0,0), (6,0), (6,3) metres -> (0,0), (4,0), (4,2) tiles.
  expect(Array.from(uvs.slice(0, 6))).toEqual([0, 0, 4, 0, 4, 2]);
});

test('the tile size is one repeat: halving it doubles the repeats, and bad input is refused', () => {
  const coarse = worldTileUvs(floor.positions, floor.indices, 2);
  const fine = worldTileUvs(floor.positions, floor.indices, 1);
  expect(Math.max(...fine)).toBe(Math.max(...coarse) * 2);
  expect(() => worldTileUvs(floor.positions, floor.indices, 0)).toThrow();
  expect(() => worldTileUvs(floor.positions, new Uint32Array([0, 1, 9]), 1)).toThrow();
});
