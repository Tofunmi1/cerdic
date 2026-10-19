// V3-05: profile point handles edit the same `points` parameter as the inspector.
import assert from "node:assert/strict";
import { chromium } from "playwright";
import { serve } from "./support.mjs";

const { server, port } = await serve();
const browser = await chromium.launch({ args: ["--enable-unsafe-swiftshader", "--ignore-gpu-blocklist"] });
try {
  const page = await browser.newPage({ viewport: { width: 1100, height: 760 } });
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto(`http://127.0.0.1:${port}/?viewer=1&eval=1&docname=v3-profile-${Date.now()}`);
  await page.waitForFunction(() => window.__ve?.ready && window.__ve.evaluation?.state.worker === "idle", null, { timeout: 90000 });
  const source = "outline = profile.polygon(points=[[-0.3,-0.2],[0.3,-0.2],[0,0.3]])\nshape = solid.extrude(outline, height=0.3)\noutput part = shape";
  await page.evaluate((text) => window.__ve.doc.sendText(text, "Profile handle fixture"), source);
  await page.waitForFunction(() => window.__ve.doc.state().revision === 1 && window.__ve.evaluation.state.worker === "idle", null, { timeout: 60000 });
  const node = await page.evaluate(() => window.__ve.doc.state().document.nodes.find((candidate) => candidate.label === "outline"));
  await page.click(`.dock .orow[data-id="${node.id}"]`);
  await page.waitForFunction(() => document.querySelector('#viewer-gizmo [data-profile-point="0"]'), null, { timeout: 10000 });
  const drag = await page.evaluate(async (index) => {
    const runtime = await import("./ve_runtime.js");
    const document = window.__ve.doc.state().document;
    const profile = document.nodes.find((candidate) => candidate.label === "outline");
    const point = profile.params.points[index];
    const start = JSON.parse(runtime.viewer_project(point[0], point[1], 0, innerWidth, innerHeight));
    const finish = JSON.parse(runtime.viewer_project(point[0] + 0.2, point[1], 0, innerWidth, innerHeight));
    return { start, finish, original: profile.params.points };
  }, 0);
  await page.mouse.move(drag.start.x, drag.start.y); await page.mouse.down();
  await page.mouse.move(drag.finish.x, drag.finish.y, { steps: 8 });
  await page.waitForFunction(() => /Move ·/.test(document.querySelector("#viewer-gizmo-status").textContent), null, { timeout: 10000 });
  assert(await page.locator("#viewer-gizmo .gizmo-preview-profile").count() === 1,
    "the candidate profile outline previews before geometry evaluation");
  assert.equal(await page.evaluate(() => window.__ve.doc.state().revision), 1,
    "profile preview leaves authored state unchanged until release");
  await page.mouse.up();
  await page.waitForFunction(() => window.__ve.doc.state().revision === 2 && window.__ve.evaluation.state.worker === "idle", null, { timeout: 60000 });
  const edited = await page.evaluate(() => window.__ve.doc.state().document.nodes.find((candidate) => candidate.label === "outline").params.points);
  assert(Math.abs(edited[0][0] - drag.original[0][0] - 0.2) < 1e-5, `the handle edits points[0].x: ${edited[0]}`);
  assert.equal(edited[0][1], drag.original[0][1], "the constrained move keeps the other profile coordinate");
  await page.evaluate(() => window.__ve.doc.undo());
  await page.waitForFunction(() => window.__ve.doc.state().revision === 3 && window.__ve.evaluation.state.worker === "idle", null, { timeout: 60000 });
  assert.deepEqual(await page.evaluate(() => window.__ve.doc.state().document.nodes.find((candidate) => candidate.label === "outline").params.points), drag.original);
  assert.deepEqual(errors, []);
  console.log("V3 profile point acceptance passed");
} finally {
  await browser.close();
  server.close();
}
