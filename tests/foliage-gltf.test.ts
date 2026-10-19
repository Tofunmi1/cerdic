// glTF export of foliage: a cutoff makes the albedo texture's alpha a mask, any foliage makes the material two-sided.
import { expect, test } from 'bun:test';
import validator from 'gltf-validator';
import { buildGlb } from '../src/engine/gltf';
import { rgbaPng } from '../src/engine/png';
import type { MeshPart, PartMaterial } from '../src/engine/worker-types';

const material = (extra: Partial<PartMaterial> = {}): PartMaterial => ({
  color: [0.2, 0.6, 0.2], metallic: 0, roughness: 0.8, emissive: [0, 0, 0], emits_light: false, ...extra,
});
const part = (mat: PartMaterial): MeshPart => ({
  node: 'n', name: 'leaf', volume: 0, material: mat,
  positions: new Float32Array([0, 0, 0, 1, 0, 0, 0, 1, 0]),
  indices: new Uint32Array([0, 1, 2]), uvs: new Float32Array([0, 0, 1, 0, 0, 1]),
  occlusion: new Float32Array(3),
});
function jsonOf(glb: Uint8Array) {
  const length = new DataView(glb.buffer, glb.byteOffset).getUint32(12, true);
  return JSON.parse(new TextDecoder().decode(glb.subarray(20, 20 + length)));
}
const errors = async (glb: Uint8Array) => (await validator.validateBytes(glb)).issues.numErrors;

/** A 4 by 4 albedo map whose left half is opaque and right half is clear. */
async function leafAlbedo() {
  const rgba = new Uint8Array(4 * 4 * 4);
  for (let y = 0; y < 4; y++) for (let x = 0; x < 4; x++) rgba.set([30, 160, 30, x < 2 ? 255 : 0], (y * 4 + x) * 4);
  return rgbaPng(rgba, 4);
}

test('a cutoff with an albedo map exports as a mask and a two-sided material', async () => {
  const glb = buildGlb([part(material({ foliage: { alpha_cutoff: 0.4, sway: 0, flutter: 0 } }))], { textureImages: new Map([[0, { albedo: await leafAlbedo() }]]) });
  const json = jsonOf(glb);
  expect(json.materials[0].alphaMode).toBe('MASK');
  expect(json.materials[0].alphaCutoff).toBeCloseTo(0.4, 6);
  expect(json.materials[0].doubleSided).toBe(true);
  expect(json.materials[0].pbrMetallicRoughness.baseColorTexture).toBeDefined();
  expect(await errors(glb)).toBe(0);
});

test('wind alone makes the material two-sided and leaves its alpha alone', async () => {
  const glb = buildGlb([part(material({ foliage: { alpha_cutoff: 0, sway: 0.5, flutter: 0.2 } }))], { textureImages: new Map([[0, { albedo: await leafAlbedo() }]]) });
  const json = jsonOf(glb);
  expect(json.materials[0].doubleSided).toBe(true);
  expect(json.materials[0].alphaMode).toBeUndefined();
  expect(json.materials[0].alphaCutoff).toBeUndefined();
  expect(await errors(glb)).toBe(0);
});

test('a cutoff with no albedo map has no alpha to cut, so only the two-sidedness is written', async () => {
  const glb = buildGlb([part(material({ foliage: { alpha_cutoff: 0.5, sway: 0, flutter: 0 } }))]);
  const json = jsonOf(glb);
  expect(json.materials[0].doubleSided).toBe(true);
  expect(json.materials[0].alphaMode).toBeUndefined();
  expect(await errors(glb)).toBe(0);
});

test('a translucent leaf stays blended rather than masked', async () => {
  const glb = buildGlb([part(material({ opacity: 0.5, foliage: { alpha_cutoff: 0.5, sway: 0, flutter: 0 } }))], { textureImages: new Map([[0, { albedo: await leafAlbedo() }]]) });
  expect(jsonOf(glb).materials[0].alphaMode).toBe('BLEND');
  expect(await errors(glb)).toBe(0);
});

test('a surface with no foliage, or with all of it at zero, exports exactly as before', async () => {
  const plain = jsonOf(buildGlb([part(material())])).materials[0];
  const zero = jsonOf(buildGlb([part(material({ foliage: { alpha_cutoff: 0, sway: 0, flutter: 0 } }))])).materials[0];
  expect(plain.doubleSided).toBeUndefined();
  expect(plain.alphaMode).toBeUndefined();
  expect(zero).toEqual(plain);
  expect(jsonOf(buildGlb([part(material({ foliage: null }))])).materials[0]).toEqual(plain);
});
