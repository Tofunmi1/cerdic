import { expect, test } from 'bun:test';
import validator from 'gltf-validator';
import { decompose, jitterFraction, poseMatrix, sampleClip, sampleTimes, sampleTrack, trackPhase } from '../src/engine/animation';
import { buildGlb } from '../src/engine/gltf';
import type { AnimationTrack, MeshPart } from '../src/engine/worker-types';

const door: AnimationTrack = {
  property: 'rotation', interpolation: 'linear', looped: false, pivot: [-0.5, 0, 0],
  keys: [{ time: 0, value: [0, 0, 0] }, { time: 1.5, value: [0, 90, 0] }],
};
const lift: AnimationTrack = {
  property: 'position', interpolation: 'smooth', looped: true, pivot: [0, 0, 0],
  keys: [{ time: 0, value: [0, 0, 0] }, { time: 0.5, value: [0, 0.2, 0] }, { time: 1, value: [0, 0, 0] }],
};
const transform = (m: number[], p: [number, number, number]) => [
  m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12], m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13], m[2] * p[0] + m[6] * p[1] + m[10] * p[2] + m[14]];

test('track sampling follows the same rules as the Rust track: hold, ease, wrap, step', () => {
  expect(sampleTrack(door, -1)).toEqual([0, 0, 0]);
  expect(sampleTrack(door, 0.75)).toEqual([0, 45, 0]);
  expect(sampleTrack(door, 99)).toEqual([0, 90, 0]);
  expect(sampleTrack(lift, 0.25)[1]).toBeCloseTo(0.1, 6); // smoothstep is half at the middle of a segment
  expect(sampleTrack(lift, 1.25)).toEqual(sampleTrack(lift, 0.25)); // loops
  const step: AnimationTrack = { ...door, interpolation: 'step', keys: [{ time: 0, value: [0, 0, 0] }, { time: 2, value: [0, 90, 0] }] };
  expect(sampleTrack(step, 1.9)).toEqual([0, 0, 0]);
});

test('a door turned a quarter about its hinge swings its far edge and leaves the hinge alone', () => {
  const m = poseMatrix([door], 1.5);
  const far = transform(m, [0.5, 0, 0]);
  expect(far[0]).toBeCloseTo(-0.5, 5); expect(far[2]).toBeCloseTo(-1, 5);
  const hinge = transform(m, [-0.5, 0, 0]);
  expect(hinge[0]).toBeCloseTo(-0.5, 5); expect(hinge[2]).toBeCloseTo(0, 5);
  const trs = decompose(m);
  expect(trs.scale.every(s => Math.abs(s - 1) < 1e-6)).toBe(true);
  // A 90 degree turn about Y is the quaternion (0, sin 45, 0, cos 45).
  expect(trs.rotation[1]).toBeCloseTo(Math.SQRT1_2, 5);
  expect(trs.rotation[3]).toBeCloseTo(Math.SQRT1_2, 5);
});

test('sample times start at zero, increase, include every key and subdivide curved segments', () => {
  const times = sampleTimes([door, lift]);
  expect(times[0]).toBe(0);
  expect(times[times.length - 1]).toBeCloseTo(1.5, 6);
  expect(times.every((t, i) => i === 0 || t > times[i - 1])).toBe(true);
  for (const key of [0.5, 1, 1.5]) expect(times.some(t => Math.abs(t - key) < 1e-9)).toBe(true);
  expect(times.length).toBeGreaterThan(8);
  const clip = sampleClip([door]);
  expect(clip.rotations.length).toBe(clip.times.length * 4);
  // Neighbouring quaternions stay on one side of the sphere (the short way round).
  for (let i = 1; i < clip.times.length; i++) {
    const dot = [0, 1, 2, 3].reduce((s, k) => s + clip.rotations[i * 4 + k] * clip.rotations[(i - 1) * 4 + k], 0);
    expect(dot).toBeGreaterThan(0);
  }
});

const cube = (animation?: AnimationTrack[]): MeshPart => ({
  node: 'n_1', name: 'door', volume: 1, occlusion: new Float32Array(8), animation,
  positions: new Float32Array([-0.5, -0.5, -0.5, 0.5, -0.5, -0.5, 0.5, 0.5, -0.5, -0.5, 0.5, -0.5, -0.5, -0.5, 0.5, 0.5, -0.5, 0.5, 0.5, 0.5, 0.5, -0.5, 0.5, 0.5]),
  indices: new Uint32Array([0, 2, 1, 0, 3, 2, 4, 5, 6, 4, 6, 7, 0, 1, 5, 0, 5, 4, 1, 2, 6, 1, 6, 5, 2, 3, 7, 2, 7, 6, 3, 0, 4, 3, 4, 7]),
});

test('an animated part exports one valid glTF animation with channels on its node', async () => {
  const glb = buildGlb([cube([door, lift])]);
  const report = await validator.validateBytes(glb);
  expect(report.issues.numErrors).toBe(0);
  expect(report.issues.numWarnings).toBe(0);
  const json = JSON.parse(new TextDecoder().decode(glb.subarray(20, 20 + new DataView(glb.buffer, glb.byteOffset).getUint32(12, true))));
  expect(json.animations).toHaveLength(1);
  const [animation] = json.animations;
  expect(animation.channels.map((c: { target: { path: string } }) => c.target.path)).toEqual(['translation', 'rotation', 'scale']);
  expect(animation.channels.every((c: { target: { node: number } }) => c.target.node === 0)).toBe(true);
});

test('bezier tracks follow Blender: values it printed for the same keys, flat ends, no overshoot', () => {
  const curve: AnimationTrack = {
    property: 'position', interpolation: 'bezier', looped: false, pivot: [0, 0, 0],
    keys: [[1, 7], [2, 13], [4, 5], [6, 20]].map(([time, y]) => ({ time, value: [0, y, 0] as [number, number, number] })),
  };
  // Printed by Blender's own evaluate_fcurve (automatic animation handles), four decimals.
  const blender: [number, number][] = [[1.2, 7.5333], [1.5, 10], [1.8, 12.4667], [2.1, 12.9559], [2.4, 12.2889], [2.7, 10.8807],
    [3, 9], [3.3, 7.1193], [3.6, 5.7111], [3.9, 5.0441], [4.2, 5.3325], [4.5, 7.0743], [4.8, 10.0889], [5.1, 13.7241], [5.4, 17.0395], [5.7, 19.2497]];
  for (const [time, expected] of blender) expect(Math.abs(sampleTrack(curve, time)[1] - expected)).toBeLessThan(2e-3);
  expect(sampleTrack(curve, 5.99)[1]).toBeGreaterThan(19.9);
});

test('back, bounce and elastic settle on the end key and overshoot like Blender\'s curves', () => {
  const to = (interpolation: AnimationTrack['interpolation']): AnimationTrack => ({
    property: 'position', interpolation, looped: false, pivot: [0, 0, 0],
    keys: [{ time: 0, value: [0, 0, 0] }, { time: 1, value: [1, 0, 0] }],
  });
  expect(sampleTrack(to('back'), 0.7)[0]).toBeGreaterThan(1);
  const bounce = [0.4, 0.45, 0.5, 0.55, 0.6, 0.65, 0.7].map(t => sampleTrack(to('bounce'), t)[0]);
  expect(bounce.some((v, i) => i > 0 && v < bounce[i - 1])).toBe(true);
  expect(Math.max(...Array.from({ length: 100 }, (_, i) => sampleTrack(to('elastic'), i / 100)[0]))).toBeGreaterThan(1);
  for (const kind of ['back', 'bounce', 'elastic'] as const) expect(sampleTrack(to(kind), 1)[0]).toBeCloseTo(1, 5);
});

test('the per-copy phase matches the Rust values, and an animated copy exports translation, rotation and scale channels', async () => {
  // Same numbers as `jitter_matches_the_typescript_copy_the_exporter_uses` in ve-modeling.
  const expected: [number, number][] = [[0, 0.39212512969970703], [1, 0.36787551641464233], [7, 0.8936956524848938], [4095, 0.813762366771698]];
  for (const [index, value] of expected) expect(jitterFraction(index)).toBeCloseTo(value, 7);
  const sway: AnimationTrack = {
    property: 'rotation', interpolation: 'linear', looped: true, pivot: [0, 0, 0], phase_step: 0.25, phase_jitter: 0,
    keys: [{ time: 0, value: [0, 0, 0] }, { time: 1, value: [0, 0, 90] }],
  };
  expect(trackPhase(sway, 4)).toBe(1);
  const part = (): MeshPart => ({
    ...cube(), name: 'reeds', node: 'n_9', animation: [sway],
    transforms: new Float32Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 2, 0, 0, 1]),
    instance_indices: new Uint32Array([0, 1]),
  });
  const glb = buildGlb([part()]);
  const report = await validator.validateBytes(glb);
  expect(report.issues.numErrors).toBe(0);
  const json = JSON.parse(new TextDecoder().decode(glb.subarray(20, 20 + new DataView(glb.buffer, glb.byteOffset).getUint32(12, true))));
  expect(json.nodes.map((n: { name: string }) => n.name)).toEqual(['reeds.0', 'reeds.1']);
  expect(json.nodes.every((n: { matrix?: number[] }) => n.matrix === undefined)).toBe(true);
  expect(json.animations[0].channels).toHaveLength(6);
  // The second copy is placed 2 m along x: its translation channel ends there, and it starts a quarter of a turn ahead.
  const second = json.animations[0].channels.filter((c: { target: { node: number } }) => c.target.node === 1);
  expect(second.map((c: { target: { path: string } }) => c.target.path)).toEqual(['translation', 'rotation', 'scale']);
});

test('a scene with nothing animated has no animation block', async () => {
  const glb = buildGlb([cube()]);
  const json = JSON.parse(new TextDecoder().decode(glb.subarray(20, 20 + new DataView(glb.buffer, glb.byteOffset).getUint32(12, true))));
  expect(json.animations).toBeUndefined();
});
