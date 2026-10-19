// V3-06: a profile radius handle edits the inspector's radius parameter.
import assert from "node:assert/strict";
import { chromium } from "playwright";
import { serve } from "./support.mjs";

const { server, port } = await serve();
const browser = await chromium.launch({ args: ["--enable-unsafe-swiftshader", "--ignore-gpu-blocklist"] });
try {
  const page = await browser.newPage({ viewport: { width: 1100, height: 760 } });
  await page.goto(`http://127.0.0.1:${port}/?viewer=1&eval=1&docname=v3-radius-${Date.now()}`);
  await page.waitForFunction(() => window.__ve?.ready && window.__ve.evaluation?.state.worker === "idle", null, { timeout: 90000 });
  const source = "outline = profile.circle(radius=0.2, segments=32)\nshape = solid.extrude(outline, height=0.3)\noutput part = shape";
  await page.evaluate((text) => window.__ve.doc.sendText(text, "Radius handle fixture"), source);
  await page.waitForFunction(() => window.__ve.doc.state().revision === 1 && window.__ve.evaluation.state.worker === "idle", null, { timeout: 60000 });
  const node = await page.evaluate(() => window.__ve.doc.state().document.nodes.find((candidate) => candidate.label === "outline"));
  await page.click(`.dock .orow[data-id="${node.id}"]`);
  await page.waitForFunction(() => document.querySelector('#viewer-gizmo [data-profile-radius="true"]'), null, { timeout: 10000 });
  const projected = await page.evaluate(async (radius) => {
    const runtime = await import("./ve_runtime.js");
    return [
      JSON.parse(runtime.viewer_project(radius, 0, 0, innerWidth, innerHeight)),
      JSON.parse(runtime.viewer_project(radius + 0.2, 0, 0, innerWidth, innerHeight)),
    ];
  }, node.params.radius);
  await page.mouse.move(projected[0].x, projected[0].y); await page.mouse.down();
  await page.mouse.move(projected[1].x, projected[1].y, { steps: 8 });
  await page.waitForFunction(() => /Move ·/.test(document.querySelector("#viewer-gizmo-status").textContent), null, { timeout: 10000 });
  assert(await page.locator("#viewer-gizmo .gizmo-preview-profile").count() === 1,
    "the candidate radius outline previews before geometry evaluation");
  assert.equal(await page.evaluate(() => window.__ve.doc.state().revision), 1,
    "radius preview leaves authored state unchanged until release");
  await page.mouse.up();
  await page.waitForFunction(() => window.__ve.doc.state().revision === 2 && window.__ve.evaluation.state.worker === "idle", null, { timeout: 60000 });
  const radius = await page.evaluate(() => window.__ve.doc.state().document.nodes.find((candidate) => candidate.label === "outline").params.radius);
  assert(Math.abs(radius - 0.4) < 1e-5, `the circle radius is authored through set_param (${radius})`);
  await page.evaluate(() => window.__ve.doc.undo());
  await page.waitForFunction(() => window.__ve.doc.state().revision === 3 && window.__ve.evaluation.state.worker === "idle", null, { timeout: 60000 });
  assert.equal(await page.evaluate(() => window.__ve.doc.state().document.nodes.find((candidate) => candidate.label === "outline").params.radius), 0.2);
  console.log("V3 radius handle acceptance passed");
} finally {
  await browser.close();
  server.close();
}
