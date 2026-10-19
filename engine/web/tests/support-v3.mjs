// Helpers shared by the V3 viewport-editing browser tests: open the viewer on a text document, wait for
// the worker, select a node through the real outliner, and find a gizmo handle's screen position.
import { chromium } from "playwright";
import { serve } from "./support.mjs";

// One server and one browser per test file; every viewer gets its own context. (Launching a second browser
// right after closing the first failed with a dead DevTools pipe, so the tests do not do that.)
let shared = null;
async function environment() {
  if (!shared) {
    const { server, port } = await serve();
    const browser = await chromium.launch({ args: ["--enable-unsafe-swiftshader", "--ignore-gpu-blocklist"] });
    shared = { server, port, browser };
  }
  return shared;
}

/** Closes the shared browser and server; call once, when the test file is done. */
export async function shutdown() {
  if (!shared) return;
  const { server, browser } = shared;
  shared = null;
  await browser.close();
  server.close();
}

export async function openViewer(text, label) {
  const { port, browser } = await environment();
  const context = await browser.newContext({ viewport: { width: 1100, height: 760 } });
  const page = await context.newPage();
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto(`http://127.0.0.1:${port}/?viewer=1&eval=1&docname=${label}-${Date.now()}`);
  await page.waitForFunction(() => window.__ve?.ready && window.__ve.evaluation?.state.worker === "idle", null, { timeout: 90000 });
  // Viewport editing/picking checks use the editor's wide interaction view; the product viewer's
  // 45-degree default is tested independently in viewer.mjs.
  await page.evaluate(() => window.__ve.viewer.api.viewer_configure(true, 90, false, 2));
  await page.evaluate(([source, name]) => window.__ve.doc.sendText(source, name), [text, label]);
  await settle(page, 1);
  return { page, errors, close: () => context.close() };
}

/** Waits until the document is at least `revision`, nothing is staged and the worker is idle. */
export const settle = (page, revision) => page.waitForFunction((wanted) => {
  const state = window.__ve.doc.state();
  return state.revision >= wanted && !state.pending_document && window.__ve.evaluation.state.worker === "idle";
}, revision, { timeout: 90000 });

export const revisionOf = (page) => page.evaluate(() => window.__ve.doc.state().revision);
export const documentOf = (page) => page.evaluate(() => window.__ve.doc.state().document);
export const nodeId = (page, label) => page.evaluate((name) => window.__ve.doc.state().document.nodes.find((node) => node.label === name).id, label);
export const status = (page) => page.evaluate(() => document.querySelector("#viewer-gizmo-status").textContent);

/** Selects through the real outliner row and waits until the engine agrees. */
export async function select(page, id) {
  await page.click(`.dock .orow[data-id="${id}"]`);
  await page.waitForFunction((wanted) => JSON.parse(window.__ve.viewer.api.selection_get()).node === wanted, id, { timeout: 60000 });
}

/** Points the camera at `target` from `position` and waits for a few frames to draw with it. */
export async function lookAt(page, position, target) {
  const frame = await page.evaluate(() => window.__ve.read().frames);
  await page.evaluate(([from, to]) => window.__ve.viewer.api.viewer_look_at(...from, ...to), [position, target]);
  await page.waitForFunction((previous) => window.__ve.read().frames >= previous + 3, frame, { timeout: 15000 });
}

/** Where a world point is on screen, in page coordinates. */
export const screenOf = (page, world) => page.evaluate((point) => {
  const rect = document.querySelector("#ve").getBoundingClientRect();
  const projected = window.__ve.viewer.api.viewer_project(...point, rect.width, rect.height);
  const at = projected ? JSON.parse(projected) : null;
  return at ? { x: rect.x + at.x, y: rect.y + at.y } : null;
}, world);

/** A real click on the viewport. The press is held across rendered frames: on a software renderer a
 * press and release inside one frame is never seen as a click. */
export async function viewportClick(page, point) {
  await page.mouse.move(point.x, point.y);
  const frame = await page.evaluate(() => window.__ve.read().frames);
  await page.mouse.down();
  await page.waitForFunction((previous) => window.__ve.read().frames >= previous + 3, frame, { timeout: 15000 });
  await page.mouse.up();
  await page.evaluate(() => document.exitPointerLock?.());
}

/** Screen positions along a translate handle (or a rotation ring segment): where a real drag starts and ends. */
export async function axisDrag(page, selector = '#viewer-gizmo [data-axis="0"]') {
  await page.waitForSelector(selector, { state: "attached", timeout: 60000 });
  const axis = await page.locator(selector).first().evaluate((line) => ({
    x1: Number(line.getAttribute("x1")), y1: Number(line.getAttribute("y1")),
    x2: Number(line.getAttribute("x2")), y2: Number(line.getAttribute("y2")),
    rect: document.querySelector("#ve").getBoundingClientRect().toJSON(),
  }));
  const at = (t) => ({ x: axis.rect.x + axis.x1 * (1 - t) + axis.x2 * t, y: axis.rect.y + axis.y1 * (1 - t) + axis.y2 * t });
  return { start: at(0.12), middle: at(0.25), end: at(0.38) };
}

/** Presses on a handle with the real mouse and moves partway, until the drag says it is under way. */
export async function beginDrag(page, label = "Move ·", selector) {
  const points = await axisDrag(page, selector);
  await page.mouse.move(points.start.x, points.start.y);
  await page.mouse.down();
  await page.mouse.move(points.middle.x, points.middle.y, { steps: 6 });
  await page.waitForFunction((pattern) => new RegExp(pattern).test(document.querySelector("#viewer-gizmo-status").textContent),
    label, { timeout: 60000 });
  return points;
}
