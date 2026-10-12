// Resizing a rendering page must never stop the engine. Bevy only recomputes a camera's target size on
// a window-resized message; without one the render targets keep the old size, the surface takes the
// new one, validation fails, and Bevy quits the app. Every size below must leave frames advancing,
// the drawing buffer matching the canvas, the picture drawn and the console free of render errors.
import { chromium } from "playwright";
import { PNG } from "pngjs";
import { drawnPixels, driverAdvisory, serve } from "./support.mjs";

const failures = [];
const check = (ok, message) => { if (!ok) failures.push(message); };

const { server, port } = await serve();
const browser = await chromium.launch({ args: ["--enable-unsafe-swiftshader", "--ignore-gpu-blocklist"] });
try {
  const page = await browser.newPage({ viewport: { width: 640, height: 360 } });
  const logs = [];
  page.on("console", (m) => {
    const text = m.text();
    if (/Quitting the application|Caught rendering error|differing sizes/.test(text)) logs.push(`engine: ${text.replace(/\s+/g, " ").slice(0, 200)}`);
    else if (["error", "warning"].includes(m.type()) && !driverAdvisory.test(text)) logs.push(`${m.type()}: ${text}`);
  });
  page.on("pageerror", (e) => logs.push(`pageerror: ${e.message}`));

  await page.goto(`http://127.0.0.1:${port}/?scene=xr`);
  await page.waitForFunction(() => window.__ve?.ready || window.__ve?.error, null, { timeout: 90000 });
  const frames = () => page.evaluate(() => window.__ve.read().frames);
  const advances = async (label) => {
    const start = await frames();
    try {
      await page.waitForFunction((n) => window.__ve.read().frames >= n + 5, start, { timeout: 20000 });
      return true;
    } catch {
      check(false, `${label}: frames stopped at ${start}`);
      return false;
    }
  };
  check(await advances("boot"), "the engine does not run at boot");

  // Larger, smaller, much smaller, a sliver, and back: each must keep the engine running.
  for (const [w, h] of [[800, 450], [500, 300], [1000, 600], [200, 120], [64, 48], [640, 360]]) {
    await page.setViewportSize({ width: w, height: h });
    const label = `after resizing to ${w}x${h}`;
    if (!(await advances(label))) break;
    const size = await page.evaluate(() => {
      const c = document.getElementById("ve");
      return { client: [c.clientWidth, c.clientHeight], buffer: [c.width, c.height] };
    });
    check(size.client[0] === size.buffer[0] && size.client[1] === size.buffer[1], `${label}: buffer ${size.buffer} does not match canvas ${size.client}`);
  }

  // After all that, the picture is still drawn.
  const shot = PNG.sync.read(await page.screenshot());
  check(drawnPixels(shot, 0, shot.width) > 2000, "nothing is drawn after the resizes");
  check(logs.length === 0, `render or browser problems:\n  ${logs.join("\n  ")}`);
} finally {
  await browser.close();
  server.close();
}

if (failures.length > 0) {
  console.error(`resize test FAILED (${failures.length}):\n- ${failures.join("\n- ")}`);
  process.exit(1);
}
console.log("resize test passed");
