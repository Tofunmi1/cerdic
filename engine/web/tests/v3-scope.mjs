// V3-09, V3-10, V3-11, V3-13, V3-14 through the real page: feature picking and `selection.ambiguous`, a
// handle on a group member, instance sets (edit the set, refuse one placement), the diagnostic overlay on
// a real node failure, and a drag on the last good scene after a failed evaluation.
import assert from "node:assert/strict";
import {
  axisDrag, beginDrag, documentOf, lookAt, nodeId, openViewer, revisionOf, screenOf, select, settle, shutdown, viewportClick,
} from "./support-v3.mjs";

const problems = [];
const hashOf = (page) => page.evaluate(async () => JSON.parse((await import("./ve_runtime.js")).doc_candidate_state()).hash);
const selectionOf = (page) => page.evaluate(() => JSON.parse(window.__ve.viewer.api.selection_get()));

const step = (name) => console.log(`-- ${name}`);

// ---- V3-13 and V3-14: a failed evaluation names a node; the last good scene stays editable.
{
  step("failed evaluation and diagnostics");
  const viewer = await openViewer("base = solid.box(size=[0.4, 0.2, 0.3])\nturned = xform.align(base, from=[1, 0, 0], to=[0, 1, 0])\noutput part = turned", "v3-failed");
  const { page } = viewer;
  try {
    const turned = await nodeId(page, "turned");
    await select(page, turned);
    const hash = await hashOf(page);
    await page.evaluate((id) => window.__ve.doc.send([{ set_param: { node: id, param: "from", value: [0, 0, 0] } }], "zero direction"), turned);
    await page.waitForFunction((id) => window.__ve.evaluation.state.lastFailure?.node === id
      && window.__ve.evaluation.state.worker === "idle" && !window.__ve.doc.state().pending_document, turned, { timeout: 60000 });
    assert.equal(await revisionOf(page), 1, "the failed edit was not committed");
    assert.equal(await hashOf(page), hash, "the document the engine evaluates is the last good one");

    // V3-13: the overlay draws the failing node's bounds, with no selection at all, and edits nothing.
    await page.evaluate(async () => (await import("./ve_runtime.js")).selection_clear());
    await page.waitForFunction(() => JSON.parse(window.__ve.viewer.api.selection_get()).node === null, null, { timeout: 10000 });
    assert.equal(await page.locator("#viewer-gizmo .gizmo-diagnostic").count(), 0, "off by default");
    await page.check("#gizmo-diagnostics");
    await page.waitForFunction(() => document.querySelectorAll("#viewer-gizmo .gizmo-diagnostic").length === 12, null, { timeout: 10000 });
    assert.equal(await revisionOf(page), 1, "the overlay edits nothing");
    assert.equal(await hashOf(page), hash, "and does not change the evaluation hash");
    await page.uncheck("#gizmo-diagnostics");
    await page.waitForFunction(() => document.querySelectorAll("#viewer-gizmo .gizmo-diagnostic").length === 0, null, { timeout: 10000 });

    // V3-14: the last good scene is still selectable and a real drag still commits.
    await select(page, turned);
    const points = await beginDrag(page);
    await page.mouse.move(points.end.x, points.end.y, { steps: 4 });
    await page.mouse.up();
    await settle(page, 2);
    assert.equal((await documentOf(page)).nodes.some((node) => node.op === "xform.move@1"), true, "the drag authored a move");
    // The failure stays as history (the status panel keeps a crash visible) but is no longer a current problem.
    assert.equal(await page.evaluate(() => window.__ve.evaluation.state.failureCurrent), false, "a good evaluation supersedes the failure");
    await page.waitForSelector("#dock-problems", { state: "hidden", timeout: 10000 }); // so the Problems list no longer shows it
    await page.check("#gizmo-diagnostics");
    await page.evaluate(async () => (await import("./ve_runtime.js")).selection_clear());
    await page.waitForFunction(() => JSON.parse(window.__ve.viewer.api.selection_get()).node === null, null, { timeout: 10000 });
    assert.equal(await page.locator("#viewer-gizmo .gizmo-diagnostic").count(), 0, "and the overlay no longer outlines the node");
    await page.uncheck("#gizmo-diagnostics");
    problems.push(...viewer.errors);
  } finally { await viewer.close(); }
}

// ---- V3-10: a handle on a group member edits the definition, and says so.
{
  step("group member");
  const viewer = await openViewer(`group wheel
  tyre = solid.torus@1(major=0.3, minor=0.06, segments=48, tube_segments=16)
  hub = solid.cylinder@1(height=0.2, radius=0.08, segments=32)
  rim = solid.union@1(tyre, hub)
  expose radius = tyre.major
  result rim
end
front = wheel()
back = wheel(radius=0.4)
shifted = xform.move@1(back, by=[1.2, 0, 0])
output front
output shifted`, "v3-group");
  const { page } = viewer;
  try {
    const before = await documentOf(page);
    const tyre = before.nodes.find((node) => node.label === "tyre");
    const front = await nodeId(page, "front");
    const versions = before.groups.map((group) => group.version);
    await select(page, tyre.id);
    assert.match(await page.textContent("#dock-inspector .banner"), /Part of the group wheel/, "the inspector says it is the definition");
    const frontBefore = await page.evaluate((id) => window.__ve.evaluation.boundsByNode[id], front);
    const points = await beginDrag(page);
    await page.mouse.move(points.end.x, points.end.y, { steps: 4 });
    await page.mouse.up();
    await settle(page, 2);
    const after = await documentOf(page);
    const inserted = after.nodes.find((node) => node.op === "xform.move@1" && node.group);
    assert(inserted, "the move was authored inside the group's definition");
    assert.equal(inserted.group, tyre.group, "in the same group as the tyre");
    assert.deepEqual(after.groups.map((group) => group.version), versions, "no interface change, so no instance goes out of date");
    assert.equal(after.nodes.filter((node) => node.instance).every((node) => !node.outdated), true);
    const frontAfter = await page.evaluate((id) => window.__ve.evaluation.boundsByNode[id], front);
    assert.notDeepEqual(frontAfter, frontBefore, "every use of the group changed, not only the selected instance");
    problems.push(...viewer.errors);
  } finally { await viewer.close(); }
}

// ---- V3-11: edit an instance set through its parameters; one placement cannot be edited.
{
  step("instance set");
  const viewer = await openViewer("base = solid.box(size=[0.3, 0.2, 0.2])\ngrid = inst.array(base, count_x=3, count_y=1, count_z=1, spacing=[0.5, 0, 0])\noutput set = grid", "v3-set");
  const { page } = viewer;
  try {
    await lookAt(page, [0.5, 0.5, 2.6], [0.5, 0, 0]);
    const second = await screenOf(page, [0.5, 0, 0]);
    assert(second, "the middle copy is on screen");
    await viewportClick(page, second);
    step("  placement clicked");
    await page.waitForFunction(() => JSON.parse(window.__ve.viewer.api.selection_get()).instance !== null, null, { timeout: 15000 });
    assert.equal((await selectionOf(page)).instance, 1, "a real click picks the second placement");
    await page.waitForFunction(() => /individual placement edits are unavailable/.test(document.querySelector("#viewer-gizmo-status").textContent), null, { timeout: 10000 });
    assert.equal(await page.locator("#viewer-gizmo [data-axis]").count(), 0, "no handle is offered for one placement");
    assert.equal(await revisionOf(page), 1, "and nothing was edited");

    step("  placement refused");
    await select(page, await nodeId(page, "grid"));
    step("  set selected");
    const spacing ='#viewer-gizmo [data-axis="0"][data-parameter="spacing"]';
    const points = await axisDrag(page, spacing);
    await page.mouse.move(points.start.x, points.start.y);
    await page.mouse.down();
    await page.mouse.move(points.end.x, points.end.y, { steps: 6 });
    await page.mouse.up();
    await settle(page, 2);
    const grid = (await documentOf(page)).nodes.find((node) => node.label === "grid");
    assert.notDeepEqual(grid.params.spacing, [0.5, 0, 0], "the set's spacing changed");
    assert.equal((await documentOf(page)).nodes.length, 2, "no node was added: the pattern's own parameter was edited");
    problems.push(...viewer.errors);
  } finally { await viewer.close(); }
}

// ---- V3-09: pick the authored feature; a combining node is reported as ambiguous, never guessed.
{
  step("feature pick");
  const viewer = await openViewer(`one = solid.box(size=[0.4, 0.4, 0.4])
two = solid.sphere(radius=0.25, segments=24)
two_moved = xform.move(two, by=[0.35, 0, 0])
both = solid.union(one, two_moved)
lone = solid.box(size=[0.3, 0.3, 0.3])
lone_moved = xform.move(lone, by=[1.4, 0, 0])
output joined = both
output single = lone_moved`, "v3-feature");
  const { page } = viewer;
  try {
    const ids = { one: await nodeId(page, "one"), both: await nodeId(page, "both"), lone: await nodeId(page, "lone") };
    await lookAt(page, [0.6, 0.7, 3.2], [0.6, 0, 0]);
    await page.check("#pick-feature");

    await viewportClick(page, await screenOf(page, [-0.15, 0, 0.2]));
    await page.waitForSelector('#dock-inspector [data-code="selection.ambiguous"]', { timeout: 15000 });
    assert.equal((await selectionOf(page)).node, ids.both, "the part stays selected: nothing unrelated is picked");
    assert.deepEqual(await page.$$eval("#dock-inspector button.candidate", (b) => b.map((x) => x.dataset.candidate)).then((list) => list.sort()),
      [ids.one, await nodeId(page, "two_moved")].sort(), "the candidates are the union's operands");
    assert.match(await page.textContent('#dock-inspector [data-code="selection.ambiguous"]'), /could come from/);
    await page.click(`#dock-inspector button.candidate[data-candidate="${ids.one}"]`);
    await page.waitForFunction((id) => JSON.parse(window.__ve.viewer.api.selection_get()).node === id, ids.one, { timeout: 10000 });
    assert.equal(await page.locator('#dock-inspector [data-code="selection.ambiguous"]').count(), 0, "choosing one settles it");

    await viewportClick(page, await screenOf(page, [1.4, 0, 0.15]));
    await page.waitForFunction((id) => JSON.parse(window.__ve.viewer.api.selection_get()).node === id, ids.lone, { timeout: 15000 });
    assert.equal(await page.locator('#dock-inspector [data-code="selection.ambiguous"]').count(), 0,
      "a part built by one feature through single inputs resolves to that feature");
    assert.equal(await revisionOf(page), 1, "feature picking is view state: no command was sent");
    problems.push(...viewer.errors);
  } finally { await viewer.close(); }
}

await shutdown();
assert.deepEqual(problems, [], "no page errors");
console.log("V3 scope acceptance passed");
