// V3-07, V3-08, V3-14: a drag ends without a commit whenever anything but a clean release ends it, and
// never commits against a stale or busy document. Real mouse for the drags; the two events a browser
// cannot be told to send (a native pointercancel, a window blur) are dispatched on the real elements.
import assert from "node:assert/strict";
import { beginDrag, documentOf, nodeId, openViewer, revisionOf, select, settle, shutdown } from "./support-v3.mjs";

const viewer = await openViewer("base = solid.box(size=[0.4, 0.2, 0.3])\noutput part = base", "v3-guards");
const { page } = viewer;
try {
  const base = await nodeId(page, "base");
  await select(page, base);
  await page.evaluate(() => {
    window.__pointer = null;
    document.querySelector("#viewer-gizmo").addEventListener("pointerdown", (event) => { window.__pointer = event.pointerId; }, true);
  });
  const nodeCount = async () => (await documentOf(page)).nodes.length;
  const unchanged = async (revision, why) => {
    await page.mouse.up();
    assert.equal(await revisionOf(page), revision, `${why}: the revision did not move`);
    assert.equal(await nodeCount(), 1, `${why}: no transform node was authored`);
  };

  const step = (name) => console.log(`-- ${name}`);
  // V3-08: the document changes under a held handle. A rename moves the revision without any evaluation.
  step("stale revision");
  let revision = await revisionOf(page);
  let points = await beginDrag(page);
  const frame = await page.evaluate(() => window.__ve.read().frames);
  await page.evaluate((id) => window.__ve.doc.send([{ set_label: { node: id, label: "renamed" } }], "rename under the drag"), base);
  await settle(page, revision + 1);
  await page.waitForFunction((previous) => window.__ve.read().frames >= previous + 3, frame, { timeout: 60000 });
  await page.mouse.move(points.end.x, points.end.y, { steps: 4 });
  await page.waitForTimeout(500);
  await unchanged(revision + 1, "a stale revision");
  assert.equal(await page.locator("#viewer-gizmo .gizmo-preview").count(), 0, "the preview is cleared");

  step("lost capture");
  revision = await revisionOf(page);
  points = await beginDrag(page);
  await page.evaluate(() => document.querySelector("#viewer-gizmo").releasePointerCapture(window.__pointer));
  // The browser delivers `lostpointercapture` with the next pointer event for that pointer, not on a timer.
  await page.mouse.move(points.end.x, points.end.y);
  await page.waitForFunction(() => /capture lost/.test(document.querySelector("#viewer-gizmo-status").textContent), null, { timeout: 60000 });
  await unchanged(revision, "lost capture");

  step("pointer cancel");
  points = await beginDrag(page);
  await page.evaluate(() => document.querySelector("#viewer-gizmo").dispatchEvent(
    new PointerEvent("pointercancel", { pointerId: window.__pointer, bubbles: true })));
  await page.waitForFunction(() => /Drag cancelled/.test(document.querySelector("#viewer-gizmo-status").textContent), null, { timeout: 60000 });
  await unchanged(revision, "pointer cancel");

  step("window blur");
  points = await beginDrag(page);
  await page.evaluate(() => window.dispatchEvent(new Event("blur")));
  await page.waitForFunction(() => /lost focus/.test(document.querySelector("#viewer-gizmo-status").textContent), null, { timeout: 60000 });
  await unchanged(revision, "window blur");

  step("escape");
  points = await beginDrag(page);
  await page.keyboard.press("Escape");
  await unchanged(revision, "Escape");

  // V3-14: a handle pressed while an earlier edit is still being evaluated is refused with the reason, and
  // the last good scene stays selectable. The edit is sent and the press dispatched in one task, so the
  // staged document is certainly still pending when the handle sees it.
  const busy = await page.evaluate((id) => {
    const outcome = window.__ve.doc.send([{ set_param: { node: id, param: "size", value: [0.5, 0.2, 0.3] } }], "resize");
    const pending = Boolean(window.__ve.doc.state().pending_document);
    const hit = document.querySelector('#viewer-gizmo [data-axis="0"]');
    hit.dispatchEvent(new PointerEvent("pointerdown", { pointerId: 9, button: 0, bubbles: true }));
    return { outcome, pending, message: document.querySelector("#viewer-gizmo-status").textContent };
  }, base);
  assert(busy.outcome.pending || busy.outcome.committed, "the resize was accepted");
  assert(busy.pending, "the edit was still being evaluated when the handle was pressed");
  assert.match(busy.message, /still being evaluated/, "the refusal says why");
  await settle(page, revision + 1);
  assert.equal(await nodeCount(), 1, "the refused press authored nothing");
  const after = await revisionOf(page);
  points = await beginDrag(page);
  await page.mouse.move(points.end.x, points.end.y, { steps: 4 });
  await page.mouse.up();
  await settle(page, after + 1);
  assert.equal(await nodeCount(), 2, "once the evaluation finished, the same handle edits again");

  assert.deepEqual(viewer.errors, [], "no page errors");
  console.log("V3 drag guards acceptance passed");
} finally {
  await viewer.close();
  await shutdown();
}
