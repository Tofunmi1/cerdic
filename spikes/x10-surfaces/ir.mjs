// The shared intermediate form. Every control surface lowers to {nodes, outputs}; the DSL text is
// only a serialisation used to run the one evaluator.
import { NODES, compile } from './lang.mjs';

export const ID_RE = /^[A-Za-z_]\w*$/;

export function irToDsl(ir) {
  const done = new Set(), lines = [], pending = [...ir.nodes];
  while (pending.length) {
    const before = pending.length;
    for (let i = 0; i < pending.length; i++) {
      const n = pending[i];
      if (n.inputs.every((r) => done.has(r))) {
        const args = [...n.inputs, ...Object.entries(n.params ?? {}).map(([k, v]) => `${k}=${JSON.stringify(v)}`)];
        lines.push(`${n.name} = ${n.kind}(${args.join(', ')})`); done.add(n.name); pending.splice(i, 1); i--;
      }
    }
    if (pending.length === before) return { error: `cycle or missing input among: ${pending.map((n) => n.name).join(', ')}` };
  }
  for (const o of ir.outputs) lines.push(`output ${o}`);
  return { text: lines.join('\n') };
}

export function dslToIr(text) {
  const c = compile(text);
  if (!c.ok) return { ok: false, errors: c.errors.map((e) => ({ ln: e.ln, msg: e.msg })) };
  return { ok: true, compiled: c, ir: { nodes: c.nodes.map((n) => ({ name: n.name, kind: n.kind, inputs: n.inputs, params: n.params })), outputs: c.outputs } };
}

export function irToJs(ir) {
  const lines = [];
  for (const n of ir.nodes) {
    const args = [...n.inputs];
    if (Object.keys(n.params ?? {}).length) args.push(JSON.stringify(n.params).replace(/"(\w+)":/g, '$1: '));
    lines.push(`const ${n.name} = ${n.kind}(${args.join(', ')});`);
  }
  for (const o of ir.outputs) lines.push(`output(${o});`);
  return lines.join('\n');
}

export function irToCommands(ir) {
  return [...ir.nodes.map((n) => ({ op: 'add', id: n.name, kind: n.kind, inputs: n.inputs, params: n.params })), ...ir.outputs.map((id) => ({ op: 'output', id }))];
}

// order-independent fingerprint of one node's content (kind and parameters), for edit-drift metrics
export const nodeKey = (n) => `${n.kind}:${JSON.stringify(Object.fromEntries(Object.entries(n.params ?? {}).sort()))}`;

export function retention(before, after) {
  const pool = new Map();
  for (const n of after.nodes) pool.set(nodeKey(n), (pool.get(nodeKey(n)) ?? 0) + 1);
  let kept = 0;
  for (const n of before.nodes) { const k = nodeKey(n), c = pool.get(k) ?? 0; if (c > 0) { kept++; pool.set(k, c - 1); } }
  const names = new Set(after.nodes.filter((n) => n.name).map((n) => n.name + ':' + n.kind));
  const stable = before.nodes.filter((n) => names.has(n.name + ':' + n.kind)).length;
  return { kept: kept / Math.max(before.nodes.length, 1), idStable: stable / Math.max(before.nodes.length, 1), before: before.nodes.length, after: after.nodes.length };
}

export const KINDS = Object.keys(NODES);
