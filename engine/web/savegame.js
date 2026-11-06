// Save games for the page. The engine writes a save as JSON text (the whole deterministic state of the running game); this stores it in the browser
// per document and slot, and loads it back. A save only fits the game it was written from: the engine refuses another game's save with the reason.
//
//   const saves = createSaves(api, { name: 'minigolf' });
//   await saves.save('quick');            // { ok: true, bytes }
//   await saves.load('quick');            // { ok: true } or { ok: false, error }
//   saves.list();                         // [{ slot, savedAt, bytes }]

const PREFIX = 've-save:';
const POLL_MS = 16;
const TIMEOUT_MS = 2000;

const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

/** Storage that never throws: private windows and blocked site data fall back to nothing saved. */
function safeStorage(storage) {
  return {
    get: (key) => { try { return storage.getItem(key); } catch { return null; } },
    set: (key, value) => { try { storage.setItem(key, value); return true; } catch { return false; } },
    remove: (key) => { try { storage.removeItem(key); } catch { /* nothing to do */ } },
    keys: () => { try { return Array.from({ length: storage.length }, (_, i) => storage.key(i)).filter(Boolean); } catch { return []; } },
  };
}

export function createSaves(api, { name = 'game', storage = globalThis.localStorage, sleep = wait } = {}) {
  const disk = safeStorage(storage ?? { getItem: () => null, setItem: () => { throw new Error('no storage'); }, removeItem: () => {}, length: 0, key: () => null });
  const keyOf = (slot) => `${PREFIX}${name}:${slot}`;

  /** Asks the running game for a save and keeps it in the browser. */
  async function save(slot = 'quick') {
    api.game_save_request();
    let text = '';
    for (let waited = 0; waited < TIMEOUT_MS; waited += POLL_MS) {
      await sleep(POLL_MS);
      text = api.game_save_take();
      if (text) break;
    }
    if (!text) return { ok: false, error: 'the game did not answer: is one running?' };
    const stored = disk.set(keyOf(slot), JSON.stringify({ savedAt: Date.now(), game: text }));
    return stored ? { ok: true, bytes: text.length } : { ok: false, error: 'the browser refused to store the save (storage full or blocked)' };
  }

  /** Loads a stored save into the running game. */
  async function load(slot = 'quick') {
    const raw = disk.get(keyOf(slot));
    if (!raw) return { ok: false, error: `no save in slot ${slot}` };
    let game;
    try { game = JSON.parse(raw).game; } catch { return { ok: false, error: 'the stored save is damaged' }; }
    const refused = api.game_save_load(game);
    if (refused) return { ok: false, error: refused };
    for (let waited = 0; waited < TIMEOUT_MS; waited += POLL_MS) {
      await sleep(POLL_MS);
      const result = api.game_save_result();
      if (result === 'ok') return { ok: true };
      if (result) return { ok: false, error: result };
    }
    return { ok: false, error: 'the game did not answer: is one running?' };
  }

  const list = () => disk.keys()
    .filter((key) => key.startsWith(`${PREFIX}${name}:`))
    .map((key) => {
      let parsed = {};
      try { parsed = JSON.parse(disk.get(key) ?? '{}'); } catch { /* damaged: listed with no details */ }
      return { slot: key.slice(`${PREFIX}${name}:`.length), savedAt: parsed.savedAt ?? 0, bytes: (parsed.game ?? '').length };
    })
    .sort((a, b) => b.savedAt - a.savedAt);

  return { save, load, list, remove: (slot) => disk.remove(keyOf(slot)) };
}
