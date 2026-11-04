/**
 * Scripts: behaviour a model writes in JavaScript that runs against the running world (ADR 0065).
 *
 * Each script runs in its own Web Worker, so a mistake cannot freeze the page or reach it: the worker has no network, no DOM and a fixed,
 * small API (`world`, `on`, `every`, `log`, `time`, `random`). The page drives it frame by frame; a script that does not finish its
 * synchronous part within 250 ms (an endless loop) is terminated and told so. World calls go through `world_call`, which answers at the
 * next frame, so a script should batch (`world.batch`) what it changes every frame instead of awaiting one call per entity.
 * `Math.random` is replaced by a seeded generator so a script's behaviour can be replayed.
 */
import { allows, type ScriptPermissions } from './packs';

type Runtime = { world_call(method: string, params: string): number; world_take(id: number): string | undefined; world_events_since?(after: number): string };

type Script = {
  name: string;
  worker: Worker;
  source: string;
  logs: string[];
  errors: string[];
  frames: number;
  /** When the current frame message was sent, or 0 while the worker is idle. */
  frameSentAt: number;
  startedAt: number;
  stopped: string | null;
  /** Where it came from: a `script.js` node in the document (kept in step with it) or the `script` tool (lives until stopped). */
  owner: 'document' | 'tool';
  /** What a script from a pack may do (its manifest entry); absent for the page's own scripts, which keep the page's rules. */
  permissions?: ScriptPermissions;
};

/** What the evaluation says about a `script.js` node. */
export type ScriptDef = { name: string; source: string; enabled: boolean; node: string };

const FRAME_DEADLINE_MS = 250;
/** A gap this long between two page frames means the page, not a script, was busy. */
const STALL_MS = 100;
const MAX_LINES = 200;
const MAX_SCRIPTS = 16;
const MAX_SOURCE_BYTES = 64 * 1024;

const WORKER_SOURCE = `
'use strict';
const post = self.postMessage.bind(self);
let nextId = 1, seed = 12345, now = 0, ready = false;
const pending = new Map();
const frameHandlers = [], timers = [], eventHandlers = new Map();
const rng = () => { seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0; return seed / 4294967296; };
const text = (args) => args.map((a) => (typeof a === 'string' ? a : (() => { try { return JSON.stringify(a); } catch { return String(a); } })())).join(' ').slice(0, 1000);
const report = (where, error) => post({ type: 'error', message: where + ': ' + (error && error.message ? error.message : String(error)) });
const world = {
  call: (method, params) => new Promise((resolve, reject) => { const id = nextId++; pending.set(id, { resolve, reject }); post({ type: 'call', id, method, params: params || {} }); }),
  get: async (entity, component) => { const r = await world.call('world.get_components', { entity, components: [component] }); if (r.errors && r.errors[component]) throw new Error(r.errors[component]); return r.components[component]; },
  set: (entity, component, value) => world.call('world.insert_components', { entity, components: { [component]: value } }),
  batch: (ops) => world.call('world.batch', { ops }),
  parts: async () => (await world.call('world.parts')).parts,
  query: (params) => world.call('world.query', params || {}),
};
const on = (name, fn) => { if (typeof fn !== 'function') throw new Error('on(name, fn) needs a function'); if (name === 'frame') frameHandlers.push(fn); else { if (!eventHandlers.has(name)) eventHandlers.set(name, []); eventHandlers.get(name).push(fn); } };
const every = (seconds, fn) => { if (!(seconds > 0) || typeof fn !== 'function') throw new Error('every(seconds, fn) needs a positive number and a function'); timers.push({ seconds, fn, next: now + seconds }); };
const log = (...args) => post({ type: 'log', text: text(args) });
// The screen UI (bars, text, a colour flash, a shake) is drawn by the page; ui.set(id, spec) redraws in place, so it may be called every frame.
const ui = {
  set: (id, spec) => post({ type: 'ui', op: 'set', id: String(id), spec }),
  remove: (id) => post({ type: 'ui', op: 'remove', id: String(id) }),
  clear: () => post({ type: 'ui', op: 'clear' }),
  shake: (amount, seconds) => post({ type: 'ui', op: 'shake', spec: { amount, seconds } }),
  flash: (color, seconds, opacity) => post({ type: 'ui', op: 'flash', spec: { color, seconds, opacity } }),
};
// The view the player looks through: view.pose() answers {position: [x, y, z], rotation: [x, y, z, w]}.
// view.teleport([x, y, z], [targetX, targetY, targetZ]) puts the player at a position, looking at a target.
const view = {
  pose: () => new Promise((resolve, reject) => { const id = nextId++; pending.set(id, { resolve, reject }); post({ type: 'page', id, op: 'pose' }); }),
  teleport: (position, target) => post({ type: 'page', op: 'teleport', position, target }),
};
const time = () => now;
Math.random = rng;
// A worker starts with every capability, many of them on the global's prototype where deleting an own property does nothing: shadow each
// name with an undefined own property instead (a name this list misses is still reachable: a language with no ambient capabilities,
// such as Luau, is the end state).
for (const name of ['fetch', 'XMLHttpRequest', 'WebSocket', 'EventSource', 'importScripts', 'indexedDB', 'caches', 'BroadcastChannel', 'Worker', 'SharedWorker', 'close', 'WebTransport', 'navigator', 'location', 'crypto', 'performance', 'Notification', 'FileReader', 'FileReaderSync']) { try { Object.defineProperty(self, name, { value: undefined, writable: false, configurable: false }); } catch {} }
const guard = async (where, fn, ...args) => { try { await fn(...args); } catch (error) { report(where, error); } };
self.onmessage = async (event) => {
  const m = event.data;
  if (m.type === 'load') {
    try {
      const Async = Object.getPrototypeOf(async function () {}).constructor;
      await new Async('world', 'on', 'every', 'log', 'time', 'random', 'ui', 'view', m.source)(world, on, every, log, time, rng, ui, view);
      post({ type: 'loaded' });
    } catch (error) { report('while starting', error); post({ type: 'loaded' }); }
  } else if (m.type === 'frame') {
    now = m.t;
    for (const t of timers) if (now >= t.next) { t.next = now + t.seconds; guard('every(' + t.seconds + ')', t.fn, now); }
    for (const fn of frameHandlers) guard('frame', fn, m.dt, now);
    post({ type: 'frame_done' });
  } else if (m.type === 'event') {
    // A listener for "click" hears every click; one for "click:crate_left" hears only that part's (or effect's) event.
    const keys = [m.name];
    if (m.data && m.data.part) keys.push(m.name + ':' + m.data.part);
    // A key event is {code: 'KeyQ', down: true}: on('key') hears every key, on('key:KeyQ') one key (Mouse0, Mouse1, Mouse2 are the buttons while the mouse is locked to the view).
    if (m.data && m.data.code) keys.push(m.name + ':' + m.data.code);
    for (const key of keys) for (const fn of eventHandlers.get(key) || []) guard('on(' + key + ')', fn, m.data);
  } else if (m.type === 'reply') {
    const p = pending.get(m.id); if (!p) return; pending.delete(m.id);
    if (m.ok) p.resolve(m.result); else p.reject(new Error(m.error));
  }
};
`;

export type ScriptHost = ReturnType<typeof createScriptHost>;

/** What the page lends a script beyond the world: drawing its UI and reading where the player looks from. */
export type ScriptHooks = {
  /** Draws one UI message; returns a sentence when the spec was refused. */
  ui?: (script: string, message: { op: string; id?: unknown; spec?: unknown }) => string | null;
  /** Removes everything a script drew when it stops. */
  clearUi?: (script: string) => void;
  pose?: () => unknown;
  /** Puts the player at a position looking at a target. */
  teleport?: (position: number[], target: number[]) => void;
};

export function createScriptHost(runtime: Runtime, hooks: ScriptHooks = {}) {
  const scripts = new Map<string, Script>();
  const waiting: { script: Script; callId: number; runtimeId: number }[] = [];
  let timer = 0;
  let last = 0;
  // Events that happened before the first script started are not replayed: start from where the world is now.
  let lastEvent = runtime.world_events_since ? (JSON.parse(runtime.world_events_since(0)) as { last: number }).last : 0;
  const blobUrl = URL.createObjectURL(new Blob([WORKER_SOURCE], { type: 'text/javascript' }));

  const push = (lines: string[], line: string) => { lines.push(line); if (lines.length > MAX_LINES) lines.shift(); };

  function kill(script: Script, why: string) {
    script.worker.terminate();
    script.stopped = why;
    push(script.errors, why);
    scripts.delete(script.name);
    hooks.clearUi?.(script.name);
  }

  function tick() {
    const nowMs = performance.now();
    const dt = last ? Math.min(0.1, (nowMs - last) / 1000) : 0;
    // A page that was itself blocked for a while (a slow rendered frame) gave the worker no chance to answer: that is not the script's fault.
    const stalled = last !== 0 && nowMs - last > STALL_MS;
    last = nowMs;
    // What happened in the world since last frame (clicks, hovers, grabs, game effects): each script that listens hears it.
    if (runtime.world_events_since) {
      try {
        const fresh = JSON.parse(runtime.world_events_since(lastEvent)) as { events: Array<{ seq: number; name: string }>; last: number };
        lastEvent = fresh.last;
        for (const event of fresh.events) for (const script of scripts.values()) script.worker.postMessage({ type: 'event', name: event.name, data: event });
      } catch { /* an older runtime without events */ }
    }
    // Answers to calls the scripts made last frame.
    for (let i = waiting.length - 1; i >= 0; i--) {
      const w = waiting[i];
      const raw = runtime.world_take(w.runtimeId);
      if (raw === undefined) continue;
      waiting.splice(i, 1);
      const answer = JSON.parse(raw) as { ok: boolean; result?: unknown; error?: string };
      w.script.worker.postMessage({ type: 'reply', id: w.callId, ok: answer.ok, result: answer.result, error: answer.error });
    }
    for (const script of [...scripts.values()]) {
      if (script.frameSentAt) {
        if (stalled) script.frameSentAt = nowMs;
        else if (nowMs - script.frameSentAt > FRAME_DEADLINE_MS) kill(script, `the script did not finish a frame in ${FRAME_DEADLINE_MS} ms (an endless loop?) and was stopped`);
        continue;
      }
      script.frameSentAt = nowMs;
      script.frames++;
      script.worker.postMessage({ type: 'frame', dt, t: (nowMs - script.startedAt) / 1000 });
    }
    if (scripts.size > 0) timer = requestAnimationFrame(tick); else { timer = 0; last = 0; }
  }

  function start(name: string, source: string, owner: 'document' | 'tool' = 'tool', permissions?: ScriptPermissions) {
    if (!/^[a-z][a-z0-9_]{0,31}$/.test(name)) return { ok: false, error: 'a script name is lowercase letters, digits and underscores, starting with a letter (32 at most)' };
    if (new TextEncoder().encode(source).length > MAX_SOURCE_BYTES) return { ok: false, error: `the script is over ${MAX_SOURCE_BYTES} bytes` };
    if (!scripts.has(name) && scripts.size >= MAX_SCRIPTS) return { ok: false, error: `at most ${MAX_SCRIPTS} scripts run at once; stop one first` };
    stop(name);
    const worker = new Worker(blobUrl);
    const script: Script = { name, worker, source, logs: [], errors: [], frames: 0, frameSentAt: 0, startedAt: performance.now(), stopped: null, owner, permissions };
    scripts.set(name, script);
    worker.onmessage = (event: MessageEvent) => {
      const m = event.data as { type: string; id?: number; method?: string; params?: unknown; text?: string; message?: string; op?: string; spec?: unknown; position?: number[]; target?: number[] };
      // A script from a pack holds only what its manifest granted: UI, the view and each world method are checked here, on every use.
      const denied = (what: string) => push(script.errors, `permission denied: this script's pack does not grant ${what}`);
      if (m.type === 'ui') {
        if (script.permissions && !script.permissions.ui) { denied('drawing UI'); return; }
        const refused = hooks.ui?.(script.name, { op: String(m.op), id: m.id, spec: m.spec });
        if (refused) push(script.errors, `ui: ${refused}`);
      }
      else if (m.type === 'page' && script.permissions && !script.permissions.view) {
        denied(`the view (${String(m.op)})`);
        if (m.id !== undefined) worker.postMessage({ type: 'reply', id: m.id, ok: false, error: 'permission denied: the view is not granted to this script' });
      }
      else if (m.type === 'page' && m.op === 'teleport') {
        if (hooks.teleport && Array.isArray(m.position) && Array.isArray(m.target) && m.position.length === 3 && m.target.length === 3) hooks.teleport(m.position, m.target);
        else push(script.errors, 'view.teleport(position, target) needs two [x, y, z] arrays');
      }
      else if (m.type === 'page') {
        // Questions the worker cannot answer itself: where the player is looking from.
        try {
          if (m.op === 'pose' && hooks.pose) worker.postMessage({ type: 'reply', id: m.id, ok: true, result: hooks.pose() });
          else worker.postMessage({ type: 'reply', id: m.id, ok: false, error: `the page cannot answer ${String(m.op)} here` });
        } catch (error) { worker.postMessage({ type: 'reply', id: m.id, ok: false, error: String(error) }); }
      }
      else if (m.type === 'log') push(script.logs, m.text ?? '');
      else if (m.type === 'error') push(script.errors, m.message ?? 'error');
      else if (m.type === 'frame_done') script.frameSentAt = 0;
      else if (m.type === 'call') {
        if (!allows(script.permissions, String(m.method))) {
          denied(`the world method ${String(m.method)}`);
          worker.postMessage({ type: 'reply', id: m.id, ok: false, error: `permission denied: this script may not call ${String(m.method)}` });
          return;
        }
        const runtimeId = runtime.world_call(String(m.method), JSON.stringify(m.params ?? {}));
        if (!runtimeId) worker.postMessage({ type: 'reply', id: m.id, ok: false, error: 'the world request queue is full; try again next frame' });
        else waiting.push({ script, callId: Number(m.id), runtimeId });
      }
    };
    worker.onerror = (event) => { push(script.errors, `the script could not run: ${event.message}`); };
    // The first frame waits until the script has run to its end once (its on() and every() calls are registered by then).
    script.frameSentAt = performance.now();
    worker.postMessage({ type: 'load', source });
    const loaded = (event: MessageEvent) => { if ((event.data as { type: string }).type === 'loaded') { script.frameSentAt = 0; worker.removeEventListener('message', loaded); } };
    worker.addEventListener('message', loaded);
    if (!timer) timer = requestAnimationFrame(tick);
    return { ok: true, name };
  }

  function stop(name: string) {
    const script = scripts.get(name);
    if (!script) return false;
    script.worker.terminate();
    scripts.delete(name);
    hooks.clearUi?.(name);
    return true;
  }

  /**
   * Makes the running document scripts match what an evaluation lists: a new one starts, one whose source changed restarts, one that is
   * gone or disabled stops. Scripts started with the `script` tool are left alone. A script that is unchanged keeps running, so a re-evaluation
   * that did not touch it does not make it jump.
   */
  function sync(defs: ScriptDef[]) {
    const wanted = new Map(defs.filter(d => d.enabled).map(d => [d.name, d]));
    for (const script of [...scripts.values()]) {
      if (script.owner !== 'document') continue;
      const def = wanted.get(script.name);
      if (!def || def.source !== script.source) stop(script.name);
    }
    for (const def of wanted.values()) {
      const running = scripts.get(def.name);
      if (running && running.owner === 'document') continue;
      if (running) { push(running.errors, `the document now has a script named ${def.name}: the tool's script of that name was replaced`); }
      start(def.name, def.source, 'document');
    }
  }

  return {
    start,
    stop,
    sync,
    /** Raises an event in every script that listens for it (`on('touch', ...)`). */
    emit(name: string, data: unknown) { for (const script of scripts.values()) script.worker.postMessage({ type: 'event', name, data }); },
    list: () => [...scripts.values()].map(s => ({ name: s.name, owner: s.owner, frames: s.frames, errors: s.errors.length, lastError: s.errors.at(-1) ?? null, logs: s.logs.length })),
    logs: (name: string) => { const s = scripts.get(name); return s ? { ok: true, logs: [...s.logs], errors: [...s.errors], frames: s.frames } : { ok: false, error: `no script named ${name} is running` }; },
    source: (name: string) => scripts.get(name)?.source ?? null,
    stopAll() { for (const name of [...scripts.keys()]) stop(name); },
  };
}
