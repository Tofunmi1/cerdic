import { expect, test } from 'bun:test';
import validator from 'gltf-validator';
import { buildGlb } from '../src/engine/gltf';
import type { MeshPart } from '../src/engine/worker-types';

// A unit cube, eight shared corners and twelve triangles, wound outward.
const cube = (): Pick<MeshPart, 'positions' | 'indices'> => ({
  positions: new Float32Array([-0.5, -0.5, -0.5, 0.5, -0.5, -0.5, 0.5, 0.5, -0.5, -0.5, 0.5, -0.5, -0.5, -0.5, 0.5, 0.5, -0.5, 0.5, 0.5, 0.5, 0.5, -0.5, 0.5, 0.5]),
  indices: new Uint32Array([0, 2, 1, 0, 3, 2, 4, 5, 6, 4, 6, 7, 0, 1, 5, 0, 5, 4, 1, 2, 6, 1, 6, 5, 2, 3, 7, 2, 7, 6, 3, 0, 4, 3, 4, 7]),
});

const part = (extra: Partial<MeshPart> = {}): MeshPart => ({
  node: 'n_1', name: 'block', volume: 1, occlusion: new Float32Array(8), ...cube(), ...extra,
});

async function validate(glb: Uint8Array) {
  const report = await validator.validateBytes(glb);
  return report.issues;
}

test('a part exports as a valid glb with positions, normals and a material', async () => {
  const glb = buildGlb([part({ material: { color: [0.8, 0.4, 0.2], metallic: 0.1, roughness: 0.6, emissive: [0, 0, 0], emits_light: false } })]);
  const issues = await validate(glb);
  expect(issues.numErrors).toBe(0);
  expect(issues.numWarnings).toBe(0);
  const header = new DataView(glb.buffer, glb.byteOffset);
  expect(header.getUint32(0, true)).toBe(0x46546c67);
  expect(header.getUint32(8, true)).toBe(glb.byteLength);
  expect(glb.byteLength % 4).toBe(0);
});

test('small meshes use aligned 16-bit indices and scene units are explicit', async () => {
  const glb = buildGlb([part()]);
  const jsonLength = new DataView(glb.buffer, glb.byteOffset).getUint32(12, true);
  const json = JSON.parse(new TextDecoder().decode(glb.subarray(20, 20 + jsonLength)));
  const index = json.accessors[json.meshes[0].primitives[0].indices];
  expect(index.componentType).toBe(5123);
  expect(json.scenes[0].extras.units).toBe('metres');
  const view = json.bufferViews[index.bufferView];
  expect((view.byteOffset ?? 0) % 4).toBe(0);
});

test('optional per-vertex RGBA colours are exported as COLOR_0', async () => {
  const colored = part() as MeshPart & { vertexColors: Float32Array };
  colored.vertexColors = new Float32Array(8 * 4).fill(0.5);
  const glb = buildGlb([colored]);
  const report = await validator.validateBytes(glb);
  expect(report.issues.numErrors).toBe(0);
  const jsonLength = new DataView(glb.buffer, glb.byteOffset).getUint32(12, true);
  const json = JSON.parse(new TextDecoder().decode(glb.subarray(20, 20 + jsonLength)));
  const attrs = json.meshes[0].primitives[0].attributes;
  expect(json.accessors[attrs.COLOR_0].type).toBe('VEC4');
  expect(json.accessors[attrs.COLOR_0].count).toBe(json.accessors[attrs.POSITION].count);
});

test('transparent material exports BLEND alpha and validates cleanly', async () => {
  const glb = buildGlb([part({ material: { color: [0.1, 0.3, 0.9], opacity: 0.4, metallic: 0, roughness: 0.2, emissive: [0, 0, 0], emits_light: false } })]);
  const report = await validator.validateBytes(glb);
  expect(report.issues.numErrors).toBe(0);
  expect(report.issues.numWarnings).toBe(0);
  const jsonLength = new DataView(glb.buffer, glb.byteOffset).getUint32(12, true);
  const json = JSON.parse(new TextDecoder().decode(glb.subarray(20, 20 + jsonLength)));
  expect(json.materials[0].alphaMode).toBe('BLEND');
  expect(json.materials[0].pbrMetallicRoughness.baseColorFactor[3]).toBe(0.4);
});

test('a cube keeps its hard edges: 24 vertices, one normal per face, not 8 smoothed ones', async () => {
  const glb = buildGlb([part()]);
  const report = await validator.validateBytes(glb);
  const info = report.info;
  expect(info.totalVertexCount).toBe(24);
  expect(info.totalTriangleCount).toBe(12);
  expect(info.materialCount).toBe(1);
});

test('closed meshes export generated UV charts without welding across chart seams', async () => {
  const glb = buildGlb([part()]);
  const report = await validator.validateBytes(glb);
  expect(report.issues.numErrors).toBe(0);
  const jsonLength = new DataView(glb.buffer, glb.byteOffset).getUint32(12, true);
  const json = JSON.parse(new TextDecoder().decode(glb.subarray(20, 20 + jsonLength)));
  const primitive = json.meshes[0].primitives[0];
  expect(primitive.attributes.TEXCOORD_0).toBeDefined();
  const uv = json.accessors[primitive.attributes.TEXCOORD_0];
  expect(uv.type).toBe('VEC2');
  expect(uv.count).toBe(json.accessors[primitive.attributes.POSITION].count);
});

test('shape keys export named position and normal deltas across split vertices', async () => {
  const positions = cube().positions;
  const delta = new Float32Array(positions.length);
  for (let point = 0; point < positions.length / 3; point++) {
    if (positions[point * 3 + 1] > 0 && positions[point * 3] > 0) delta[point * 3 + 1] = 0.15;
  }
  const glb = buildGlb([part({ morph_targets: [{ name: 'raise', weight: 0.5, positions: delta }] })]);
  const report = await validator.validateBytes(glb);
  expect(report.issues.numErrors).toBe(0);
  const jsonLength = new DataView(glb.buffer, glb.byteOffset).getUint32(12, true);
  const json = JSON.parse(new TextDecoder().decode(glb.subarray(20, 20 + jsonLength)));
  const mesh = json.meshes[0];
  const target = mesh.primitives[0].targets[0];
  expect(mesh.weights).toEqual([0.5]);
  expect(mesh.extras.targetNames).toEqual(['raise']);
  expect(json.accessors[target.POSITION].type).toBe('VEC3');
  expect(json.accessors[target.NORMAL].type).toBe('VEC3');
  const chunkStart = 20 + jsonLength + 8;
  const values = (accessorIndex: number) => {
    const accessor = json.accessors[accessorIndex];
    const view = json.bufferViews[accessor.bufferView];
    const offset = chunkStart + (view.byteOffset ?? 0) + (accessor.byteOffset ?? 0);
    const binary = new DataView(glb.buffer, glb.byteOffset);
    return Array.from({ length: accessor.count * 3 }, (_, i) => binary.getFloat32(offset + i * 4, true));
  };
  expect(values(target.POSITION).some(value => Math.abs(value) > 0.01)).toBe(true);
  expect(values(target.NORMAL).some(value => Math.abs(value) > 0.01)).toBe(true);
});

test('authored per-corner UVs take precedence over generated charts', () => {
  const authored = new Float32Array(cube().indices.length * 2);
  for (let corner = 0; corner < authored.length / 2; corner++) authored.set([0.25, 0.75], corner * 2);
  const glb = buildGlb([part({ uvs: authored })]);
  const jsonLength = new DataView(glb.buffer, glb.byteOffset).getUint32(12, true);
  const json = JSON.parse(new TextDecoder().decode(glb.subarray(20, 20 + jsonLength)));
  const uvAccessor = json.accessors[json.meshes[0].primitives[0].attributes.TEXCOORD_0];
  const view = json.bufferViews[uvAccessor.bufferView];
  const uvOffset = 20 + jsonLength + 8 + (view.byteOffset ?? 0) + (uvAccessor.byteOffset ?? 0);
  const binary = new DataView(glb.buffer, glb.byteOffset);
  expect(binary.getFloat32(uvOffset, true)).toBe(0.25);
  expect(binary.getFloat32(uvOffset + 4, true)).toBe(0.75);
});

test('baked PNG maps become embedded glTF textures and material bindings', async () => {
  const png = Uint8Array.from(atob('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/mN8AAAAASUVORK5CYII='), c => c.charCodeAt(0));
  const glb = buildGlb([part()], { textureImages: new Map([[0, { albedo: png, metallicRoughness: png, normal: png, emissive: png }]]) });
  const report = await validator.validateBytes(glb);
  expect(report.issues.numErrors).toBe(0);
  const jsonLength = new DataView(glb.buffer, glb.byteOffset).getUint32(12, true);
  const json = JSON.parse(new TextDecoder().decode(glb.subarray(20, 20 + jsonLength)));
  expect(json.images).toHaveLength(4);
  expect(json.textures).toHaveLength(4);
  expect(json.materials[0].pbrMetallicRoughness.baseColorTexture.index).toBe(0);
  expect(json.materials[0].pbrMetallicRoughness.metallicRoughnessTexture.index).toBe(1);
  expect(json.materials[0].normalTexture.index).toBe(2);
  expect(json.materials[0].emissiveTexture.index).toBe(3);
  expect(json.materials[0].emissiveFactor).toEqual([1, 1, 1]);
  expect(json.accessors[json.meshes[0].primitives[0].attributes.TANGENT].type).toBe('VEC4');
});

test('MikkTSpace corner tangents split vertices at tangent seams', async () => {
  const png = Uint8Array.from(atob('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/mN8AAAAASUVORK5CYII='), c => c.charCodeAt(0));
  let calls = 0;
  const glb = buildGlb([part()], {
    textureImages: new Map([[0, { normal: png }]]),
    mikkTangents: (_positions, normals, _uvs, indices) => {
      calls++;
      const out = new Float32Array(indices.length * 4);
      for (let corner = 0; corner < indices.length; corner++) {
        const at = indices[corner] * 3, nx = normals[at], ny = normals[at + 1], nz = normals[at + 2];
        const axis = Math.abs(nx) < 0.8 ? [1, 0, 0] : [0, 1, 0];
        let tx = ny * axis[2] - nz * axis[1], ty = nz * axis[0] - nx * axis[2], tz = nx * axis[1] - ny * axis[0];
        const length = Math.hypot(tx, ty, tz); tx /= length; ty /= length; tz /= length;
        out.set([tx, ty, tz, corner % 2 ? -1 : 1], corner * 4);
      }
      return out;
    },
  });
  const report = await validator.validateBytes(glb);
  expect(calls).toBe(1);
  expect(report.issues.numErrors).toBe(0);
  expect(report.issues.numWarnings).toBe(0);
  expect(report.info.totalVertexCount).toBeGreaterThan(24);
  const jsonLength = new DataView(glb.buffer, glb.byteOffset).getUint32(12, true);
  const json = JSON.parse(new TextDecoder().decode(glb.subarray(20, 20 + jsonLength)));
  expect(json.accessors[json.meshes[0].primitives[0].attributes.TANGENT].type).toBe('VEC4');
});

test('a smooth surface shares its vertices across shallow edges', async () => {
  // A flat 3 x 3 grid of quads (planar): every interior vertex is shared by its faces.
  const n = 3, positions: number[] = [], indices: number[] = [];
  for (let j = 0; j <= n; j++) for (let i = 0; i <= n; i++) positions.push(i, 0, j);
  for (let j = 0; j < n; j++) for (let i = 0; i < n; i++) {
    const a = j * (n + 1) + i, b = a + 1, c = a + n + 2, d = a + n + 1;
    indices.push(a, d, c, a, c, b);
  }
  const glb = buildGlb([part({ positions: new Float32Array(positions), indices: new Uint32Array(indices) })]);
  const report = await validator.validateBytes(glb);
  expect(report.issues.numErrors).toBe(0);
  expect(report.info.totalVertexCount).toBe((n + 1) * (n + 1));
});

test('instances become one node each sharing a mesh; materials are shared', async () => {
  const translate = (x: number) => [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, x, 0, 0, 1];
  const transforms = new Float32Array([...translate(0), ...translate(2), ...translate(4)]);
  const glb = buildGlb([part({ transforms, instance_indices: new Uint32Array([0, 1, 2]) })]);
  const report = await validator.validateBytes(glb);
  expect(report.issues.numErrors).toBe(0);
  expect(report.info.meshCount ?? 1).toBe(1);
  const json = JSON.parse(new TextDecoder().decode(glb.subarray(20, 20 + new DataView(glb.buffer, glb.byteOffset).getUint32(12, true))));
  expect(json.nodes.length).toBe(3);
  expect(json.meshes.length).toBe(1);
  expect(json.nodes[2].matrix[12]).toBe(4);
});

test('sRGB colour is converted to the linear values glTF expects, and emission is kept', () => {
  const glb = buildGlb([part({ material: { color: [1, 0.5, 0], metallic: 0, roughness: 1, emissive: [0.2, 0, 0], emits_light: true } })]);
  const json = JSON.parse(new TextDecoder().decode(glb.subarray(20, 20 + new DataView(glb.buffer, glb.byteOffset).getUint32(12, true))));
  const [r, g, b] = json.materials[0].pbrMetallicRoughness.baseColorFactor;
  expect(r).toBeCloseTo(1, 5);
  expect(g).toBeCloseTo(0.214, 3);
  expect(b).toBe(0);
  expect(json.materials[0].emissiveFactor).toEqual([0.2, 0, 0]);
});

test('an empty model is still a valid file', async () => {
  const glb = buildGlb([]);
  const issues = await validate(glb);
  expect(issues.numErrors).toBe(0);
});
