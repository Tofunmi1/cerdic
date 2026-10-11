// M3 acceptance: the canvas is cleared to the expected colour through the real wgpu/WebGL2 path,
// the device limits meet our budget, and nothing logs an error.
// Run via `cargo xtask web-test` (builds first) or `npm test` in engine/web after `cargo xtask web`.
import fs from "node:fs";
import http from "node:http";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { PNG } from "pngjs";

const dist = path.join(path.dirname(fileURLToPath(import.meta.url)), "..", "dist");
const types = { ".js": "text/javascript", ".wasm": "application/wasm", ".html": "text/html" };
const server = http
  .createServer((req, res) => {
    const file = path.join(dist, req.url === "/" ? "index.html" : req.url.split("?")[0]);
    try {
      res.writeHead(200, { "content-type": types[path.extname(file)] ?? "application/octet-stream" });
      res.end(fs.readFileSync(file));
    } catch {
      res.writeHead(404);
      res.end();
    }
  })
  .listen(0);
const port = server.address().port;

const EXPECTED = [26, 51, 102]; // sRGB of the clear colour set in ve-runtime
const TOLERANCE = 3;
const BUDGET = { max_texture_dimension_2d: 2048, max_bind_groups: 4, max_color_attachments: 4 };

const failures = [];
const check = (ok, message) => { if (!ok) failures.push(message); };

const browser = await chromium.launch({ args: ["--enable-unsafe-swiftshader", "--ignore-gpu-blocklist"] });
try {
  const page = await browser.newPage({ viewport: { width: 640, height: 360 } });
  const logs = [];
  // Chrome's GL driver reports performance advisories (for example while wgpu probes the adapter).
  // They describe the environment, not our code, so they are ignored; everything else is fatal.
  const driverAdvisory = /GL Driver Message \(OpenGL, Performance,/;
  page.on("console", (m) => {
    if (["error", "warning"].includes(m.type()) && !driverAdvisory.test(m.text())) logs.push(`${m.type()}: ${m.text()}`);
  });
  page.on("pageerror", (e) => logs.push(`pageerror: ${e.message}`));

  await page.goto(`http://127.0.0.1:${port}/`);
  await page.waitForFunction(() => window.__ve?.ready || window.__ve?.error, null, { timeout: 60000 });
  const error = await page.evaluate(() => window.__ve.error);
  check(!error, `start failed: ${error}`);

  // Let a few frames render, then read the composited pixels.
  await page.waitForFunction(() => window.__ve.read().frames >= 10, null, { timeout: 30000 });
  const status = await page.evaluate(() => window.__ve.read());
  const bootMs = await page.evaluate(() => window.__ve.bootMs);
  const png = PNG.sync.read(await page.screenshot());
  const at = (x, y) => { const i = (png.width * y + x) * 4; return [png.data[i], png.data[i + 1], png.data[i + 2]]; };
  const center = at(png.width >> 1, png.height >> 1);
  const corner = at(2, 2);

  for (const [name, pixel] of [["center", center], ["corner", corner]]) {
    const off = pixel.map((c, i) => Math.abs(c - EXPECTED[i]));
    check(Math.max(...off) <= TOLERANCE, `${name} pixel ${pixel} is not ${EXPECTED} (+/-${TOLERANCE})`);
  }
  check(String(status.backend).toLowerCase().includes("gl"), `unexpected backend ${status.backend}`);
  for (const [key, minimum] of Object.entries(BUDGET)) {
    check((status.limits?.[key] ?? 0) >= minimum, `limit ${key}=${status.limits?.[key]} is below ${minimum}`);
  }
  check(status.frames >= 10, `only ${status.frames} frames rendered`);
  check(logs.length === 0, `browser logged problems:\n  ${logs.join("\n  ")}`);

  // Resize: the drawing buffer must follow the CSS size.
  await page.setViewportSize({ width: 800, height: 450 });
  await page.waitForFunction(() => document.getElementById("ve").width === 800, null, { timeout: 10000 })
    .catch(() => failures.push("canvas did not follow a viewport resize to 800 wide"));

  const wasmBytes = fs.statSync(path.join(dist, "ve_runtime_bg.wasm")).size;
  console.log(JSON.stringify({ bootMs, wasmBytes, adapter: status.adapter, backend: status.backend, limits: status.limits, center, corner }, null, 2));
} finally {
  await browser.close();
  server.close();
}

if (failures.length > 0) {
  console.error(`\nFAILED (${failures.length}):\n- ${failures.join("\n- ")}`);
  process.exit(1);
}
console.log("\nM3 browser checks passed");
