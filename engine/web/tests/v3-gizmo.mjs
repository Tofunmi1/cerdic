// V3-01/02/07: desktop pointer drag commits one authored transform and Escape discards a drag.
import assert from "node:assert/strict";
import { chromium } from "playwright";
import { mkdir } from "node:fs/promises";
import { serve } from "./support.mjs";

const { server, port } = await serve();
const browser = await chromium.launch({ args: ["--enable-unsafe-swiftshader", "--ignore-gpu-blocklist"] });
const errors = [];
try {
  const page = await browser.newPage({ viewport: { width: 1100, height: 760 } });
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto(`http://127.0.0.1:${port}/?viewer=1&eval=1&docname=v3-gizmo-${Date.now()}`);
  await page.waitForFunction(() => window.__ve?.ready && window.__ve.evaluation?.state.worker === "idle", null, { timeout: 90000 });
  await page.evaluate(() => window.__ve.doc.sendText(
    "base = solid.box(size=[0.4, 0.2, 0.3])\noutput part = base", "V3 gizmo fixture"));
  await page.waitForFunction(() => window.__ve.doc.state().revision === 1 && window.__ve.evaluation.state.worker === "idle", null, { timeout: 60000 });
  const base = await page.evaluate(() => window.__ve.doc.state().document.nodes.find((node) => node.label === "base"));
  await page.click(`.dock .orow[data-id="${base.id}"]`);
  await page.waitForFunction(() => document.querySelector('#viewer-gizmo [data-axis="0"]'), null, { timeout: 10000 });
  await mkdir("target/reports", { recursive: true });
  await page.screenshot({ path: "target/reports/v3-translate-gizmo.png" });
  await page.check("#gizmo-bounds");
  assert.equal(await page.locator("#viewer-gizmo .gizmo-bounds").count(), 12, "the bounds overlay draws the twelve box edges");
  const boundsRevision = await page.evaluate(() => window.__ve.doc.state().revision);
  await page.uncheck("#gizmo-bounds");
  assert.equal(await page.evaluate(() => window.__ve.doc.state().revision), boundsRevision, "view overlays do not edit the document");
  await page.check("#gizmo-wire");
  assert(await page.locator("#viewer-gizmo .gizmo-wire").count() > 0, "wire overlay draws sampled evaluated edges");
  await page.check("#gizmo-normals");
  assert(await page.locator("#viewer-gizmo .gizmo-normal").count() > 0, "normal overlay draws sampled face normals");
  await page.uncheck("#gizmo-wire"); await page.uncheck("#gizmo-normals");
  assert.equal(await page.evaluate(() => window.__ve.doc.state().revision), boundsRevision, "mesh overlays leave the authored revision unchanged");
  const axis = await page.locator('#viewer-gizmo [data-axis="0"]').evaluate((line) => ({
    x1: Number(line.getAttribute("x1")), y1: Number(line.getAttribute("y1")),
    x2: Number(line.getAttribute("x2")), y2: Number(line.getAttribute("y2")),
    rect: document.querySelector("#ve").getBoundingClientRect().toJSON(),
  }));
  const start = { x: axis.rect.x + axis.x1 * 0.88 + axis.x2 * 0.12, y: axis.rect.y + axis.y1 * 0.88 + axis.y2 * 0.12 };
  const end = { x: axis.rect.x + axis.x1 * 0.62 + axis.x2 * 0.38, y: axis.rect.y + axis.y1 * 0.62 + axis.y2 * 0.38 };
  await page.mouse.move(start.x, start.y);
  await page.mouse.down();
  await page.mouse.move(end.x, end.y, { steps: 8 });
  await page.waitForFunction(() => /Move ·/.test(document.querySelector("#viewer-gizmo-status").textContent), null, { timeout: 10000 });
  assert(await page.locator("#viewer-gizmo .gizmo-preview").count() > 0, "drag preview moves the visible selection bounds without rebuilding geometry");
  assert.equal(await page.evaluate(() => window.__ve.doc.state().revision), 1, "preview does not mutate the document");
  await page.mouse.up();
  await page.waitForFunction(() => window.__ve.doc.state().revision === 2 && window.__ve.evaluation.state.worker === "idle", null, { timeout: 60000 });
  const moved = await page.evaluate(() => window.__ve.doc.state().document);
  const transform = moved.nodes.find((node) => node.op === "xform.move@1");
  assert(transform, "the pointer drag inserted an authored move node");
  assert(transform.params.by[0] > 0, `the X gizmo moved positively: ${transform.params.by}`);
  assert.equal(moved.outputs[0].from.node, transform.id, "the output follows the new transform");

  await page.evaluate(() => window.__ve.doc.undo());
  await page.waitForFunction(() => window.__ve.doc.state().revision === 3 && window.__ve.evaluation.state.worker === "idle", null, { timeout: 60000 });
  assert.equal((await page.evaluate(() => window.__ve.doc.state().document)).nodes.length, 1, "one undo removes the drag transaction");

  await page.selectOption("#gizmo-mode", "rotate");
  await page.waitForFunction(() => document.querySelector('#viewer-gizmo [data-ring-axis="2"]'), null, { timeout: 10000 });
  const ring = await page.locator('#viewer-gizmo [data-ring-axis="2"]').evaluate((polyline) => {
    const points = polyline.getAttribute("points").split(" ").map((pair) => pair.split(",").map(Number));
    const rect = document.querySelector("#ve").getBoundingClientRect().toJSON();
    for (let index = 0; index < points.length - 10; index++) {
      const a = points[index], b = points[index + 8];
      if ([a, b].every(([x, y]) => x > 330 && x < 700 && y > 70 && y < 680)) {
        return { a: { x: rect.x + a[0], y: rect.y + a[1] }, b: { x: rect.x + b[0], y: rect.y + b[1] } };
      }
    }
    return null;
  });
  assert(ring, "a rotation ring segment is reachable outside the panels");
  await page.mouse.move(ring.a.x, ring.a.y); await page.mouse.down();
  await page.mouse.move(ring.b.x, ring.b.y, { steps: 8 });
  await page.waitForFunction(() => /Rotate ·/.test(document.querySelector("#viewer-gizmo-status").textContent), null, { timeout: 10000 });
  await page.mouse.up();
  await page.waitForFunction(() => window.__ve.doc.state().revision === 4 && window.__ve.evaluation.state.worker === "idle", null, { timeout: 60000 });
  const rotated = await page.evaluate(() => window.__ve.doc.state().document);
  const rotateNode = rotated.nodes.find((node) => node.op === "xform.rotate@1");
  assert(rotateNode && Math.abs(rotateNode.params.angles[2]) > 0, "the ring authors a Z rotation");
  await page.evaluate(() => window.__ve.doc.undo());
  await page.waitForFunction(() => window.__ve.doc.state().revision === 5 && window.__ve.evaluation.state.worker === "idle", null, { timeout: 60000 });

  await page.selectOption("#gizmo-mode", "scale");
  await page.waitForFunction(() => document.querySelector('#viewer-gizmo [data-axis="0"]'), null, { timeout: 10000 });
  const scaleLine = await page.locator('#viewer-gizmo [data-axis="0"]').evaluate((line) => ({
    x1: Number(line.getAttribute("x1")), y1: Number(line.getAttribute("y1")),
    x2: Number(line.getAttribute("x2")), y2: Number(line.getAttribute("y2")),
    rect: document.querySelector("#ve").getBoundingClientRect().toJSON(),
  }));
  const scaleStart = { x: scaleLine.rect.x + scaleLine.x1 * 0.88 + scaleLine.x2 * 0.12,
    y: scaleLine.rect.y + scaleLine.y1 * 0.88 + scaleLine.y2 * 0.12 };
  const scaleEnd = { x: scaleLine.rect.x + scaleLine.x1 * 0.62 + scaleLine.x2 * 0.38,
    y: scaleLine.rect.y + scaleLine.y1 * 0.62 + scaleLine.y2 * 0.38 };
  await page.mouse.move(scaleStart.x, scaleStart.y); await page.mouse.down();
  await page.mouse.move(scaleEnd.x, scaleEnd.y, { steps: 8 });
  await page.waitForFunction(() => /Scale ·/.test(document.querySelector("#viewer-gizmo-status").textContent), null, { timeout: 10000 });
  await page.mouse.up();
  await page.waitForFunction(() => window.__ve.doc.state().revision === 6 && window.__ve.evaluation.state.worker === "idle", null, { timeout: 60000 });
  const scaled = await page.evaluate(() => window.__ve.doc.state().document);
  const scaleNode = scaled.nodes.find((node) => node.op === "xform.scale@1");
  assert(scaleNode && scaleNode.params.by[0] > 1, "the scale handle authors a positive X scale");
  await page.evaluate(() => window.__ve.doc.undo());
  await page.waitForFunction(() => window.__ve.doc.state().revision === 7 && window.__ve.evaluation.state.worker === "idle", null, { timeout: 60000 });

  const revision = await page.evaluate(() => window.__ve.doc.state().revision);
  const next = await page.locator('#viewer-gizmo [data-axis="0"]').evaluate((line) => ({
    x: Number(line.getAttribute("x1")) * 0.5 + Number(line.getAttribute("x2")) * 0.5,
    y: Number(line.getAttribute("y1")) * 0.5 + Number(line.getAttribute("y2")) * 0.5,
    rect: document.querySelector("#ve").getBoundingClientRect().toJSON(),
  }));
  await page.mouse.move(next.rect.x + next.x, next.rect.y + next.y);
  await page.mouse.down();
  await page.mouse.move(next.rect.x + next.x + 18, next.rect.y + next.y, { steps: 4 });
  await page.keyboard.press("Escape");
  await page.mouse.up();
  assert.equal(await page.evaluate(() => window.__ve.doc.state().revision), revision, "Escape abandons the uncommitted drag");

  await page.selectOption("#gizmo-mode", "translate");
  await page.check("#gizmo-bounds");

  const reviewViews = [
    ["front", [0, 0.55, 1.6]], ["back", [0, 0.55, -1.6]],
    ["left", [-1.6, 0.55, 0]], ["right", [1.6, 0.55, 0]],
    ["top", [0, 1.8, 0.001]], ["bottom", [0, -1.8, 0.001]],
  ];
  for (const [name, camera] of reviewViews) {
    const frame = await page.evaluate(() => window.__ve.read().frames);
    assert(await page.evaluate((position) => window.__ve.viewer.api.viewer_look_at(...position, 0, 0, 0), camera),
      `${name} review camera is valid`);
    await page.waitForFunction((previous) => window.__ve.read().frames >= previous + 3, frame, { timeout: 10000 });
    await page.screenshot({ path: `target/reports/v3-view-${name}.png` });
  }

  assert.deepEqual(errors, []);
  console.log("V3 gizmo pointer acceptance passed");
} finally {
  await browser.close();
  server.close();
}
