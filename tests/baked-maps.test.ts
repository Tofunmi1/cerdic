import { expect, test } from 'bun:test';
import validator from 'gltf-validator';
import { buildGlbWithTextures } from '../src/engine/gltf';
import { resampleRgba, uploadResult } from '../src/engine/mesh-upload';
import { bakedMapsFromEvaluator, type MeshPart } from '../src/engine/worker-types';

const part = (extra: Partial<MeshPart> = {}): MeshPart => ({
  node: 'n', name: 'part', volume: 1,
  positions: new Float32Array([0, 0, 0, 1, 0, 0, 0, 1, 0]),
  indices: new Uint32Array([0, 1, 2]), uvs: new Float32Array([0, 0, 1, 0, 0, 1]),
  occlusion: new Float32Array(3), ...extra,
});
function jsonOf(glb: Uint8Array) {
  const length = new DataView(glb.buffer, glb.byteOffset).getUint32(12, true);
  return JSON.parse(new TextDecoder().decode(glb.subarray(20, 20 + length)));
}
function imageSize(glb: Uint8Array, index: number) {
  const json = jsonOf(glb), view = json.bufferViews[json.images[index].bufferView];
  const start = 20 + new DataView(glb.buffer, glb.byteOffset).getUint32(12, true) + 8 + view.byteOffset;
  return new DataView(glb.buffer, glb.byteOffset + start).getUint32(16, false);
}
const validate = async (glb: Uint8Array) => (await validator.validateBytes(glb)).issues.numErrors;

test('evaluator baked exports populate only baked parts', () => {
  const normal = new Uint8Array(64);
  const baked = bakedMapsFromEvaluator(4, () => normal, () => new Uint8Array(64));
  expect(baked?.size).toBe(4);
  expect(baked?.normal?.length).toBe(64);
  expect(bakedMapsFromEvaluator(0, () => normal, () => normal)).toBeUndefined();
});

test('RGBA resampling is bilinear, constant preserving and identity at 512', () => {
  const colors = Uint8Array.from([0, 0, 0, 255, 100, 0, 0, 255, 0, 100, 0, 255, 100, 100, 0, 255]);
  const scaled = resampleRgba(colors, 2, 512);
  expect([...scaled.slice(0, 4)]).toEqual([...colors.slice(0, 4)]);
  expect([...scaled.slice((511 * 512 + 511) * 4, (511 * 512 + 511) * 4 + 4)]).toEqual([...colors.slice(12, 16)]);
  const center = [...scaled.slice((256 * 512 + 256) * 4, (256 * 512 + 256) * 4 + 4)];
  expect(center.every((v, i) => Math.abs(v - [50, 50, 0, 255][i]) <= 1)).toBe(true);
  const constant = new Uint8Array(16).fill(91);
  expect([...resampleRgba(constant, 2, 512).slice(0, 4)]).toEqual([91, 91, 91, 91]);
  const map = new Uint8Array(512 * 512 * 4);
  expect(resampleRgba(map, 512, 512)).toBe(map);
});

test('viewer stages baked maps at 512 and ignores AO', async () => {
  const originalRaf = globalThis.requestAnimationFrame;
  globalThis.requestAnimationFrame = ((callback: FrameRequestCallback) => { callback(0); return 1; }) as typeof requestAnimationFrame;
  const token = { document: 'd', revision: 1, request: 2, generation: 3, quality: 'final' as const, hash: 'h' };
  const calls: Uint8Array[][] = [];
  const runtime = new Proxy({}, { get: (_target, key) => {
    if (key === 'eval_projection_maps') return (...args: unknown[]) => { calls.push(args.slice(3) as Uint8Array[]); return true; };
    if (key === 'eval_projection_status') return () => JSON.stringify({ committed: token });
    return () => true;
  } }) as Parameters<typeof uploadResult>[0];
  const run = async (extra: Partial<MeshPart>) => {
    calls.length = 0;
    await uploadResult(runtime, token, [part(extra)], [], null, () => true);
    return calls[0];
  };
  try {
    // The renderer's layers are 1024 square (ve_render::LAYER_SIZE): any other baked size is resampled to fit.
    const layer = 1024;
    const scaled = await run({ baked: { size: 256, normal: new Uint8Array(256 * 256 * 4).fill(9) } });
    expect(scaled[0].length).toBe(0);
    expect(scaled[1].length).toBe(layer * layer * 4);
    expect(scaled[1][0]).toBe(9);
    const direct = new Uint8Array(layer * layer * 4).fill(7);
    expect((await run({ baked: { size: layer, normal: direct } }))[1]).toBe(direct);
    expect((await run({ baked: { size: layer, ao: direct } }))[1].length).toBe(0);
  } finally { globalThis.requestAnimationFrame = originalRaf; }
});

test('a part with a baked normal map sends its own UVs with its vertices, others do not', async () => {
  const originalRaf = globalThis.requestAnimationFrame;
  globalThis.requestAnimationFrame = ((callback: FrameRequestCallback) => { callback(0); return 1; }) as typeof requestAnimationFrame;
  const token = { document: 'd', revision: 1, request: 2, generation: 3, quality: 'final' as const, hash: 'h' };
  const chunks: { floats: number; hasUvs: boolean }[] = [];
  const runtime = new Proxy({}, { get: (_target, key) => {
    if (key === 'eval_projection_chunk_part') return (...args: unknown[]) => { chunks.push({ floats: (args[5] as Float32Array).length, hasUvs: args[6] as boolean }); return true; };
    if (key === 'eval_projection_status') return () => JSON.stringify({ committed: token });
    return () => true;
  } }) as Parameters<typeof uploadResult>[0];
  try {
    // One triangle: three vertices of seven floats (position, normal, occlusion), or nine with texture coordinates.
    await uploadResult(runtime, token, [part()], [], null, () => true);
    await uploadResult(runtime, token, [part({ baked: { size: 4, normal: new Uint8Array(64) } })], [], null, () => true);
    await uploadResult(runtime, token, [part({ baked: { size: 4, ao: new Uint8Array(64) } })], [], null, () => true);
    expect(chunks).toEqual([{ floats: 21, hasUvs: false }, { floats: 27, hasUvs: true }, { floats: 21, hasUvs: false }]);
  } finally { globalThis.requestAnimationFrame = originalRaf; }
});

test('baked normal exports a PNG, tangents and a valid glTF', async () => {
  const normal = new Uint8Array(4 * 4 * 4).fill(128);
  const glbNormal = await buildGlbWithTextures([part({ baked: { size: 4, normal } })], { mikkTangents: (_p, _n, _u, ix) => Float32Array.from({ length: ix.length * 4 }, (_, i) => i % 4 === 0 || i % 4 === 3 ? 1 : 0) });
  const normalJson = jsonOf(glbNormal);
  expect(normalJson.materials[0].normalTexture).toBeDefined();
  expect(normalJson.meshes[0].primitives[0].attributes.TANGENT).toBeDefined();
  expect(normalJson.images).toHaveLength(1);
  expect(imageSize(glbNormal, 0)).toBe(4);
  expect(await validate(glbNormal)).toBe(0);
});

test('baked AO is a valid glTF texture and both maps embed two images', async () => {
  const ao = new Uint8Array(4 * 4 * 4).fill(255), normal = new Uint8Array(4 * 4 * 4).fill(128);
  const aoGlb = await buildGlbWithTextures([part({ baked: { size: 4, ao } })]);
  expect(jsonOf(aoGlb).materials[0].occlusionTexture).toBeDefined();
  expect(await validate(aoGlb)).toBe(0);
  const both = await buildGlbWithTextures([part({ baked: { size: 4, normal, ao } })]);
  expect(jsonOf(both).images).toHaveLength(2);
});

test('baked normal replaces procedural normal for the part', async () => {
  const normal = new Uint8Array(4 * 4 * 4).fill(128);
  const procedural = Uint8Array.from(atob('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/mN8AAAAASUVORK5CYII='), c => c.charCodeAt(0));
  const replaced = await buildGlbWithTextures([part({ baked: { size: 4, normal } })], { textureImages: new Map([[0, { normal: procedural }]]) });
  expect(imageSize(replaced, 0)).toBe(4);
  expect(procedural.length).toBeGreaterThan(8);
});

test('undefined baked data leaves export byte-identical', async () => {
  const a = await buildGlbWithTextures([part()]);
  const b = await buildGlbWithTextures([part({ baked: undefined })]);
  expect([...a]).toEqual([...b]);
});
