// Quality tiers must change what the headset actually renders, not just a number in the status. A
// runtime that honours viewport-scale requests is emulated on top of IWER (which has none): the
// engine must ask for the tier's scale, draw into the smaller viewports it is then given, and come
// back to full size, repeatedly, without breaking the picture. Whether a real Quest grants these
// requests is a hardware question; this proves the engine's side of the exchange.
import fs from "node:fs";
import path from "node:path";
import { chromium } from "playwright";
import { PNG } from "pngjs";
import { drawnPixels, driverAdvisory, here, serve } from "./support.mjs";

const iwerBundle = fs.readFileSync(path.join(here, "..", "node_modules", "iwer", "build", "iwer.min.js"), "utf8");
const failures = [];
const check = (ok, message) => { if (!ok) failures.push(message); };

const { server, port } = await serve();
const browser = await chromium.launch({ args: ["--enable-unsafe-swiftshader", "--ignore-gpu-blocklist"] });
try {
  const page = await browser.newPage({ viewport: { width: 640, height: 360 } });
  const logs = [];
  page.on("console", (m) => {
    if (["error", "warning"].includes(m.type()) && !driverAdvisory.test(m.text())) logs.push(`${m.type()}: ${m.text()}`);
  });
  page.on("pageerror", (e) => logs.push(`pageerror: ${e.message}`));
  await page.addInitScript({ content: iwerBundle });
  await page.addInitScript(() => {
    const device = new window.IWER.XRDevice(window.IWER.metaQuest3);
    device.installRuntime({ forceInstall: true });
    device.stereoEnabled = true;
    // A runtime that grants viewport scaling: a view remembers the scale asked of it and the layer
    // reports a viewport shrunk about the viewport's top-left corner... (origin is bottom-left in GL,
    // so shrink towards the origin and keep it inside the allocation).
    window.__requests = [];
    if (typeof XRView === "function" && !("requestViewportScale" in XRView.prototype)) {
      XRView.prototype.requestViewportScale = function (scale) { this.__scale = scale; window.__requests.push(scale); };
    }
    // IWER's frames and views are not instances of the browser's own classes, so patch them where
    // the page receives them: every frame handed to the frame callback gives its views the method.
    const realRequest = navigator.xr.requestSession.bind(navigator.xr);
    navigator.xr.requestSession = async (...args) => {
      const session = await realRequest(...args);
      const raf = session.requestAnimationFrame.bind(session);
      session.requestAnimationFrame = (callback) => raf((time, frame) => {
        if (!frame.__patched) {
          frame.__patched = true;
          const pose = frame.getViewerPose.bind(frame);
          frame.getViewerPose = (space) => {
            const result = pose(space);
            for (const view of result?.views ?? []) {
              view.requestViewportScale = (scale) => { view.__scale = scale; window.__requests.push(scale); };
            }
            return result;
          };
        }
        return callback(time, frame);
      });
      return session;
    };
    const real = XRWebGLLayer.prototype.getViewport;
    XRWebGLLayer.prototype.getViewport = function (view) {
      const v = real.call(this, view);
      const s = view.__scale ?? 1;
      return { x: v.x, y: v.y, width: Math.max(1, Math.round(v.width * s)), height: Math.max(1, Math.round(v.height * s)) };
    };
    window.__xr = device;
  });

  await page.goto(`http://127.0.0.1:${port}/?scene=xr&xr=auto&quality=high`);
  await page.waitForFunction(() => window.__ve?.ready || window.__ve?.error, null, { timeout: 90000 });
  check(!(await page.evaluate(() => window.__ve.error)), `start failed: ${await page.evaluate(() => window.__ve.error)}`);
  await page.waitForFunction(() => window.__ve.read().xr.state === "Presenting" && window.__ve.read().xr.frames >= 6, null, { timeout: 60000 });

  const xr = () => page.evaluate(() => window.__ve.read().xr);
  const settle = async (n = 8) => {
    const target = (await xr()).frames + n;
    await page.waitForFunction((t) => window.__ve.read().xr.frames >= t, target, { timeout: 30000 });
  };
  const eyeWidth = async () => (await xr()).last_frame.viewports[0][2];

  check((await xr()).viewport_scale.supported === true, "the engine did not detect viewport scaling");
  await settle();
  const full = await eyeWidth();
  check(full > 100 && (await xr()).viewport_scale.requested === 1, `high tier: eye width ${full}, requested ${(await xr()).viewport_scale.requested}`);
  const fullDrawn = drawnPixels(PNG.sync.read(await page.screenshot()), 0, 320);

  // Low asks for 0.8 of the allocation: the eyes must really shrink by that factor, and the
  // picture must still be there, drawn into a smaller area.
  await page.evaluate(() => window.__ve.setQuality("low"));
  await settle(12);
  const low = await eyeWidth();
  check(Math.abs(low / full - 0.8) < 0.02, `low tier eye width ${low} is not 0.8 of ${full}`);
  check(Math.abs((await xr()).viewport_scale.requested - 0.8) < 1e-6, `requested scale ${(await xr()).viewport_scale.requested}`);
  const lowDrawn = drawnPixels(PNG.sync.read(await page.screenshot()), 0, 320);
  check(lowDrawn > 1000, `the low tier drew nothing (${lowDrawn} pixels)`);
  check(lowDrawn < fullDrawn, `the low tier did not draw less area (${lowDrawn} vs ${fullDrawn})`);

  // Repeated toggling returns to exactly the same size each time, with no errors and no stalls.
  for (let i = 0; i < 4; i++) {
    await page.evaluate(() => window.__ve.setQuality("high"));
    await settle(10);
    check((await eyeWidth()) === full, `round ${i}: high tier eye width ${await eyeWidth()}, expected ${full}`);
    await page.evaluate(() => window.__ve.setQuality("low"));
    await settle(10);
    check((await eyeWidth()) === low, `round ${i}: low tier eye width ${await eyeWidth()}, expected ${low}`);
  }
  const state = await xr();
  check(state.state === "Presenting" && state.error === null, `session state after toggling: ${state.state} ${state.error}`);
  const sizes = state.last_frame.viewports;
  check(sizes.length === 2 && sizes[0][2] === sizes[1][2], `eyes differ in size: ${JSON.stringify(sizes)}`);
  check(state.last_frame.framebuffer[0] > sizes[0][2] * 2 - 1, "the allocation shrank: it should stay at the full layer size");
  check(logs.length === 0, `browser logged problems:\n  ${logs.join("\n  ")}`);
  console.log(JSON.stringify({ full, low, requests: await page.evaluate(() => window.__requests.length) }));
} finally {
  await browser.close();
  server.close();
}

if (failures.length > 0) {
  console.error(`xr-resolution test FAILED (${failures.length}):\n- ${failures.join("\n- ")}`);
  process.exit(1);
}
console.log("xr-resolution test passed");
