import { expect, test } from 'bun:test';
import validator from 'gltf-validator';
import { buildGlb } from '../src/engine/gltf';
import type { MeshPart } from '../src/engine/worker-types';

const box = (size = 1) => ({
  positions: new Float32Array([-1,-1,-1, 1,-1,-1, 1,1,-1, -1,1,-1, -1,-1,1, 1,-1,1, 1,1,1, -1,1,1].map(v => v * size)),
  indices: new Uint32Array([0,2,1,0,3,2,4,5,6,4,6,7,0,1,5,0,5,4,1,2,6,1,6,5,2,3,7,2,7,6,3,0,4,3,4,7]),
});
const pyramid = () => ({ positions: new Float32Array([0,1,0,-1,0,-1,1,0,-1,1,0,1,-1,0,1]), indices: new Uint32Array([1,2,3,1,3,4,1,0,2,2,0,3,3,0,4,4,0,1,0,2,4,0,4,3]) });
const tetra = () => ({ positions: new Float32Array([1,1,1,-1,-1,1,-1,1,-1,1,-1,-1]), indices: new Uint32Array([0,1,2,0,3,1,0,2,3,1,3,2]) });
const part = (name: string, mesh = box()): MeshPart => ({ node: name, name, volume: 1, occlusion: new Float32Array(mesh.positions.length / 3), ...mesh });
const jsonOf = (parts: MeshPart[]) => {
  const glb = buildGlb(parts), length = new DataView(glb.buffer, glb.byteOffset).getUint32(12, true);
  return { glb, json: JSON.parse(new TextDecoder().decode(glb.subarray(20, 20 + length))) };
};

test('LOD levels have normalized names, level metadata, and group count', () => {
  const { json } = jsonOf([part('rock_lod0', box(1)), part('rock_lod1', box(0.7)), part('rock_lod2', box(0.4))]);
  expect(json.nodes.map((node: any) => node.name)).toEqual(['rock_LOD0', 'rock_LOD1', 'rock_LOD2']);
  expect(json.nodes.map((node: any) => [node.extras.lod.level, node.extras.lod.levels])).toEqual([[0,3],[1,3],[2,3]]);
});

test('level zero references only the lower levels and declares MSFT_lod', () => {
  const { json } = jsonOf([part('rock_lod0'), part('rock_lod1'), part('rock_lod2')]);
  expect(json.nodes[0].extensions.MSFT_lod.ids).toEqual([1, 2]);
  expect(json.extensionsUsed).toContain('MSFT_lod');
});

test('screen coverage decreases by half per LOD level', () => {
  const { json } = jsonOf([part('rock_lod0'), part('rock_lod1'), part('rock_lod2')]);
  expect(json.nodes[0].extras.lod.screenCoverage).toEqual([1, 0.5, 0.25]);
});

test('triangle metadata reports real counts for each LOD mesh', () => {
  const { json } = jsonOf([part('rock_lod0'), part('rock_lod1', pyramid()), part('rock_lod2', tetra())]);
  expect(json.nodes.map((node: any) => node.extras.lod.triangles)).toEqual([12, 8, 4]);
});

test('a missing level is preserved and warned about', () => {
  const { json } = jsonOf([part('bush_lod0'), part('bush_lod2')]);
  expect(json.nodes.map((node: any) => node.name)).toEqual(['bush_LOD0', 'bush_LOD2']);
  expect(json.asset.extras.warnings).toHaveLength(1);
  expect(json.asset.extras.warnings[0].toLowerCase()).toContain('bush');
  expect(json.asset.extras.warnings[0].toLowerCase()).toContain('missing');
});

test('duplicate levels are preserved and warned about', () => {
  const { json } = jsonOf([part('tree_lod1'), part('tree_lod1')]);
  expect(json.nodes).toHaveLength(2);
  expect(json.asset.extras.warnings).toHaveLength(1);
  expect(json.asset.extras.warnings[0].toLowerCase()).toContain('tree');
  expect(/twice|duplicate/i.test(json.asset.extras.warnings[0])).toBe(true);
});

test('ordinary names containing lod text do not gain LOD metadata', () => {
  const { json } = jsonOf([part('ordinary'), part('lodge'), part('lod_stand')]);
  expect(json.nodes.map((node: any) => node.name)).toEqual(['ordinary', 'lodge', 'lod_stand']);
  expect(json.nodes.every((node: any) => node.extras?.lod === undefined)).toBe(true);
  expect(json.nodes.every((node: any) => node.extensions?.MSFT_lod === undefined)).toBe(true);
  expect(json.extensionsUsed ?? []).not.toContain('MSFT_lod');
});

test('a three-level LOD file validates without errors', async () => {
  const { glb } = jsonOf([part('rock_lod0'), part('rock_lod1', pyramid()), part('rock_lod2', tetra())]);
  const report = await validator.validateBytes(glb);
  expect(report.issues.numErrors).toBe(0);
  // Validator warnings for the unregistered MSFT_lod extension are acceptable.
});

test('separate groups retain independent extension ids', () => {
  const { json } = jsonOf([part('rock_lod0'), part('rock_lod1'), part('bush_lod0'), part('bush_lod1')]);
  expect(json.nodes[0].extensions.MSFT_lod.ids).toEqual([1]);
  expect(json.nodes[2].extensions.MSFT_lod.ids).toEqual([3]);
});
