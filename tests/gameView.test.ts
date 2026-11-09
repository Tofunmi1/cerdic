// When a running game owns the view, the studio's own camera controls step aside (wheel zoom, click-to-look, hotkeys the game binds).
import { describe, expect, test } from 'bun:test';
import { boundKeys, gameCamera, gameRunning, viewerHotkeyWanted } from '../src/studio/gameView';

test('gameRunning reads the present flag and survives bad text', () => {
  expect(gameRunning(JSON.stringify({ present: true }))).toBe(true);
  expect(gameRunning(JSON.stringify({ present: false }))).toBe(false);
  expect(gameRunning('')).toBe(false);
});

const status = (camera: unknown) => JSON.stringify({ present: true, camera });

describe('gameCamera', () => {
  test('is the active camera of a running game, or null without one', () => {
    expect(gameCamera(status({ name: 'side', mode: 'side', orbit: false }))).toEqual({ name: 'side', mode: 'side', orbit: false });
    expect(gameCamera(status({ name: 'behind', mode: 'third', orbit: true }))?.orbit).toBe(true);
    expect(gameCamera(status(null))).toBeNull();
    expect(gameCamera(JSON.stringify({ present: false, camera: null }))).toBeNull();
    expect(gameCamera('not json')).toBeNull();
    expect(gameCamera('')).toBeNull();
  });
});

describe('boundKeys', () => {
  test('are every key a game binds, from the input table', () => {
    const table = JSON.stringify([
      { name: 'move', kind: 'axis', axes: ['x', 'z'], keys: ['KeyD', 'KeyA', 'KeyW', 'KeyS'], pad: [], label: 'Move' },
      { name: 'reload', kind: 'press', axes: [], keys: ['KeyR'], pad: [], label: 'Reload' },
    ]);
    expect([...boundKeys(table)].sort()).toEqual(['KeyA', 'KeyD', 'KeyR', 'KeyS', 'KeyW']);
    expect(boundKeys('').size).toBe(0);
    expect(boundKeys('garbage').size).toBe(0);
  });
});

describe('viewerHotkeyWanted', () => {
  const keys = new Set(['KeyR', 'KeyW']);
  test('the studio hotkey is skipped when the running game binds that key', () => {
    expect(viewerHotkeyWanted('KeyR', keys, true)).toBe(false);
    expect(viewerHotkeyWanted('KeyF', keys, true)).toBe(true);
  });
  test('without a running game every hotkey works as before', () => {
    expect(viewerHotkeyWanted('KeyR', keys, false)).toBe(true);
  });
});
