// V3-06/11: parameter gizmos edit the operator values the inspector reads.
import assert from "node:assert/strict";
import { chromium } from "playwright";
import { serve } from "./support.mjs";

const { server, port } = await serve();
const browser = await chromium.launch({ args: ["--enable-unsafe-swiftshader", "--ignore-gpu-blocklist"] });
try {
  const page = await browser.newPage({ viewport: { width: 1100, height: 760 } });
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto(`http://127.0.0.1:${port}/?viewer=1&eval=1&docname=v3-parameters-${Date.now()}`);
  await page.waitForFunction(() => window.__ve?.ready && window.__ve.evaluation?.state.worker === "idle", null, { timeout: 90000 });
  // Parameter hit targets are tested at the editor's wide interaction FOV; the product viewer's
  // 45-degree default is covered by the viewer acceptance test.
  await page.evaluate(() => window.__ve.viewer.api.viewer_configure(true, 90, false, 2));
  await page.evaluate(() => window.__ve.doc.sendText(`
    source = solid.box(size=[0.2,0.2,0.2])
    copies = xform.array(source, count_x=3, spacing=[0.5,0,0])
    socket = frame.add(copies, name="mount", position=[0,0,0], rotation=[0,0,0])
    section = profile.polygon(points=[[0.2,0],[0.35,0],[0.35,0.2],[0.2,0.2]])
    lathe = solid.revolve(section, angle=1.5, segments=24)
    output array_result = copies
    output socket_result = socket
    output lathe_result = lathe
  `, "V3 parameter fixture"));
  await page.waitForFunction(() => window.__ve.doc.state().revision === 1 && window.__ve.evaluation.state.worker === "idle", null, { timeout: 90000 });

  const node = async (op) => page.evaluate((family) => window.__ve.doc.state().document.nodes.find((item) => item.op.startsWith(family)), op);
  const select = async (item) => {
    await page.click(`.dock .orow[data-id="${item.id}"]`);
    await page.waitForFunction((id) => JSON.parse(window.__ve.viewer.api.selection_get()).node === id, item.id, { timeout: 10000 });
  };
  const dragAxis = async (selector, label) => {
    const revision = await page.evaluate(() => window.__ve.doc.state().revision);
    const axis = await page.locator(selector).evaluate((line) => ({
      x1: Number(line.getAttribute("x1")), y1: Number(line.getAttribute("y1")),
      x2: Number(line.getAttribute("x2")), y2: Number(line.getAttribute("y2")),
      rect: document.querySelector("#ve").getBoundingClientRect().toJSON(),
    }));
    const start = { x: axis.rect.x + axis.x1 * 0.88 + axis.x2 * 0.12, y: axis.rect.y + axis.y1 * 0.88 + axis.y2 * 0.12 };
    const end = { x: axis.rect.x + axis.x1 * 0.62 + axis.x2 * 0.38, y: axis.rect.y + axis.y1 * 0.62 + axis.y2 * 0.38 };
    await page.mouse.move(start.x, start.y); await page.mouse.down();
    await page.mouse.move(end.x, end.y, { steps: 8 });
    await page.waitForFunction((pattern) => pattern.test(document.querySelector("#viewer-gizmo-status").textContent), new RegExp(label), { timeout: 10000 });
    await page.mouse.up();
    await page.waitForFunction((previous) => window.__ve.doc.state().revision === previous + 1
      && window.__ve.evaluation.state.worker === "idle", revision, { timeout: 90000 });
  };
  const dragRing = async (selector) => {
    const revision = await page.evaluate(() => window.__ve.doc.state().revision);
    const points = await page.locator(selector).evaluate((ring) => {
      const vertices = ring.getAttribute("points").split(" ").map((pair) => pair.split(",").map(Number));
      const rect = document.querySelector("#ve").getBoundingClientRect().toJSON();
      for (let i = 0; i < vertices.length - 10; i++) {
        const a = vertices[i], b = vertices[i + 8];
        if ([a,b].every(([x,y]) => x > 330 && x < 700 && y > 70 && y < 680)) {
          return { a: { x: rect.x + a[0], y: rect.y + a[1] }, b: { x: rect.x + b[0], y: rect.y + b[1] } };
        }
      }
      return null;
    });
    assert(points, `visible ring segment for ${selector}`);
    await page.mouse.move(points.a.x, points.a.y); await page.mouse.down();
    await page.mouse.move(points.b.x, points.b.y, { steps: 8 });
    await page.waitForFunction(() => /Rotate ·/.test(document.querySelector("#viewer-gizmo-status").textContent), null, { timeout: 10000 });
    await page.mouse.up();
    await page.waitForFunction((previous) => window.__ve.doc.state().revision === previous + 1
      && window.__ve.evaluation.state.worker === "idle", revision, { timeout: 90000 });
  };

  const array = await node("xform.array");
  await select(array);
  await page.waitForFunction(() => document.querySelector('[data-parameter="spacing"][data-axis="0"]'), null, { timeout: 10000 });
  await dragAxis('[data-parameter="spacing"][data-axis="0"]', "Move ·");
  assert((await node("xform.array")).params.spacing[0] > 0.5, "array spacing handle updates inspector parameter");

  const frame = await node("frame.add");
  await select(frame);
  await page.waitForFunction(() => document.querySelector('[data-parameter="position"][data-axis="0"]'), null, { timeout: 10000 });
  await dragAxis('[data-parameter="position"][data-axis="0"]', "Move ·");
  assert((await node("frame.add")).params.position[0] > 0, "socket frame position handle updates the authored frame");
  await page.selectOption("#gizmo-mode", "rotate");
  await page.waitForFunction(() => document.querySelector('[data-parameter="rotation"][data-ring-axis="2"]'), null, { timeout: 10000 });
  await dragRing('[data-parameter="rotation"][data-ring-axis="2"]');
  assert(Math.abs((await node("frame.add")).params.rotation[2]) > 0, "socket rotation handle updates the authored frame");

  const lathe = await node("solid.revolve");
  await select(lathe);
  await page.waitForFunction(() => document.querySelector('[data-parameter="revolve.angle"][data-ring-axis="1"]'), null, { timeout: 10000 });
  const angle = lathe.params.angle;
  await dragRing('[data-parameter="revolve.angle"][data-ring-axis="1"]');
  assert.notEqual((await node("solid.revolve")).params.angle, angle, "revolve sweep handle updates the authored angle");
  assert.deepEqual(errors, []);
  console.log("V3 parameter handle acceptance passed");
} finally {
  await browser.close();
  server.close();
}
