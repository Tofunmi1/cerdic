import { afterEach, expect, test } from 'bun:test';
import { playAimPolicy } from '../../../src/engine/aim-playtest';
import type { Runtime } from '../../../src/engine/bridge';

const savedFrame = Object.getOwnPropertyDescriptor(globalThis, 'requestAnimationFrame');
afterEach(() => {
  if (savedFrame) Object.defineProperty(globalThis, 'requestAnimationFrame', savedFrame);
  else Reflect.deleteProperty(globalThis, 'requestAnimationFrame');
});

// A live-runtime fixture exercises orchestration only; the engine's real swing physics still needs a browser.
function runtimeFixture(profileFails = false) {
  Object.defineProperty(globalThis, 'requestAnimationFrame', { configurable: true, value: (callback: FrameRequestCallback) => {
    queueMicrotask(() => callback(0));
    return 1;
  } });
  let hole = 1, total = 0, lastHead = Infinity, restored = false;
  const profiles: Array<{ method: string; params: unknown }> = [];
  const poses: number[][] = [];
  const runtime = {
    game_status: () => JSON.stringify({ present: true, status: hole > 3 ? 'won' : 'playing', vars: { hole, total },
      entities: [{ id: hole, kind: 'puck_kind', position: [0, 1, hole * 10] },
        ...[1, 2, 3].map(i => ({ id: 10 + i, kind: `pocket_kind${i}`, position: [0, 1, i * 10 + 5] }))] }),
    game_set_hands: (raw: string) => {
      if (!raw) { restored = true; return true; }
      const [{ position, rotation, grip, trigger }] = JSON.parse(raw);
      expect(rotation).toEqual([0, 0, 0, 1]);
      expect(grip).toBe(false);
      expect(trigger).toBe(false);
      poses.push(position);
      const head = position[2] - 0.9;
      if (lastHead < hole * 10 && head >= hole * 10) { total++; hole++; }
      lastHead = head;
      return true;
    },
    world_call: (method: string, raw: string) => { profiles.push({ method, params: JSON.parse(raw) }); return 1; },
    world_take: () => JSON.stringify(profileFails ? { ok: false, error: 'no ground' } : { ok: true, result: { points: [
      { s: 0, height: 1, decel: 0.5, surface_name: 'green' }, { s: 5, height: 1, decel: 0.5, surface_name: 'green' },
    ] } }),
  } as unknown as Runtime;
  const text = ['kind puck_kind tags=rolling', ...[1, 2, 3].map(i => `kind pocket_kind${i} tags=goal${i}`)].join('\n');
  return { runtime, text, profiles, poses, restored: () => restored };
}

test('live aiming resolves tags, profiles each target, swings, records strokes and restores hands', async () => {
  const f = runtimeFixture();
  const result = await playAimPolicy(f.runtime, { thing: 'rolling', target: 'goal1', targets: ['goal1', 'goal2', 'goal3'] }, f.text);
  expect(result.error).toBeUndefined();
  expect(result.status).toBe('won');
  expect(result.won).toBe(true);
  expect(result.shotsTaken).toBe(3);
  expect(result.strokesPerTarget).toEqual([1, 2, 3].map(i => ({ target: `goal${i}`, strokes: 1, shots: 1, advanced: true })));
  expect(f.profiles).toEqual([1, 2, 3].map(i => ({ method: 'ground.profile', params: { from: [0, i * 10], to: [0, i * 10 + 5], samples: 200 } })));
  expect(f.poses.length).toBeGreaterThan(24);
  expect(result.lastPlan?.speed).toBeCloseTo(Math.sqrt(6));
  expect(f.restored()).toBe(true);
});

test('a ground profile failure returns an error report and restores the hand override', async () => {
  const f = runtimeFixture(true);
  const result = await playAimPolicy(f.runtime, { thing: 'rolling', target: 'goal1' }, f.text);
  expect(result.error).toContain('ground.profile');
  expect(result.shotsTaken).toBe(0);
  expect(result.won).toBe(false);
  expect(f.poses).toHaveLength(0);
  expect(f.restored()).toBe(true);
});
