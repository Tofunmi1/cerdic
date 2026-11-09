// Save slots, the undo stack and settings (F3): the game gives a save as text a frame after it is asked; slots keep that text under a name
// with a little about it; the stack keeps the last few saves so a player can step back. The store is injected: IndexedDB in the page, a map here.
import { describe, expect, test } from 'bun:test';
import { type SaveApi, UndoStack, captureSave, deleteSlot, listSlots, loadSave, loadSettings, memoryStore, restoreSlot, saveSlot, saveSettings } from '../src/studio/saveSlots';

/** A game that answers a save request after `delay` polls and accepts a load after `delay` polls. */
function fakeGame(delay = 2, text = '{"tick":42}'): SaveApi & { loaded: string[]; status: string } {
  let asked = 0, wait = 0, pending = '';
  const game = {
    loaded: [] as string[],
    status: JSON.stringify({ present: true, flow: { state: 'playing', level: 'cave' }, steps: 42 }),
    game_save_request() { asked++; wait = delay; },
    game_save_take() { return asked > 0 && wait-- <= 0 ? text : ''; },
    game_save_load(t: string) { if (!t.startsWith('{')) return 'not a save game'; game.loaded.push(t); wait = delay; pending = 'ok'; return ''; },
    game_save_result() { return pending && wait-- <= 0 ? pending : ''; },
    game_status() { return game.status; },
  };
  return game;
}
const instant = async () => {};

describe('capture and load', () => {
  test('a save is read once the game has written it', async () => {
    expect(await captureSave(fakeGame(3), instant)).toBe('{"tick":42}');
  });
  test('a game that never answers is an error, not a hang', async () => {
    const mute = { ...fakeGame(), game_save_take: () => '' };
    await expect(captureSave(mute, instant, 20)).rejects.toThrow(/did not answer/);
  });
  test('a load waits for the game to say whether the save fitted', async () => {
    const game = fakeGame(2);
    await loadSave(game, '{"tick":1}', instant);
    expect(game.loaded).toEqual(['{"tick":1}']);
    await expect(loadSave(game, 'junk', instant)).rejects.toThrow(/not a save game/);
  });
});

describe('slots', () => {
  test('saving keeps the text with the game, state and time, and the list is newest first', async () => {
    const store = memoryStore();
    const game = fakeGame();
    await saveSlot(game, store, 'one', 'platformer', instant, () => 1000);
    await saveSlot(game, store, 'two', 'platformer', instant, () => 2000);
    const slots = await listSlots(store);
    expect(slots.map(s => s.name)).toEqual(['two', 'one']);
    expect(slots[0]).toMatchObject({ game: 'platformer', state: 'playing', level: 'cave', savedAt: 2000 });
  });
  test('saving under an existing name replaces it', async () => {
    const store = memoryStore();
    await saveSlot(fakeGame(1, '{"a":1}'), store, 'one', 'g', instant, () => 1);
    await saveSlot(fakeGame(1, '{"a":2}'), store, 'one', 'g', instant, () => 2);
    expect(await listSlots(store)).toHaveLength(1);
    const game = fakeGame();
    await restoreSlot(game, store, 'one', instant);
    expect(game.loaded).toEqual(['{"a":2}']);
  });
  test('a slot can be deleted, and restoring a missing one says so', async () => {
    const store = memoryStore();
    await saveSlot(fakeGame(), store, 'one', 'g', instant);
    await deleteSlot(store, 'one');
    expect(await listSlots(store)).toEqual([]);
    await expect(restoreSlot(fakeGame(), store, 'one', instant)).rejects.toThrow(/no save named/);
  });
  test('slot names are trimmed and empty ones refused', async () => {
    const store = memoryStore();
    await expect(saveSlot(fakeGame(), store, '   ', 'g', instant)).rejects.toThrow(/name/);
    await saveSlot(fakeGame(), store, '  first  ', 'g', instant);
    expect((await listSlots(store))[0].name).toBe('first');
  });
});

describe('undo', () => {
  test('steps back through the last saves and stops at the oldest', () => {
    const stack = new UndoStack(3);
    ['a', 'b', 'c'].forEach(s => stack.push(s));
    expect([stack.undo(), stack.undo(), stack.undo()]).toEqual(['c', 'b', 'a']);
    expect(stack.undo()).toBeUndefined();
  });
  test('keeps only the most recent entries', () => {
    const stack = new UndoStack(2);
    ['a', 'b', 'c', 'd'].forEach(s => stack.push(s));
    expect(stack.size).toBe(2);
    expect([stack.undo(), stack.undo(), stack.undo()]).toEqual(['d', 'c', undefined]);
  });
});

describe('settings', () => {
  test('are saved, read back, and fall back to defaults for anything missing or broken', () => {
    const store = new Map<string, string>();
    const storage = { getItem: (k: string) => store.get(k) ?? null, setItem: (k: string, v: string) => void store.set(k, v) };
    expect(loadSettings(storage)).toEqual({ volume: 0.8, reducedMotion: false, shake: 1 });
    saveSettings(storage, { volume: 0.3, reducedMotion: true, shake: 0.5 });
    expect(loadSettings(storage)).toEqual({ volume: 0.3, reducedMotion: true, shake: 0.5 });
    store.set('ve.settings', '{"volume":"loud","shake":9}');
    expect(loadSettings(storage)).toEqual({ volume: 0.8, reducedMotion: false, shake: 1 });
    store.set('ve.settings', 'nonsense');
    expect(loadSettings(storage).volume).toBe(0.8);
  });
  test('storage that throws is survivable', () => {
    const broken = { getItem: () => { throw new Error('blocked'); }, setItem: () => { throw new Error('blocked'); } };
    expect(loadSettings(broken).volume).toBe(0.8);
    expect(() => saveSettings(broken, { volume: 1, reducedMotion: false, shake: 1 })).not.toThrow();
  });
});
