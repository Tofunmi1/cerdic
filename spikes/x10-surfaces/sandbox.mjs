// The "code" surface: a JavaScript builder run in an isolated vm context. Calling an operator
// function records a node; nothing else is reachable (no require, process, fs, timers).
import vm from 'node:vm';
import { NODES } from './lang.mjs';

export function runScript(src) {
  const nodes = [], outputs = [], sites = new Map(), counts = {};
  const isHandle = (v) => v && typeof v === 'object' && typeof v.__n === 'string';
  const line = () => { const m = new Error().stack.split('\n').map((s) => s.match(/model\.js:(\d+):/)).find(Boolean); return m ? Number(m[1]) : 0; };
  const ctx = {}; // intrinsics (Math, Array, JSON ...) come from the new context itself
  for (const kind of Object.keys(NODES)) {
    ctx[kind] = (...args) => {
      if (nodes.length >= 600) throw new Error('too many nodes (limit 600)');
      let params = {};
      const last = args[args.length - 1];
      if (last && typeof last === 'object' && !isHandle(last) && !Array.isArray(last)) { params = last; args = args.slice(0, -1); }
      const inputs = args.flat().map((a) => { if (!isHandle(a)) throw new Error(`${kind}: inputs must be nodes returned by other calls, got ${JSON.stringify(a)}`); return a.__n; });
      counts[kind] = (counts[kind] ?? 0) + 1;
      const name = `${kind}_${counts[kind]}`;
      nodes.push({ name, kind, inputs, params });
      sites.set(name, line());
      return { __n: name };
    };
  }
  ctx.output = (...hs) => { for (const h of hs.flat()) { if (!isHandle(h)) throw new Error('output() needs nodes'); outputs.push(h.__n); } };
  try {
    vm.runInNewContext(src, vm.createContext(ctx), { filename: 'model.js', timeout: 2000 });
  } catch (e) {
    const m = String(e.stack ?? '').match(/model\.js:(\d+):/);
    return { ok: false, errors: [{ ln: m ? Number(m[1]) : 0, msg: String(e.message ?? e) }] };
  }
  return { ok: true, ir: { nodes, outputs }, sites };
}
