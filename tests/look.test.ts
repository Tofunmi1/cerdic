import { expect, test } from 'bun:test';
import validator from 'gltf-validator';
import { sampleLook } from '../src/engine/animation';
import { buildGlb } from '../src/engine/gltf';
import type { AnimationTrack, MeshPart } from '../src/engine/worker-types';

const box = (name: string, node: string, material: MeshPart['material']): MeshPart => ({
  node, name, volume: 1, occlusion: new Float32Array(8), material,
  positions: new Float32Array([-0.5, -0.5, -0.5, 0.5, -0.5, -0.5, 0.5, 0.5, -0.5, -0.5, 0.5, -0.5, -0.5, -0.5, 0.5, 0.5, -0.5, 0.5, 0.5, 0.5, 0.5, -0.5, 0.5, 0.5]),
  indices: new Uint32Array([0, 2, 1, 0, 3, 2, 4, 5, 6, 4, 6, 7, 0, 1, 5, 0, 5, 4, 1, 2, 6, 1, 6, 5, 2, 3, 7, 2, 7, 6, 3, 0, 4, 3, 4, 7]),
});
const glow: AnimationTrack = {
  property: 'glow', interpolation: 'linear', looped: false, pivot: [0, 0, 0],
  keys: [{ time: 0, value: [0, 0, 0] }, { time: 1, value: [1, 0, 0] }],
};
const tint: AnimationTrack = {
  property: 'color', interpolation: 'linear', looped: false, pivot: [0, 0, 0],
  keys: [{ time: 0, value: [0.9, 0.1, 0.1] }, { time: 2, value: [0.1, 0.1, 0.9] }],
};
const json = (glb: Uint8Array) => JSON.parse(new TextDecoder().decode(glb.subarray(20, 20 + new DataView(glb.buffer, glb.byteOffset).getUint32(12, true))));
const lampMaterial = { color: [0.9, 0.9, 0.2], emissive: [2, 1.7, 1.1], metallic: 0, roughness: 0.4 } as MeshPart['material'];

test('glow and colour tracks sample over time like the viewer plays them', () => {
  const g = sampleLook([glow], 'glow');
  expect(g?.times[g.times.length - 1]).toBe(1);
  expect(g?.values[0]).toBe(0);
  expect(g?.values[g.values.length - 1]).toBe(1);
  const c = sampleLook([tint], 'color');
  expect(Array.from(c?.values.slice(-3) ?? [])).toEqual([0.1, 0.1, 0.9].map(Math.fround));
  expect(sampleLook([], 'glow')).toBeNull();
});

test('glow and colour export as pointer animations on a material of their own, valid for the Khronos validator', async () => {
  const lamp = { ...box('lamp', 'n_1', lampMaterial), animation: [glow, tint] };
  const twin = box('twin', 'n_2', lampMaterial); // the same look, not animated
  const glb = buildGlb([lamp, twin]);
  const report = await validator.validateBytes(glb);
  const messages = report.issues.messages.filter(m => m.severity < 2).map(m => `${m.code} ${m.pointer ?? ''} ${m.message}`).join('; ');
  expect(report.issues.numErrors, messages).toBe(0);
  expect(report.issues.numWarnings, messages).toBe(0);
  const gltf = json(glb);
  expect(gltf.extensionsUsed).toEqual(['KHR_animation_pointer', 'KHR_materials_emissive_strength']);
  const lampMaterialIndex = gltf.meshes[gltf.nodes.find((n: { name: string }) => n.name === 'lamp').mesh].primitives[0].material;
  const twinMaterialIndex = gltf.meshes[gltf.nodes.find((n: { name: string }) => n.name === 'twin').mesh].primitives[0].material;
  expect(lampMaterialIndex).not.toBe(twinMaterialIndex);
  const pointers = gltf.animations[0].channels.map((c: { target: { extensions: { KHR_animation_pointer: { pointer: string } } } }) => c.target.extensions.KHR_animation_pointer.pointer);
  expect(pointers).toEqual([
    `/materials/${lampMaterialIndex}/extensions/KHR_materials_emissive_strength/emissiveStrength`,
    `/materials/${lampMaterialIndex}/pbrMetallicRoughness/baseColorFactor`,
  ]);
  expect(gltf.materials[twinMaterialIndex].extensions.KHR_materials_emissive_strength.emissiveStrength).toBe(2);
});

const hide: AnimationTrack = {
  property: 'visible', interpolation: 'step', looped: false, pivot: [0, 0, 0],
  keys: [{ time: 0, value: [1, 0, 0] }, { time: 1, value: [0, 0, 0] }],
};

test('a visible track exports as a node visibility clip that is valid for the Khronos validator', async () => {
  const part = { ...box('ghost', 'n_1', lampMaterial), animation: [hide] };
  const glb = buildGlb([part]);
  const report = await validator.validateBytes(glb);
  const messages = report.issues.messages.filter(m => m.severity < 2).map(m => `${m.code} ${m.pointer ?? ''} ${m.message}`).join('; ');
  expect(report.issues.numErrors, messages).toBe(0);
  expect(report.issues.numWarnings, messages).toBe(0);
  const gltf = json(glb);
  expect(gltf.extensionsUsed).toContain('KHR_node_visibility');
  expect(gltf.nodes[0].extensions.KHR_node_visibility.visible).toBe(true);
  const [channel] = gltf.animations[0].channels;
  expect(channel.target.extensions.KHR_animation_pointer.pointer).toBe('/nodes/0/extensions/KHR_node_visibility/visible');
  expect(gltf.animations[0].samplers[channel.sampler].interpolation).toBe('STEP');
});

test('an unlit part with a glow track exports no pointer clip', async () => {
  const plain = { ...box('plain', 'n_1', { color: [0.5, 0.5, 0.5], emissive: [0, 0, 0], metallic: 0, roughness: 0.5 } as MeshPart['material']), animation: [glow] };
  const glb = buildGlb([plain]);
  expect(json(glb).animations).toBeUndefined();
});
