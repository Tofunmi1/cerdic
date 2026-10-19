import { expect, test } from 'bun:test';
import validator from 'gltf-validator';
import { buildGlb } from '../src/engine/gltf';
import type { AnimationTrack, MeshPart } from '../src/engine/worker-types';

const box = (name: string, node: string, centre: [number, number, number], half = 0.1): MeshPart => {
  const [cx, cy, cz] = centre;
  const corners = [[-1, -1, -1], [1, -1, -1], [1, 1, -1], [-1, 1, -1], [-1, -1, 1], [1, -1, 1], [1, 1, 1], [-1, 1, 1]];
  return {
    node, name, volume: 1, occlusion: new Float32Array(8),
    positions: new Float32Array(corners.flatMap(([x, y, z]) => [cx + x * half, cy + y * half, cz + z * half])),
    indices: new Uint32Array([0, 2, 1, 0, 3, 2, 4, 5, 6, 4, 6, 7, 0, 1, 5, 0, 5, 4, 1, 2, 6, 1, 6, 5, 2, 3, 7, 2, 7, 6, 3, 0, 4, 3, 4, 7]),
  };
};
const bend: AnimationTrack = {
  property: 'rotation', interpolation: 'linear', looped: true, pivot: [0, 0, 0],
  keys: [{ time: 0, value: [0, 0, 0] }, { time: 1, value: [0, 0, 90] }],
};
const json = (glb: Uint8Array) => JSON.parse(new TextDecoder().decode(glb.subarray(20, 20 + new DataView(glb.buffer, glb.byteOffset).getUint32(12, true))));

/** A tall skinned block, half of it weighted to each of two separate bone parts (the lower bone bends). */
function arm(): MeshPart[] {
  const lower = { ...box('lower_bone', 'n_lower', [0, -0.5, 0], 0.05), animation: [bend], bone: { head: [0, -1, 0], tail: [0, 0, 0] } } as MeshPart;
  const upper = { ...box('upper_bone', 'n_upper', [0, 0.5, 0], 0.05), bone: { head: [0, 0, 0], tail: [0, 1, 0] } } as MeshPart;
  const skin = box('sleeve', 'n_sleeve', [0, 0, 0], 0.5);
  // Eight vertices: the lower four (y < 0) follow bone 0, the upper four follow bone 1.
  const joints = new Uint32Array(32), weights = new Float32Array(32);
  for (let v = 0; v < 8; v++) {
    const top = skin.positions[v * 3 + 1] > 0;
    joints[v * 4] = top ? 1 : 0;
    weights[v * 4] = 1;
  }
  return [lower, upper, { ...skin, skin: { bones: ['lower_bone', 'upper_bone'], falloff: 3, influences: 4 }, skin_joints: joints, skin_weights: weights }];
}

test('a skinned part exports a valid glTF skin whose joints are the bone parts and whose weights follow the vertices', async () => {
  const glb = buildGlb(arm());
  const report = await validator.validateBytes(glb);
  const messages = report.issues.messages.filter(m => m.severity < 2).map(m => `${m.code} ${m.pointer ?? ''}`).join('; ');
  expect(report.issues.numErrors, messages).toBe(0);
  expect(report.issues.numWarnings, messages).toBe(0);
  const gltf = json(glb);
  const index = (name: string) => gltf.nodes.findIndex((n: { name: string }) => n.name === name);
  expect(gltf.skins).toHaveLength(1);
  expect(gltf.skins[0].joints).toEqual([index('lower_bone'), index('upper_bone')]);
  const sleeve = gltf.nodes[index('sleeve')];
  expect(sleeve.skin).toBe(0);
  const attributes = gltf.meshes[sleeve.mesh].primitives[0].attributes;
  expect(attributes.JOINTS_0).toBeDefined();
  expect(attributes.WEIGHTS_0).toBeDefined();
  // The bones are separate roots, so they hang under one armature node (the validator needs a common root).
  const armature = gltf.nodes[index('armature')];
  expect(armature.children.sort()).toEqual([index('lower_bone'), index('upper_bone')].sort());
  expect(gltf.scenes[0].nodes).toContain(index('armature'));
  expect(gltf.scenes[0].nodes).not.toContain(index('lower_bone'));
  // The lower bone's clip is exported on its node.
  expect(gltf.animations[0].channels.some((c: { target: { node: number } }) => c.target.node === index('lower_bone'))).toBe(true);
});

test('a skin naming a bone that is not in the file is exported as a plain mesh', async () => {
  const parts = arm();
  parts[2] = { ...parts[2], skin: { bones: ['lower_bone', 'ghost'], falloff: 3, influences: 4 } };
  const glb = buildGlb(parts);
  const report = await validator.validateBytes(glb);
  expect(report.issues.numErrors).toBe(0);
  expect(json(glb).skins).toBeUndefined();
});
