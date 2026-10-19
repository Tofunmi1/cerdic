// V3-02: transform insertion rewires every dependant and named output in one command batch.
import assert from "node:assert/strict";
import { chromium } from "playwright";
import { serve } from "./support.mjs";
import { insertTransformBatch, parameterHandleCommand, viewportEditConflict, viewportEditConflictReason } from "../panels/transform-commands.js";

const doc = {
  groups: [],
  nodes: [
    { id: "n_source", op: "solid.box@1", label: "source", inputs: {} },
    { id: "n_union", op: "solid.union@1", inputs: { in: [{ node: "n_source", port: "out" }, { node: "n_source", port: "out" }] } },
    { id: "n_move", op: "xform.move@1", inputs: { in: [{ node: "n_source", port: "out" }] } },
  ],
  outputs: [{ name: "body", from: { node: "n_source", port: "out" } }],
};

const batch = insertTransformBatch(doc, "n_source", {
  mode: "rotate", axis: 1, radians: Math.PI / 2, pivot: [1, 0, 0],
});
assert.equal(batch.length, 12, "three transforms, their links, every rewire and the output update are atomic");
assert.deepEqual(batch.slice(0, 3).map((command) => command.add_node.op), [
  "xform.move@1", "xform.rotate@1", "xform.move@1",
]);
assert.deepEqual(batch[1].add_node.params.angles, [0, Math.PI / 2, 0]);
assert.deepEqual(batch[0].add_node.params.by, [-1, 0, 0]);
assert.deepEqual(batch[2].add_node.params.by, [1, 0, 0]);
assert.deepEqual(batch.filter((command) => command.disconnect).map((command) => command.disconnect.to.index), [1, 0]);
assert.deepEqual(batch.filter((command) => command.connect && command.connect.to.node === "n_union")
  .map((command) => command.connect.to.index), [0, 1]);
assert.equal(batch.find((command) => command.set_output)?.set_output.from.node, "$v3_2");
assert.equal(batch.filter((command) => command.set_output).length, 1);
assert.deepEqual(insertTransformBatch(doc, "missing", { mode: "translate", delta: [1, 0, 0] }), null);
assert.deepEqual(insertTransformBatch(doc, "n_source", { mode: "scale", axis: 0, factor: -1 }), null);
const existingRotate = { ...doc.nodes[2], op: "xform.rotate@1", params: { angles: [0.2, 0, 0] } };
const pivotedRotate = insertTransformBatch({ ...doc, nodes: [existingRotate] }, existingRotate.id,
  { mode: "rotate", axis: 1, radians: 0.5, pivot: [1, 0, 0] });
assert.equal(pivotedRotate[0].add_node.op, "xform.move@1", "a nonzero pivot inserts a composing transform chain");
assert.equal(pivotedRotate.some((command) => command.set_param), false, "pivoted rotation does not silently edit Euler components");
const composedRotate = insertTransformBatch({ ...doc, nodes: [existingRotate] }, existingRotate.id,
  { mode: "rotate", axis: 1, radians: 0.5 });
assert.equal(composedRotate[0].add_node.op, "xform.rotate@1", "nonzero existing Euler angles compose as a world rotation node");
assert.equal(composedRotate.some((command) => command.set_param), false);
// V3-03 singularity rule: the drag never decomposes a composed rotation back into Euler angles, so a
// node already at gimbal lock (Y = 90 degrees) cannot lose or scramble components; it gets a new rotation.
const gimbal = { ...existingRotate, params: { angles: [0, Math.PI / 2, 0] } };
const aroundX = insertTransformBatch({ ...doc, nodes: [gimbal] }, gimbal.id, { mode: "rotate", axis: 0, radians: 0.3 });
assert.equal(aroundX.some((command) => command.set_param), false, "the locked node is left untouched");
assert.deepEqual(aroundX[0].add_node.params.angles, [0.3, 0, 0], "the turn is its own world-space X rotation");
const existingMove = { ...doc.nodes[2], params: { by: [1, 2, 3] } };
assert.deepEqual(insertTransformBatch({ ...doc, nodes: [existingMove] }, existingMove.id,
  { mode: "translate", delta: [0.5, 0, -1] }), [
  { set_param: { node: existingMove.id, param: "by", value: [1.5, 2, 2] } },
]);

const grouped = {
  groups: [{ id: "g_body", output: "n_member" }],
  nodes: [{ id: "n_member", op: "solid.box@1", group: "g_body", inputs: {} }],
  outputs: [],
};
const groupBatch = insertTransformBatch(grouped, "n_member", { mode: "translate", delta: [0, 2, 0] });
assert.equal(groupBatch[0].add_node.group, "g_body");
assert.equal(groupBatch.at(-1).set_group_output.node, "$v3");

assert.deepEqual(parameterHandleCommand({ nodes: [{ id: "array", op: "xform.array@1", params: { spacing: [0.3, 0.4, 0.5] } }] },
  "array", "spacing", 1, { kind: "translate", value: [0, 0.2, 0] }),
[{ set_param: { node: "array", param: "spacing", value: [0.3, 0.4 + 0.2, 0.5] } }]);
assert.deepEqual(parameterHandleCommand({ nodes: [{ id: "frame", op: "frame.add@1", params: { position: [1, 2, 3], rotation: [0, 0, 0] } }] },
  "frame", "position", 2, { kind: "translate", value: [0, 0, -0.25] }),
[{ set_param: { node: "frame", param: "position", value: [1, 2, 2.75] } }]);
assert.deepEqual(parameterHandleCommand({ nodes: [{ id: "frame", op: "frame.add@1", params: { rotation: [0, 0, 0] } }] },
  "frame", "rotation", 0, { kind: "rotate", radians: Math.PI / 4 }),
[{ set_param: { node: "frame", param: "rotation", value: [Math.PI / 4, 0, 0] } }]);
assert.deepEqual(parameterHandleCommand({ nodes: [{ id: "lathe", op: "solid.revolve@1", params: { angle: 1 } }] },
  "lathe", "revolve.angle", 1, { kind: "rotate", radians: 0.25 }),
[{ set_param: { node: "lathe", param: "angle", value: 1.25 } }]);
assert.equal(viewportEditConflict({ revision: 4 }, { node: "base" }, "base", 4), false);
assert.equal(viewportEditConflict({ revision: 5 }, { node: "base" }, "base", 4), true, "stale revision conflicts");
assert.equal(viewportEditConflict({ revision: 4, pending_document: {} }, { node: "base" }, "base", 4), true,
  "a candidate document conflicts while it is pending");
assert.equal(viewportEditConflict({ revision: 4 }, { node: "other" }, "base", 4), true,
  "selection changes conflict");
const reason = (current, selected = "base") => viewportEditConflictReason(current, { node: selected }, "base", 4);
assert.equal(reason({ revision: 4 }), null);
assert.equal(reason({ revision: 5 }), "revision", "the document moved: the message says so");
assert.equal(reason({ revision: 4 }, "other"), "selection");
assert.equal(reason({ revision: 4, pending_document: {} }), "pending", "an edit still evaluating is its own reason, not 'changed'");
assert.equal(reason({ revision: 5, pending_document: {} }), "revision", "a moved revision wins over pending");

// V3-04: the chain a pivoted edit authors leaves the pivot fixed. The chain is applied to points the
// way the evaluator would (a move adds, a scale multiplies, a rotation turns), in authored order.
const applyChain = (batch, point) => batch.filter((command) => command.add_node).reduce((p, { add_node: { op, params } }) => {
  if (op === "xform.move@1") return p.map((v, i) => v + params.by[i]);
  if (op === "xform.scale@1") return p.map((v, i) => v * params.by[i]);
  const [x, y, z] = params.angles;
  const turn = ([a, b, c]) => [a, b * Math.cos(x) - c * Math.sin(x), b * Math.sin(x) + c * Math.cos(x)];
  const aboutY = ([a, b, c]) => [a * Math.cos(y) + c * Math.sin(y), b, -a * Math.sin(y) + c * Math.cos(y)];
  const aboutZ = ([a, b, c]) => [a * Math.cos(z) - b * Math.sin(z), a * Math.sin(z) + b * Math.cos(z), c];
  return aboutZ(aboutY(turn(p)));
}, point);
const near = (actual, expected, why) => assert(actual.every((v, i) => Math.abs(v - expected[i]) < 1e-12), `${why}: ${actual} vs ${expected}`);
const lone = { groups: [], nodes: [{ id: "n_box", op: "solid.box@1", inputs: {} }], outputs: [] };
const pivot = [1, 2, 3];
for (const axis of [0, 1, 2]) {
  const rotate = insertTransformBatch(lone, "n_box", { mode: "rotate", axis, radians: 0.7, pivot });
  near(applyChain(rotate, pivot), pivot, `a rotation about axis ${axis} leaves its pivot fixed`);
  const scale = insertTransformBatch(lone, "n_box", { mode: "scale", axis, factor: 2.5, pivot });
  near(applyChain(scale, pivot), pivot, `a scale along axis ${axis} leaves its pivot fixed`);
  assert(applyChain(rotate, [4, 5, 6]).some((v, i) => Math.abs(v - [4, 5, 6][i]) > 1e-6), "other points do move");
}
near(applyChain(insertTransformBatch(lone, "n_box", { mode: "rotate", axis: 2, radians: Math.PI / 2, pivot }), [2, 2, 3]), [1, 3, 3],
  "a quarter turn about Z around (1,2,3) takes (2,2,3) to (1,3,3)");
near(applyChain(insertTransformBatch(lone, "n_box", { mode: "scale", axis: 0, factor: 2, pivot }), [2, 2, 3]), [3, 2, 3],
  "doubling along X about (1,2,3) takes (2,2,3) to (3,2,3)");
near(applyChain(insertTransformBatch(lone, "n_box", { mode: "rotate", axis: 1, radians: 1.1 }), [0, 0, 0]), [0, 0, 0],
  "with no pivot the world origin is the pivot");

const { server, port } = await serve();
const browser = await chromium.launch({ args: ["--enable-unsafe-swiftshader", "--ignore-gpu-blocklist"] });
try {
  const page = await browser.newPage();
  const text = "base = solid.box(size=[0.4, 0.2, 0.3])\njoined = solid.union(base, base)\noutput base_part = base\noutput joined_part = joined";
  await page.goto(`http://127.0.0.1:${port}/?viewer=1&eval=1&docname=v3-insert-${Date.now()}`);
  await page.waitForFunction(() => window.__ve?.ready && window.__ve.evaluation?.state.worker === "idle", null, { timeout: 90000 });
  await page.evaluate((source) => window.__ve.doc.sendText(source, "V3 fixture"), text);
  await page.waitForFunction(() => window.__ve.doc.state().revision === 1 && window.__ve.evaluation.state.worker === "idle", null, { timeout: 60000 });
  const original = await page.evaluate(() => window.__ve.doc.state().document);
  const applied = await page.evaluate(async () => {
    const { insertTransformBatch } = await import("./panels/transform-commands.js");
    const document = window.__ve.doc.state().document;
    const base = document.nodes.find((node) => node.label === "base");
    const commands = insertTransformBatch(document, base.id, { mode: "translate", delta: [0, 0.5, 0] });
    return { outcome: window.__ve.doc.send(commands, "Move part"), commands };
  });
  assert(applied.outcome.pending || applied.outcome.committed, `the transform batch applies through the normal document API: ${JSON.stringify(applied.outcome)}`);
  await page.waitForFunction(() => window.__ve.doc.state().revision === 2 && window.__ve.evaluation.state.worker === "idle", null, { timeout: 60000 });
  const edited = await page.evaluate(() => window.__ve.doc.state().document);
  const move = edited.nodes.find((node) => node.op === "xform.move@1");
  assert(move, "the selected geometry gets an authored move node");
  assert.deepEqual(move.inputs.in, [{ node: original.nodes.find((node) => node.label === "base").id, port: "out" }]);
  const join = edited.nodes.find((node) => node.label === "joined");
  assert(join.inputs.in.every((link) => link.node === move.id), "both variadic consumers now read the transform");
  assert.equal(edited.outputs.find((output) => output.name === "base_part").from.node, move.id, "named outputs are rewired");
  await page.evaluate(() => window.__ve.doc.undo());
  await page.waitForFunction(() => window.__ve.doc.state().revision === 3 && window.__ve.evaluation.state.worker === "idle", null, { timeout: 60000 });
  const undone = await page.evaluate(() => window.__ve.doc.state().document);
  assert.deepEqual(undone.nodes, original.nodes, "one undo removes the insertion and all rewires");
  assert.deepEqual(undone.outputs, original.outputs, "one undo restores named outputs");
  console.log("V3 transform command builder acceptance passed");
} finally {
  await browser.close();
  server.close();
}
