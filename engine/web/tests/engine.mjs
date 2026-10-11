// Browser acceptance test: boot, device limits, a golden image of the kernel-built demo scene,
// background colour, resize, and memory stability over many frames.
// Run via `cargo xtask web-test [dev]` (builds first) or `npm test` here after `cargo xtask web`.
// Set UPDATE_GOLDEN=1 to rewrite the golden image after an intentional rendering change.
import fs from "node:fs";
import http from "node:http";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { PNG } from "pngjs";

const here = path.dirname(fileURLToPath(import.meta.url));
const dist = path.join(here, "..", "dist");
const goldenPath = path.join(here, "golden", "scene-640x360.png");
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
  });
// Bind IPv4 loopback explicitly: the default wildcard bind made Chrome's connection stall at random.
await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
const port = server.address().port;

const CLEAR = [26, 51, 102]; // sRGB of the clear colour set in ve-runtime
const COLOUR_TOLERANCE = 3;
const BUDGET = { max_texture_dimension_2d: 2048, max_bind_groups: 4, max_color_attachments: 4 };
const GOLDEN = { channelTolerance: 10, maxBadFraction: 0.002 };
const MEMORY_GROWTH_LIMIT = 2 * 1024 * 1024; // bytes of wasm memory growth allowed across 600 frames

const failures = [];
const check = (ok, message) => { if (!ok) failures.push(message); };
const pixelAt = (png, x, y) => { const i = (png.width * y + x) * 4; return [png.data[i], png.data[i + 1], png.data[i + 2]]; };

function compareGolden(actual, expected) {
  if (actual.width !== expected.width || actual.height !== expected.height) {
    return { ok: false, message: `size ${actual.width}x${actual.height} != golden ${expected.width}x${expected.height}` };
  }
  let bad = 0;
  for (let i = 0; i < actual.data.length; i += 4) {
    const worst = Math.max(Math.abs(actual.data[i] - expected.data[i]), Math.abs(actual.data[i + 1] - expected.data[i + 1]), Math.abs(actual.data[i + 2] - expected.data[i + 2]));
    if (worst > GOLDEN.channelTolerance) bad++;
  }
  const fraction = bad / (actual.width * actual.height);
  return { ok: fraction <= GOLDEN.maxBadFraction, message: `${(fraction * 100).toFixed(3)}% of pixels differ by more than ${GOLDEN.channelTolerance}` };
}

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
  await page.waitForFunction(() => window.__ve?.ready || window.__ve?.error, null, { timeout: 90000 });
  check(!(await page.evaluate(() => window.__ve.error)), `start failed: ${await page.evaluate(() => window.__ve.error)}`);

  await page.waitForFunction(() => window.__ve.read().frames >= 30, null, { timeout: 60000 });
  const status = await page.evaluate(() => window.__ve.read());
  const bootMs = await page.evaluate(() => window.__ve.bootMs);
  const screenshot = PNG.sync.read(await page.screenshot());

  // Background and clear colour.
  const corner = pixelAt(screenshot, 2, 2);
  check(Math.max(...corner.map((c, i) => Math.abs(c - CLEAR[i]))) <= COLOUR_TOLERANCE, `corner ${corner} is not the clear colour ${CLEAR}`);
  const center = pixelAt(screenshot, screenshot.width >> 1, screenshot.height >> 1);
  check(Math.max(...center.map((c, i) => Math.abs(c - CLEAR[i]))) > 20, `centre ${center} looks like background: nothing was drawn`);

  // Golden image of the demo scene.
  if (process.env.UPDATE_GOLDEN) {
    fs.mkdirSync(path.dirname(goldenPath), { recursive: true });
    fs.writeFileSync(goldenPath, PNG.sync.write(screenshot));
    console.log(`golden image written to ${goldenPath}`);
  } else if (!fs.existsSync(goldenPath)) {
    failures.push(`no golden image at ${goldenPath}; run with UPDATE_GOLDEN=1 once and review it`);
  } else {
    const result = compareGolden(screenshot, PNG.sync.read(fs.readFileSync(goldenPath)));
    check(result.ok, `golden image mismatch: ${result.message}`);
  }

  // Device and platform.
  check(String(status.backend).toLowerCase().includes("gl"), `unexpected backend ${status.backend}`);
  for (const [key, minimum] of Object.entries(BUDGET)) {
    check((status.limits?.[key] ?? 0) >= minimum, `limit ${key}=${status.limits?.[key]} is below ${minimum}`);
  }
  check((status.errors ?? []).length === 0, `engine reported errors: ${JSON.stringify(status.errors)}`);

  // Memory stays flat while rendering.
  await page.waitForFunction(() => window.__ve.read().frames >= 120, null, { timeout: 60000 });
  const memoryBefore = await page.evaluate(() => window.__ve.wasmBytes());
  const framesBefore = await page.evaluate(() => window.__ve.read().frames);
  await page.waitForFunction((target) => window.__ve.read().frames >= target, framesBefore + 600, { timeout: 120000 });
  const memoryAfter = await page.evaluate(() => window.__ve.wasmBytes());
  check(memoryAfter - memoryBefore <= MEMORY_GROWTH_LIMIT, `wasm memory grew ${memoryAfter - memoryBefore} bytes over 600 frames`);

  // Resize: the drawing buffer must follow the CSS size.
  await page.setViewportSize({ width: 800, height: 450 });
  await page.waitForFunction(() => document.getElementById("ve").width === 800, null, { timeout: 10000 })
    .catch(() => failures.push("canvas did not follow a viewport resize to 800 wide"));

  check(logs.length === 0, `browser logged problems:\n  ${logs.join("\n  ")}`);

  const wasmBytes = fs.statSync(path.join(dist, "ve_runtime_bg.wasm")).size;
  console.log(JSON.stringify({ bootMs, wasmBytes, adapter: status.adapter, backend: status.backend, limits: status.limits, memoryGrowthBytes: memoryAfter - memoryBefore, wasmMemoryBytes: memoryAfter }, null, 2));
} finally {
  await browser.close();
  server.close();
}

if (failures.length > 0) {
  console.error(`\nFAILED (${failures.length}):\n- ${failures.join("\n- ")}`);
  process.exit(1);
}
console.log("\nbrowser checks passed");
