import { expect, test } from 'bun:test';
// @ts-expect-error plain module with no declarations
import { createSaves } from '../engine/web/savegame.js';

class Disk {
  data = new Map<string, string>();
  get length() { return this.data.size; }
  key(i: number) { return [...this.data.keys()][i] ?? null; }
  getItem(k: string) { return this.data.get(k) ?? null; }
  setItem(k: string, v: string) { this.data.set(k, v); }
  removeItem(k: string) { this.data.delete(k); }
}

/** A stand-in for the runtime: saving answers after a frame, loading checks the game name. */
function runtime(game: string) {
  let pending = '';
  let result = '';
  return {
    state: { value: 'state-of-' + game },
    game_save_request() { pending = ''; setTimeout(() => { pending = JSON.stringify({ game, state: runtimeState.value }); }, 5); },
    game_save_take() { return pending; },
    game_save_load(text: string) {
      let parsed; try { parsed = JSON.parse(text); } catch { return 'not a save game'; }
      result = '';
      setTimeout(() => { result = parsed.game === game ? 'ok' : 'snapshot belongs to a different game definition'; if (result === 'ok') runtimeState.value = parsed.state; }, 5);
      return '';
    },
    game_save_result() { return result; },
  };
}
const runtimeState = { value: 'start' };
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

test('a save is stored per game and slot and loads back into the running game', async () => {
  const disk = new Disk();
  const api = runtime('clock');
  const saves = createSaves(api, { name: 'clock', storage: disk, sleep });
  runtimeState.value = 'after-90-steps';
  const saved = await saves.save('a');
  expect(saved.ok).toBe(true);
  expect(saved.bytes).toBeGreaterThan(10);
  runtimeState.value = 'something-else';
  expect(await saves.load('a')).toEqual({ ok: true });
  expect(runtimeState.value).toBe('after-90-steps');
  expect(saves.list().map((s: { slot: string }) => s.slot)).toEqual(['a']);
});

test('a save from another game is refused with the engine\'s reason, a missing slot and a damaged one are reported', async () => {
  const disk = new Disk();
  const other = createSaves(runtime('other'), { name: 'clock', storage: disk, sleep });
  await other.save('x');
  const saves = createSaves(runtime('clock'), { name: 'clock', storage: disk, sleep });
  const refused = await saves.load('x');
  expect(refused.ok).toBe(false);
  expect(refused.error).toContain('different game');
  expect((await saves.load('nope')).error).toContain('no save in slot');
  disk.setItem('ve-save:clock:bad', '{broken');
  expect((await saves.load('bad')).error).toContain('damaged');
});

test('storage that refuses or is missing does not throw', async () => {
  const full = { length: 0, key: () => null, getItem: () => null, removeItem() {}, setItem() { throw new Error('quota'); } };
  const saves = createSaves(runtime('clock'), { name: 'clock', storage: full, sleep });
  const saved = await saves.save('a');
  expect(saved.ok).toBe(false);
  expect(saved.error).toContain('refused');
  expect(saves.list()).toEqual([]);
});

test('with no game running the save says so instead of waiting forever', async () => {
  const silent = { game_save_request() {}, game_save_take: () => '', game_save_load: () => '', game_save_result: () => '' };
  const saves = createSaves(silent, { name: 'x', storage: new Disk(), sleep: () => Promise.resolve() });
  const saved = await saves.save('a');
  expect(saved.ok).toBe(false);
  expect(saved.error).toContain('is one running');
}, 10000);
