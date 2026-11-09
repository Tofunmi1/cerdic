// Save slots, an undo stack and settings (F3). The running game writes a save as text a frame after it is asked (`game_save_request`, then
// `game_save_take`) and loads one the same way (`game_save_load`, then `game_save_result`); this module waits for those answers, keeps the
// text in named slots with a little about it, and keeps the last few saves for stepping back. The slot store is injected: IndexedDB in the
// page (`idbStore`), a map in tests (`memoryStore`).

export interface SaveApi {
  game_save_request(): void;
  game_save_take(): string;
  game_save_load(text: string): string;
  game_save_result(): string;
  game_status(): string;
}
export type Slot = { name: string; savedAt: number; game: string; state: string; level: string; text: string };
export interface SlotStore {
  put(slot: Slot): Promise<void>;
  get(name: string): Promise<Slot | undefined>;
  remove(name: string): Promise<void>;
  all(): Promise<Slot[]>;
}
/** Waits for the next frame in the page; tests pass a function that returns at once. */
export type Tick = () => Promise<void>;
export const nextFrame: Tick = () => new Promise(resolve => requestAnimationFrame(() => resolve()));

/** Asks the game for a save and waits for it. */
export async function captureSave(api: SaveApi, tick: Tick = nextFrame, patience = 300): Promise<string> {
  api.game_save_request();
  for (let i = 0; i < patience; i++) {
    const text = api.game_save_take();
    if (text) return text;
    await tick();
  }
  throw new Error('the game did not answer the save request (is a game running?)');
}

/** Loads a save into the running game and waits for it to say whether it fitted. */
export async function loadSave(api: SaveApi, text: string, tick: Tick = nextFrame, patience = 300): Promise<void> {
  const refused = api.game_save_load(text);
  if (refused) throw new Error(refused);
  for (let i = 0; i < patience; i++) {
    const result = api.game_save_result();
    if (result === 'ok') return;
    if (result) throw new Error(result);
    await tick();
  }
  throw new Error('the game did not answer the load');
}

function about(api: SaveApi): { state: string; level: string } {
  try {
    const flow = (JSON.parse(api.game_status()) as { flow?: { state?: string; level?: string } }).flow;
    return { state: flow?.state ?? '', level: flow?.level ?? '' };
  } catch {
    return { state: '', level: '' };
  }
}

export async function saveSlot(api: SaveApi, store: SlotStore, name: string, game: string, tick: Tick = nextFrame, now: () => number = Date.now): Promise<Slot> {
  const trimmed = name.trim();
  if (!trimmed) throw new Error('a save needs a name');
  const text = await captureSave(api, tick);
  const slot: Slot = { name: trimmed, savedAt: now(), game, ...about(api), text };
  await store.put(slot);
  return slot;
}

export async function restoreSlot(api: SaveApi, store: SlotStore, name: string, tick: Tick = nextFrame): Promise<Slot> {
  const slot = await store.get(name.trim());
  if (!slot) throw new Error(`no save named "${name}"`);
  await loadSave(api, slot.text, tick);
  return slot;
}

export const deleteSlot = (store: SlotStore, name: string) => store.remove(name.trim());

/** The slots, newest first. */
export async function listSlots(store: SlotStore): Promise<Slot[]> {
  return (await store.all()).sort((a, b) => b.savedAt - a.savedAt);
}

export function memoryStore(): SlotStore {
  const slots = new Map<string, Slot>();
  return {
    put: async slot => void slots.set(slot.name, slot),
    get: async name => slots.get(name),
    remove: async name => void slots.delete(name),
    all: async () => [...slots.values()],
  };
}

/** Slots in the browser's IndexedDB. */
export function idbStore(databaseName = 've-saves'): SlotStore {
  const open = () =>
    new Promise<IDBDatabase>((resolve, reject) => {
      const request = indexedDB.open(databaseName, 1);
      request.onupgradeneeded = () => request.result.createObjectStore('slots', { keyPath: 'name' });
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => reject(request.error ?? new Error('IndexedDB is not available'));
    });
  const run = async <T>(mode: IDBTransactionMode, work: (store: IDBObjectStore) => IDBRequest<T>): Promise<T> => {
    const db = await open();
    return new Promise<T>((resolve, reject) => {
      const request = work(db.transaction('slots', mode).objectStore('slots'));
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => reject(request.error ?? new Error('IndexedDB failed'));
    });
  };
  return {
    put: async slot => void (await run('readwrite', s => s.put(slot))),
    get: async name => (await run<Slot | undefined>('readonly', s => s.get(name))) ?? undefined,
    remove: async name => void (await run('readwrite', s => s.delete(name))),
    all: () => run<Slot[]>('readonly', s => s.getAll()),
  };
}

/** The last few saves, newest on top. */
export class UndoStack {
  private readonly entries: string[] = [];
  constructor(private readonly limit = 20) {}
  get size() { return this.entries.length; }
  push(text: string) {
    this.entries.push(text);
    if (this.entries.length > this.limit) this.entries.shift();
  }
  undo(): string | undefined { return this.entries.pop(); }
}

export type Settings = { volume: number; reducedMotion: boolean; shake: number };
const DEFAULT_SETTINGS: Settings = { volume: 0.8, reducedMotion: false, shake: 1 };
type Storage = { getItem(key: string): string | null; setItem(key: string, value: string): void };

/** The saved settings; anything missing, out of range or unreadable is the default, and blocked storage is the defaults. */
export function loadSettings(storage: Storage): Settings {
  try {
    const raw: unknown = JSON.parse(storage.getItem('ve.settings') ?? '{}');
    const value = (raw && typeof raw === 'object' ? raw : {}) as Record<string, unknown>;
    const number = (key: 'volume' | 'shake', max: number) => (typeof value[key] === 'number' && (value[key] as number) >= 0 && (value[key] as number) <= max ? (value[key] as number) : DEFAULT_SETTINGS[key]);
    return {
      volume: number('volume', 1),
      shake: number('shake', 2),
      reducedMotion: typeof value.reducedMotion === 'boolean' ? value.reducedMotion : DEFAULT_SETTINGS.reducedMotion,
    };
  } catch {
    return { ...DEFAULT_SETTINGS };
  }
}

export function saveSettings(storage: Storage, settings: Settings): void {
  try {
    storage.setItem('ve.settings', JSON.stringify(settings));
  } catch {
    /* blocked storage: the settings last until the page closes */
  }
}
