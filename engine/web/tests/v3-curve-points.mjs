// V3/G5: sampled curves are visible and authored 3D Bézier points can be moved in the viewport.
import assert from "node:assert/strict";
import { nodeId, openViewer, screenOf, select, settle, shutdown } from "./support-v3.mjs";

const source = [
  "path = curve.bezier(points=[[0,0,0],[0,0.4,0],[0.4,0.6,0],[0.4,1,0]], steps=24)",
  "section = profile.circle(radius=0.04, segments=16)",
  "pipe = solid.sweep(path, section)",
  "circle_path = curve.circle(center=[1,0,0], radius=0.25, segments=24)",
  "ring = solid.sweep(circle_path, section)",
  "loop_path = curve.polyline(points=[[-1,0,0],[-0.7,0,0],[-0.7,0,0.3],[-1,0,0.3],[-1,0,0]], closed=true)",
  "loop = solid.sweep(loop_path, section)",
  "output pipe = pipe",
  "output ring = ring",
  "output loop = loop",
].join("\n");

const viewer = await openViewer(source, "v3-g5-curve-points");
try {
  const { page } = viewer;
  const path = await page.evaluate((id) => window.__ve.doc.state().document.nodes.find((node) => node.id === id),
    await nodeId(page, "path"));
  const original = path.params.points;
  await select(page, path.id);
  await page.waitForFunction(() => document.querySelector("#viewer-gizmo .gizmo-curve")
    && document.querySelector('#viewer-gizmo [data-curve-point="1"]'));
  assert.equal(await page.locator("#viewer-gizmo .gizmo-control-polygon").count(), 1);

  const start = await screenOf(page, original[1]);
  await page.mouse.move(start.x, start.y);
  const frame = await page.evaluate(() => window.__ve.read().frames);
  await page.mouse.down();
  await page.evaluate((before) => new Promise((resolve) => {
    const tick = () => window.__ve.read().frames >= before + 2 ? resolve() : requestAnimationFrame(tick);
    tick();
  }), frame);
  await page.mouse.move(start.x + 28, start.y - 18, { steps: 6 });
  await page.waitForFunction(() => document.querySelector("#viewer-gizmo .gizmo-preview-curve"));
  assert.equal(await page.evaluate(() => window.__ve.doc.state().revision), 1,
    "the preview does not change authored state before release");
  await page.mouse.up();
  await settle(page, 2);
  const edited = await page.evaluate((id) => window.__ve.doc.state().document.nodes.find((node) => node.id === id).params.points, path.id);
  assert.notDeepEqual(edited[1], original[1], "the authored 3D control point changes");
  assert.equal(edited.length, original.length);
  await page.evaluate(() => window.__ve.doc.undo());
  await settle(page, 3);
  assert.deepEqual(await page.evaluate((id) => window.__ve.doc.state().document.nodes.find((node) => node.id === id).params.points, path.id), original);

  const circle = await nodeId(page, "circle_path");
  await select(page, circle);
  await page.waitForFunction(() => document.querySelector("#viewer-gizmo .gizmo-curve"));
  assert.equal(await page.locator('#viewer-gizmo [data-curve-point]').count(), 0,
    "a sampled circle is drawn without fake editable control points");
  await select(page, await nodeId(page, "loop_path"));
  await page.waitForFunction(() => document.querySelector("#viewer-gizmo .gizmo-curve"));
  const curvePixels = await page.locator("#viewer-gizmo .gizmo-curve").getAttribute("points");
  const projectedPoints = curvePixels.split(" ");
  assert.equal(projectedPoints[0], projectedPoints.at(-1), "the closed polyline overlay includes its seam");
  assert.deepEqual(viewer.errors, []);
  console.log("V3/G5 curve overlay and 3D control-point acceptance passed");
} finally {
  await viewer.close();
  await shutdown();
}
