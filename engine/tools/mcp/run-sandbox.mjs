const workerScope = globalThis;
workerScope.process = undefined;
workerScope.require = undefined;
workerScope.fetch = undefined;
workerScope.WebSocket = undefined;
workerScope.XMLHttpRequest = undefined;
workerScope.Worker = undefined;
workerScope.importScripts = undefined;

let nextCall = 0;
const awaiting = new Map();
const logs = [];

const printable = (value) => {
  if (typeof value === 'string') return value;
  try { return JSON.stringify(value); } catch { return String(value); }
};
const log = (...values) => logs.push(values.map(printable).join(' '));
const tools = new Proxy({}, {
  get: (_, name) => (...args) => new Promise((resolve, reject) => {
    const id = ++nextCall;
    awaiting.set(id, { resolve, reject });
    postMessage({ type: 'tool', id, name: String(name), args: args[0] ?? {} });
  }),
});

addEventListener('message', async ({ data }) => {
  if (data?.type === 'tool_result') {
    const pending = awaiting.get(data.id);
    if (!pending) return;
    awaiting.delete(data.id);
    if (data.isError) pending.reject(new Error(data.text));
    else pending.resolve(data.text);
    return;
  }
  if (data?.type !== 'execute') return;
  try {
    if (/\bimport\b\s*(?:\(|[\w{*])/.test(data.code)) throw new Error('Import is not allowed in run code');
    const hidden = new Set(['process', 'require', 'fetch', 'Bun', 'WebSocket', 'XMLHttpRequest', 'Worker', 'importScripts']);
    const safeGlobal = new Proxy({}, { get: (_, key) => hidden.has(key) ? undefined : workerScope[key] });
    const AsyncFunction = Object.getPrototypeOf(async function () {}).constructor;
    const fn = new AsyncFunction('tools', 'log', 'process', 'require', 'fetch', 'Bun', 'globalThis', data.code);
    let result = await fn(tools, log, undefined, undefined, undefined, undefined, safeGlobal);
    let encoded;
    try { encoded = JSON.stringify(result); } catch { throw new Error('run result is not JSON serializable'); }
    if (encoded === undefined) encoded = 'null';
    let truncated = false;
    if (encoded.length > 8000) {
      truncated = true;
      if (typeof result === 'string') result = result.slice(0, 8000);
      else result = encoded.slice(0, 8000);
    }
    postMessage({ type: 'done', result, logs, truncated });
  } catch (error) {
    postMessage({ type: 'failure', error: error?.message ?? String(error) });
  }
});
