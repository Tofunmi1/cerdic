// Pure helpers over a document's graph, shared by the panels and unit tested without a browser.
// A document is `{ nodes, outputs, groups }`. A node is `{ id, op, label, inputs: { port: [{ node, port }] },
// flags, group?, instance? }`: `group` marks a member of a group's definition (not a top-level node),
// `instance` (`{ group, version, overrides }`) marks a use of a group. A group is `{ id, name, version,
// output, params: [{ name, node, param }] }`.

/// The operator's family without its version: "solid.box@1" -> "solid.box".
export const family = (op) => String(op).split("@")[0];

/// A short name for a node: its label, or its operator.
export const nodeName = (node) => node.label || family(node.op);

const inputsOf = (node) => Object.values(node.inputs ?? {}).flat().map((ref) => ref.node);

/// The nodes of the top level: everything that is not a member of a group's definition.
export const topLevel = (doc) => doc.nodes.filter((node) => !node.group);

/// The nodes whose results are shown: the named outputs, or (when none are named) every top-level node nothing reads.
export function sinks(doc) {
  const named = (doc.outputs ?? []).map((output) => output.from.node);
  if (named.length) return new Set(named);
  const scope = topLevel(doc);
  const read = new Set(scope.flatMap(inputsOf));
  return new Set(scope.filter((node) => !read.has(node.id)).map((node) => node.id));
}

/// The groups whose parts include `group`'s: itself and every group that (however deep) holds an instance of it.
function usersOf(doc, group) {
  const seen = new Set([group]);
  for (let grew = true; grew;) {
    grew = false;
    for (const node of doc.nodes) {
      if (node.group && node.instance && seen.has(node.instance.group) && !seen.has(node.group)) {
        seen.add(node.group);
        grew = true;
      }
    }
  }
  return seen;
}

/// The shown parts a node (or a group, by its id) builds: itself when it is shown, and every shown node
/// that reads it, however far down. A member of a group builds every part its group is used in.
export function partsFor(doc, id) {
  const readers = new Map();
  for (const node of doc.nodes) {
    for (const source of inputsOf(node)) {
      if (!readers.has(source)) readers.set(source, []);
      readers.get(source).push(node.id);
    }
  }
  const owner = doc.groups?.some((g) => g.id === id) ? id : doc.nodes.find((n) => n.id === id)?.group;
  const start = new Set([id]);
  if (owner) {
    const users = usersOf(doc, owner);
    for (const node of topLevel(doc)) if (node.instance && users.has(node.instance.group)) start.add(node.id);
  }
  const seen = new Set(start);
  const queue = [...start];
  while (queue.length) {
    for (const reader of readers.get(queue.pop()) ?? []) {
      if (!seen.has(reader)) { seen.add(reader); queue.push(reader); }
    }
  }
  const shown = sinks(doc);
  return [...seen].filter((node) => shown.has(node));
}

/// The group an instance node is a use of, and what to know about it: whether it is behind the
/// group's version, what an upgrade would drop, and the group's input names. `null` for other nodes.
export function instanceInfo(doc, node) {
  if (!node?.instance) return null;
  const group = doc.groups?.find((g) => g.id === node.instance.group);
  if (!group) return null;
  const inputs = doc.nodes
    .filter((n) => n.group === group.id && family(n.op) === "group.input")
    .map((n) => n.params?.name)
    .filter(Boolean);
  const outdated = node.instance.version !== group.version;
  const exposed = new Set(group.params.map((p) => p.name));
  return {
    group, inputs, outdated, version: node.instance.version,
    droppedOverrides: outdated ? Object.keys(node.instance.overrides ?? {}).filter((name) => !exposed.has(name)) : [],
    droppedLinks: outdated ? Object.keys(node.inputs ?? {}).filter((port) => !inputs.includes(port)) : [],
  };
}

/// A group's exposed parameters with the member node that holds each.
export function exposedParams(doc, group) {
  return group.params
    .map((p) => ({ ...p, node: doc.nodes.find((n) => n.id === p.node) }))
    .filter((p) => p.node);
}

/// Rows for the nodes of one scope reachable from `roots`, each with what builds it underneath. A node
/// read twice is expanded once and listed again as a reference.
function tree(doc, scope, roots, collapsed, base, expanded, names) {
  const byId = new Map(scope.map((node) => [node.id, node]));
  const groupName = new Map((doc.groups ?? []).map((g) => [g.id, g]));
  const rows = [];
  const walk = (id, depth) => {
    const node = byId.get(id);
    if (!node) return;
    const children = [...new Set(inputsOf(node))].filter((child) => byId.has(child));
    const reference = expanded.has(id);
    const info = instanceInfo(doc, node);
    rows.push({
      id, depth: base + depth, name: nodeName(node), op: node.op, output: depth === 0 && base === 0 ? names.get(id) ?? null : null,
      muted: !!node.flags?.muted, bypass: !!node.flags?.bypass,
      children: reference ? 0 : children.length, reference, collapsed: collapsed.has(id),
      instanceOf: info ? info.group.name : node.instance ? groupName.get(node.instance.group)?.name ?? "?" : null,
      outdated: !!info?.outdated,
    });
    if (reference) return;
    expanded.add(id);
    if (!collapsed.has(id)) for (const child of children) walk(child, depth + 1);
  };
  for (const id of roots) walk(id, 0);
  return rows;
}

/// The outliner as a flat list of rows: each shown part with the nodes that build it underneath, then
/// anything no part uses, then each group with its members. `collapsed` is a Set of node and group ids
/// whose children are hidden.
export function outlineRows(doc, collapsed = new Set()) {
  const names = new Map((doc.outputs ?? []).map((output) => [output.from.node, output.name]));
  const scope = topLevel(doc);
  const expanded = new Set();
  const shown = (doc.outputs ?? []).length ? [...new Set(doc.outputs.map((o) => o.from.node))] : [...sinks(doc)];
  const rows = tree(doc, scope, shown, collapsed, 0, expanded, names);
  const unused = scope.filter((node) => !expanded.has(node.id));
  if (unused.length) {
    // The tops of the unused nodes: the ones no other unused node reads.
    const readByUnused = new Set(unused.flatMap(inputsOf));
    rows.push({ header: "Not used by a part" });
    const tops = unused.filter((n) => !readByUnused.has(n.id)).map((n) => n.id);
    rows.push(...tree(doc, scope, tops, collapsed, 0, expanded, names));
  }
  for (const group of doc.groups ?? []) {
    rows.push({
      group: group.id, name: group.name, version: group.version, collapsed: collapsed.has(group.id),
      instances: doc.nodes.filter((n) => n.instance?.group === group.id).length,
    });
    if (collapsed.has(group.id)) continue;
    const members = doc.nodes.filter((n) => n.group === group.id);
    const seen = new Set();
    rows.push(...tree(doc, members, [group.output], collapsed, 1, seen, names));
    const stray = members.filter((n) => !seen.has(n.id));
    const readByStray = new Set(stray.flatMap(inputsOf));
    const tops = stray.filter((n) => !readByStray.has(n.id)).map((n) => n.id);
    rows.push(...tree(doc, members, tops, collapsed, 1, seen, names).map((row) => ({ ...row, unused: true })));
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
