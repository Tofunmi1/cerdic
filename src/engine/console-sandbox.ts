// The worker behind the page console (`console.ts`): runs one async JavaScript function body with `tools.<name>(args)` and `log(...)`.
// It has no page: no DOM, no storage, and the network and worker globals are hidden; the only way out is a tool call, which the page
// answers through the same typed commands as every other author. This is a guard against slips, not a hard sandbox against hostile
// code (a worker still has `self`); the code is the person's own.
/// <reference lib="webworker" />
const scope = self as unknown as Record<string, unknown>;
const HIDDEN = ['fetch', 'XMLHttpRequest', 'WebSocket', 'EventSource', 'Worker', 'SharedWorker', 'importScripts', 'indexedDB', 'caches', 'process', 'require', 'Bun'];
for (const name of HIDDEN) { try { scope[name] = undefined; } catch { /* a read-only global stays; the Proxy below hides it from the code */ } }

let nextCall = 0;
const awaiting = new Map<number, { resolve: (text: string) => void; reject: (error: Error) => void }>();
const logs: string[] = [];

const printable = (value: unknown) => {
  if (typeof value === 'string') return value;
  try { return JSON.stringify(value); } catch { return String(value); }
};
const log = (...values: unknown[]) => { logs.push(values.map(printable).join(' ')); };
const tools = new Proxy({}, {
  get: (_, name) => (...args: unknown[]) => new Promise<string>((resolve, reject) => {
    const id = ++nextCall;
    awaiting.set(id, { resolve, reject });
    postMessage({ type: 'tool', id, name: String(name), args: args[0] ?? {} });
  }),
});

addEventListener('message', async (event: MessageEvent) => {
  const data = event.data as { type?: string; id?: number; isError?: boolean; text?: string; code?: string; limit?: number } | undefined;
  if (data?.type === 'tool_result') {
    const pending = awaiting.get(data.id ?? -1);
    if (!pending) return;
    awaiting.delete(data.id ?? -1);
    if (data.isError) pending.reject(new Error(data.text));
    else pending.resolve(data.text ?? '');
    return;
  }
  if (data?.type !== 'execute') return;
  const limit = data.limit ?? 8000;
  try {
    const code = String(data.code ?? '');
    if (/\bimport\b\s*(?:\(|[\w{*])/.test(code)) throw new Error('import is not allowed in console code');
    const hidden = new Set(HIDDEN);
    const safeGlobal = new Proxy({}, { get: (_, key) => (typeof key === 'string' && hidden.has(key) ? undefined : scope[key as string]) });
    const AsyncFunction = Object.getPrototypeOf(async function () { /* constructor holder */ }).constructor as new (...args: string[]) => (...values: unknown[]) => Promise<unknown>;
    const fn = new AsyncFunction('tools', 'log', 'globalThis', ...HIDDEN, code);
    let result = await fn(tools, log, safeGlobal, ...HIDDEN.map(() => undefined));
    let encoded: string | undefined;
    try { encoded = JSON.stringify(result); } catch { throw new Error('the result is not JSON serializable'); }
    if (encoded === undefined) encoded = 'null';
    let truncated = false;
    if (encoded.length > limit) {
      truncated = true;
      result = typeof result === 'string' ? result.slice(0, limit) : encoded.slice(0, limit);
    }
    postMessage({ type: 'done', result, logs, truncated });
  } catch (error) {
    postMessage({ type: 'failure', error: error instanceof Error ? error.message : String(error), logs });
  }
});
