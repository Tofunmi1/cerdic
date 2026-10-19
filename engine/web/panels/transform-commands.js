// Viewport transform edits are authored command batches. This builder only changes graph structure;
// runtime drag math supplies the deltas and the document API owns validation and history.
const ref = (node) => ({ node, port: "out" });
const connect = (from, node, port, index) => ({ connect: { from: ref(from), to: { node, port, ...(index === undefined ? {} : { index }) } } });

function axisVector(axis) {
  if (![0, 1, 2].includes(axis)) throw new RangeError("transform axis must be X, Y or Z");
  return [0, 1, 2].map((index) => index === axis ? 1 : 0);
}

/** Builds one atomic insertion-and-rewire batch for a world-space viewport transform. */
export function insertTransformBatch(document, nodeId, edit, alias = "$v3") {
  const source = document.nodes.find((node) => node.id === nodeId);
  if (!source || source.group && !document.groups?.some((group) => group.id === source.group)) return null;
  if (!/^\$[A-Za-z0-9_]{1,32}$/.test(alias)) throw new RangeError("transform alias is invalid");
  const pivot = edit.pivot ?? [0, 0, 0];
  if (pivot.length !== 3 || !pivot.every(Number.isFinite)) throw new RangeError("pivot must be finite XYZ");

  const direct = editOnTransform(source, edit, pivot);
  if (direct) return [direct];

  const operations = operationsFor(edit, pivot);
  if (!operations) return null;
  const aliases = operations.map((_, index) => operations.length === 1 ? alias : `${alias.slice(0, 25)}_${index}`);
  const commands = operations.map((operation, index) => ({ add_node: {
    op: operation.op, params: operation.params, label: index === operations.length - 1 ? `Edit ${source.label || source.op}` : undefined,
    alias: aliases[index], ...(source.group ? { group: source.group } : {}),
  } }));
  commands.push(connect(nodeId, aliases[0], "in"));
  for (let index = 1; index < aliases.length; index++) commands.push(connect(aliases[index - 1], aliases[index], "in"));

  for (const consumer of document.nodes) {
    if (consumer.id === nodeId || consumer.group !== source.group) continue;
    for (const [port, links] of Object.entries(consumer.inputs ?? {})) {
      const indices = links.flatMap((link, index) => link.node === nodeId ? [index] : []);
      if (!indices.length) continue;
      if (links.length === 1) commands.push(connect(aliases.at(-1), consumer.id, port));
      else {
        for (const index of [...indices].reverse()) commands.push({ disconnect: { to: { node: consumer.id, port, index } } });
        for (const index of indices) commands.push(connect(aliases.at(-1), consumer.id, port, index));
      }
    }
  }
  for (const output of document.outputs ?? []) {
    if (output.from.node === nodeId) commands.push({ set_output: { name: output.name, from: ref(aliases.at(-1)) } });
  }
  if (source.group && document.groups.find((group) => group.id === source.group)?.output === nodeId) {
    commands.push({ set_group_output: { group: source.group, node: aliases.at(-1) } });
  }
  return commands;
}

function editOnTransform(node, edit, pivot) {
  const family = node.op.split("@")[0];
  if (edit.mode === "translate" && family === "xform.move" && edit.delta?.length === 3 && edit.delta.every(Number.isFinite)) {
    const before = node.params?.by ?? [0, 0, 0];
    return { set_param: { node: node.id, param: "by", value: before.map((value, axis) => value + edit.delta[axis]) } };
  }
  if (edit.mode === "rotate" && family === "xform.rotate" && Number.isFinite(edit.radians)
    && pivot.every((component) => component === 0)) {
    const axis = axisVector(edit.axis).indexOf(1);
    const before = node.params?.angles ?? [0, 0, 0];
    if (before.every((angle) => angle === 0)) {
      return { set_param: { node: node.id, param: "angles", value: before.map((value, index) => value + (index === axis ? edit.radians : 0)) } };
    }
  }
  if (edit.mode === "scale" && family === "xform.scale" && Number.isFinite(edit.factor) && edit.factor > 0
    && pivot.every((component) => component === 0)) {
    const axis = axisVector(edit.axis).indexOf(1);
    const before = node.params?.by ?? [1, 1, 1];
    return { set_param: { node: node.id, param: "by", value: before.map((value, index) => value * (index === axis ? edit.factor : 1)) } };
  }
  return null;
}

/** Sets the exact authored parameter a viewport handle represents. */
export function parameterHandleCommand(document, nodeId, parameter, axis, delta) {
  const node = document.nodes.find((candidate) => candidate.id === nodeId);
  if (!node || ![0, 1, 2].includes(axis) || !delta) return null;
  const family = node.op.split("@")[0];
  // Spacing and step belong to the pattern node, so an instance set is edited by its parameters; no
  // handle ever edits one placement's matrix (the set shares one prototype, ADR 0014).
  const vectorParams = parameter === "spacing" && ["xform.array", "inst.array"].includes(family) ? ["spacing", "translate"]
    : parameter === "step" && family === "inst.repeat" ? ["step", "translate"]
    : parameter === "position" && family === "frame.add" ? ["position", "translate"]
      : parameter === "rotation" && family === "frame.add" ? ["rotation", "rotate"] : [];
  const [param, mode] = vectorParams;
  if (param && mode === delta.kind) {
    const before = node.params?.[param] ?? [0, 0, 0];
    if (!Array.isArray(before) || before.length !== 3 || !before.every(Number.isFinite)) return null;
    const amount = mode === "translate" ? delta.value?.[axis] : delta.radians;
    if (!Number.isFinite(amount)) return null;
    return [{ set_param: { node: nodeId, param, value: before.map((value, index) => value + (index === axis ? amount : 0)) } }];
  }
  if (parameter === "revolve.angle" && family === "solid.revolve" && delta.kind === "rotate" && axis === 1
    && Number.isFinite(delta.radians)) {
    const angle = Math.max(1e-3, Math.min(2 * Math.PI, (node.params?.angle ?? 2 * Math.PI) + delta.radians));
    return [{ set_param: { node: nodeId, param: "angle", value: angle } }];
  }
  return null;
}

/** Why a drag must not apply: `revision` (the document moved), `selection` (another node is selected), or
 * `pending` (an earlier edit is still being evaluated); `null` when it may. A stale base is never committed. */
export function viewportEditConflictReason(current, selection, nodeId, revision) {
  if (current.revision !== revision) return "revision";
  if (selection.node !== nodeId) return "selection";
  return current.pending_document ? "pending" : null;
}

/** Rejects a drag before it can apply to a changed document or selection. */
export function viewportEditConflict(current, selection, nodeId, revision) {
  return viewportEditConflictReason(current, selection, nodeId, revision) !== null;
}

function operationsFor(edit, pivot) {
  if (edit.mode === "translate" && edit.delta?.length === 3 && edit.delta.every(Number.isFinite)) {
    return [{ op: "xform.move@1", params: { by: edit.delta } }];
  }
  if (edit.mode === "rotate" && Number.isFinite(edit.radians)) {
    const angles = axisVector(edit.axis).map((component) => component * edit.radians);
    return aroundPivot("xform.rotate@1", "angles", angles, pivot);
  }
  if (edit.mode === "scale" && Number.isFinite(edit.factor) && edit.factor > 0) {
    const factors = axisVector(edit.axis).map((component) => component ? edit.factor : 1);
    return aroundPivot("xform.scale@1", "by", factors, pivot);
  }
  return null;
}

function aroundPivot(op, param, value, pivot) {
  const center = pivot.some((component) => component !== 0);
  const moves = center ? [
    { op: "xform.move@1", params: { by: pivot.map((component) => component === 0 ? 0 : -component) } },
    { op, params: { [param]: value } },
    { op: "xform.move@1", params: { by: pivot } },
  ] : [{ op, params: { [param]: value } }];
  return moves;
}
