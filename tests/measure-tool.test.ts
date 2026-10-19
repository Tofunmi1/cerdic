import { expect, test } from 'bun:test';
import { measureParts } from '../src/engine/mcp-bridge';
import type { McpHost } from '../src/engine/mcp-bridge';
import type { MeshPart } from '../src/engine/worker-types';

type Call = { positions: Float32Array; indices: Uint32Array; uvs: Float32Array; detail: string };

// A stand-in for the wasm runtime: records each call and answers with whatever the test wants. The
// bridge never inspects the reply, so the tests only need it to be well-formed.
function fakeRuntime(reply: (call: Call) => string) {
  const calls: Call[] = [];
  const runtime = {
    measure_mesh(positions: Float32Array, indices: Uint32Array, uvs: Float32Array, detail: string) {
      const call = { positions, indices, uvs, detail };
      calls.push(call);
      return reply(call);
    },
  };
  return { runtime, calls };
}

function hostWith(parts: MeshPart[], runtime: unknown, revision = 7): McpHost {
  return { ready: true, doc: null, evaluation: { parts, state: { revision } }, runtime } as unknown as McpHost;
}

function part(over: Partial<MeshPart>): MeshPart {
  return {
    node: 'n1', name: 'out', volume: 0,
    positions: new Float32Array(0), indices: new Uint32Array(0), occlusion: new Float32Array(0),
    ...over,
  };
}

const FULL = '{"triangles":12,"vertices":8,"volume":1,"watertight":true,'
  + '"thickness":{"min":0.1},"symmetry":{"x":{"max":0}},"uv":null,"bounding_shapes":{"best":"aabb"}}';

test('the summary is per part, prefixed by its name, and asks for summary detail', () => {
  const { runtime, calls } = fakeRuntime(() => 'triangles: 12\nvolume: 1.000000 m3\nok');
  const host = hostWith([part({ name: 'floor', node: 'n1' }), part({ name: 'roof', node: 'n2' })], runtime);
  const text = measureParts(host, {});
  expect(text).toBe('floor\ntriangles: 12\nvolume: 1.000000 m3\nok\n\nroof\ntriangles: 12\nvolume: 1.000000 m3\nok');
  expect(calls.map(call => call.detail)).toEqual(['summary', 'summary']);
});

test('part selects one output by name or node, and an unknown name says so', () => {
  const { runtime, calls } = fakeRuntime(() => 'ok');
  const host = hostWith([part({ name: 'floor', node: 'n1' }), part({ name: 'roof', node: 'n2' })], runtime);
  expect(measureParts(host, { part: 'roof' })).toBe('roof\nok');
  expect(measureParts(host, { part: 'n1' })).toBe('floor\nok');
  expect(calls).toHaveLength(2);
  expect(measureParts(host, { part: 'wall' })).toBe('no part named wall is on screen');
  expect(calls).toHaveLength(2);
});

test('nothing on screen is reported, not measured', () => {
  const { runtime, calls } = fakeRuntime(() => 'ok');
  expect(measureParts(hostWith([], runtime), {})).toBe('there is nothing to measure yet');
  expect(calls).toHaveLength(0);
});

test('full detail returns JSON keyed by part, and checks keeps only the named sections', () => {
  const { runtime, calls } = fakeRuntime(() => FULL);
  const host = hostWith([part({ name: 'roof', node: 'n2' })], runtime);
  const all = measureParts(host, { detail: 'full' }) as { revision: number; parts: Record<string, unknown>[] };
  expect(calls[0].detail).toBe('full');
  expect(all.revision).toBe(7);
  expect(all.parts[0]).toMatchObject({ name: 'roof', node: 'n2', triangles: 12 });
  expect(Object.keys(all.parts[0])).toContain('symmetry');

  const some = measureParts(host, { detail: 'full', checks: ['thickness', 'collision'] }) as { parts: Record<string, unknown>[] };
  const kept = some.parts[0];
  expect(kept.thickness).toEqual({ min: 0.1 });
  expect(kept.bounding_shapes).toEqual({ best: 'aabb' });
  expect(kept).not.toHaveProperty('symmetry');
  expect(kept).not.toHaveProperty('uv');
  expect(kept).toHaveProperty('triangles');
});

test('corner uvs become per-vertex uvs when there is no seam, and are dropped on a seam', () => {
  const positions = new Float32Array([0, 0, 0, 1, 0, 0, 1, 1, 0, 0, 1, 0]);
  const indices = new Uint32Array([0, 1, 2, 0, 2, 3]);
  const { runtime, calls } = fakeRuntime(() => 'ok');
  const clean = part({ positions, indices, uvs: new Float32Array([0, 0, 1, 0, 1, 1, 0, 0, 1, 1, 0, 1]) });
  measureParts(hostWith([clean], runtime), {});
  expect(Array.from(calls[0].uvs)).toEqual([0, 0, 1, 0, 1, 1, 0, 1]);

  const seamed = part({ positions, indices, uvs: new Float32Array([0.5, 0, 1, 0, 1, 1, 0, 0, 1, 1, 0, 1]) });
  measureParts(hostWith([seamed], runtime), {});
  expect(calls[1].uvs.length).toBe(0);

  const bare = part({ positions, indices });
  measureParts(hostWith([bare], runtime), {});
  expect(calls[2].uvs.length).toBe(0);
});
