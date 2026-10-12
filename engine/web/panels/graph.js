// Pure helpers over a document's graph, shared by the panels and unit tested without a browser.
// A document is `{ nodes: [{ id, op, label, inputs: { port: [{ node, port }] }, flags }], outputs: [{ name, from: { node } }] }`.

/// The operator's family without its version: "solid.box@1" -> "solid.box".
export const family = (op) => String(op).split("@")[0];

/// A short name for a node: its label, or its operator.
export const nodeName = (node) => node.label || family(node.op);

const inputsOf = (node) => Object.values(node.inputs ?? {}).flat().map((ref) => ref.node);

/// The nodes whose results are shown: the named outputs, or (when none are named) every node nothing reads.
export function sinks(doc) {
  const named = (doc.outputs ?? []).map((output) => output.from.node);
  if (named.length) return new Set(named);
  const read = new Set(doc.nodes.flatMap(inputsOf));
  return new Set(doc.nodes.filter((node) => !read.has(node.id)).map((node) => node.id));
}

/// The shown parts a node builds: itself when it is shown, and every shown node that reads it, however far down.
export function partsFor(doc, id) {
  const readers = new Map();
  for (const node of doc.nodes) {
    for (const source of inputsOf(node)) {
      if (!readers.has(source)) readers.set(source, []);
      readers.get(source).push(node.id);
    }
  }
  const seen = new Set([id]);
  const queue = [id];
  while (queue.length) {
    for (const reader of readers.get(queue.pop()) ?? []) {
      if (!seen.has(reader)) { seen.add(reader); queue.push(reader); }
    }
  }
  const shown = sinks(doc);
  return [...seen].filter((node) => shown.has(node));
}

/// The outliner as a flat list of rows: each shown part with the nodes that build it underneath, then
/// anything no part uses. A node read twice is expanded once and listed again as a reference.
/// `collapsed` is a Set of node ids whose children are hidden.
export function outlineRows(doc, collapsed = new Set()) {
  const byId = new Map(doc.nodes.map((node) => [node.id, node]));
  const names = new Map((doc.outputs ?? []).map((output) => [output.from.node, output.name]));
  const rows = [];
  const expanded = new Set();
  const walk = (id, depth) => {
    const node = byId.get(id);
    if (!node) return;
    const children = [...new Set(inputsOf(node))].filter((child) => byId.has(child));
    const reference = expanded.has(id);
    rows.push({
      id, depth, name: nodeName(node), op: node.op, output: depth === 0 ? names.get(id) ?? null : null,
      muted: !!node.flags?.muted, bypass: !!node.flags?.bypass,
      children: reference ? 0 : children.length, reference, collapsed: collapsed.has(id),
    });
    if (reference) return;
    expanded.add(id);
    if (!collapsed.has(id)) for (const child of children) walk(child, depth + 1);
  };
  const shown = (doc.outputs ?? []).length ? [...new Set(doc.outputs.map((o) => o.from.node))] : [...sinks(doc)];
  for (const id of shown) walk(id, 0);
  const unused = doc.nodes.filter((node) => !expanded.has(node.id));
  if (unused.length) {
    // The tops of the unused nodes: the ones no other unused node reads.
    const readByUnused = new Set(unused.flatMap(inputsOf));
    rows.push({ header: "Not used by a part" });
    for (const node of unused.filter((n) => !readByUnused.has(n.id))) walk(node.id, 0);
  }
  return rows;
}

/// Diagnostics grouped by the node they name (and the nodes they list as related), newest source first.
export function diagnosticsByNode(...lists) {
  const map = new Map();
  for (const list of lists) {
    for (const diagnostic of list ?? []) {
      if (!diagnostic) continue;
      for (const id of new Set([diagnostic.node, ...(diagnostic.related ?? [])].filter(Boolean))) {
        if (!map.has(id)) map.set(id, []);
        map.get(id).push(diagnostic);
      }
    }
  }
  return map;
}

/// Radians shown as degrees and back, for angle parameters.
export const toDegrees = (radians) => (radians * 180) / Math.PI;
export const fromDegrees = (degrees) => (degrees * Math.PI) / 180;

/// A number shown without floating point noise: at most `digits` decimals, no trailing zeros.
export function show(value, digits = 4) {
  const text = Number(value).toFixed(digits);
  return text.includes(".") ? text.replace(/0+$/, "").replace(/\.$/, "") : text;
}
