// Quality tiers: every tier boots with its settings and draws the scene, and switching tiers at
// runtime many times neither breaks rendering nor leaks (wasm memory stays flat, the picture returns
// to the golden image when we come back to the starting tier).
import fs from "node:fs";
import path from "node:path";
import { chromium } from "playwright";
import { PNG } from "pngjs";
import { differingFraction, drawnPixels, driverAdvisory, GOLDEN, here, serve } from "./support.mjs";

const TIERS = {
  low: { msaa: 1, shadow_resolution: 0, max_point_lights: 2 },
  medium: { msaa: 2, shadow_resolution: 1024, max_point_lights: 4 },
  high: { msaa: 4, shadow_resolution: 2048, max_point_lights: 8 },
};
const SWITCHES = 24;
const MEMORY_GROWTH_LIMIT = 4 * 1024 * 1024;
const golden = PNG.sync.read(fs.readFileSync(path.join(here, "golden", "kernel-640x360.png")));
const shots = {};
const failures = [];
const check = (ok, message) => { if (!ok) failures.push(message); };

const { server, port } = await serve();
const browser = await chromium.launch({ args: ["--enable-unsafe-swiftshader", "--ignore-gpu-blocklist"] });
try {
  for (const [tier, expected] of Object.entries(TIERS)) {
    const page = await browser.newPage({ viewport: { width: 640, height: 360 } });
    const logs = [];
    page.on("console", (m) => {
      if (["error", "warning"].includes(m.type()) && !driverAdvisory.test(m.text())) logs.push(`${m.type()}: ${m.text()}`);
    });
    page.on("pageerror", (e) => logs.push(`pageerror: ${e.message}`));
    await page.goto(`http://127.0.0.1:${port}/?scene=kernel&quality=${tier}`);
    await page.waitForFunction(() => window.__ve?.ready || window.__ve?.error, null, { timeout: 90000 });
    const status = await page.evaluate(() => window.__ve.read());
    check(status.quality.tier === tier, `[${tier}] booted on tier ${status.quality.tier}`);
    for (const [key, value] of Object.entries(expected)) {
      check(status.quality[key] === value, `[${tier}] ${key} is ${status.quality[key]}, expected ${value}`);
    }
    await page.waitForTimeout(400);
    const shot = PNG.sync.read(await page.screenshot());
    check(drawnPixels(shot, 0, shot.width) > 5000, `[${tier}] the scene is not drawn`);
    // Every tier is the same picture to within anti-aliasing and shadow differences.
    check(differingFraction(shot, golden, 40) < 0.03, `[${tier}] looks different from the kernel golden`);
    check(logs.length === 0, `[${tier}] browser logged problems:\n  ${logs.join("\n  ")}`);
    shots[tier] = shot;
    await page.close();
  }
  // The tiers must really change the picture: without multisampling, silhouette edges differ.
  const edges = differingFraction(shots.low, shots.high, 8);
  check(edges > 0.0003, `low and high render the same (${(edges * 100).toFixed(4)}% of pixels differ): MSAA is not applied`);

  // Runtime switching: cycle through the tiers and come back to the start.
  const page = await browser.newPage({ viewport: { width: 640, height: 360 } });
  const logs = [];
  page.on("console", (m) => {
    if (["error", "warning"].includes(m.type()) && !driverAdvisory.test(m.text())) logs.push(`${m.type()}: ${m.text()}`);
  });
  page.on("pageerror", (e) => logs.push(`pageerror: ${e.message}`));
  await page.goto(`http://127.0.0.1:${port}/?scene=kernel`);
  await page.waitForFunction(() => window.__ve?.ready || window.__ve?.error, null, { timeout: 90000 });
  const order = Object.keys(TIERS);
  const settle = async (tier) => {
    await page.evaluate((t) => window.__ve.setQuality(t), tier);
    await page.waitForFunction((t) => window.__ve.read().quality.tier === t, tier, { timeout: 20000 });
    const frames = await page.evaluate(() => window.__ve.read().frames);
    await page.waitForFunction((n) => window.__ve.read().frames >= n + 3, frames, { timeout: 20000 });
  };
  await settle("medium");
  await settle("high");
  const memoryBefore = await page.evaluate(() => window.__ve.wasmBytes());
  for (let i = 0; i < SWITCHES; i++) await settle(order[i % order.length]);
  await settle("high");
  const memoryAfter = await page.evaluate(() => window.__ve.wasmBytes());
  check(memoryAfter - memoryBefore <= MEMORY_GROWTH_LIMIT, `wasm memory grew ${(memoryAfter - memoryBefore) / 1024} KiB over ${SWITCHES} tier switches`);
  const status = await page.evaluate(() => window.__ve.read());
  check(status.errors.length === 0, `engine errors: ${status.errors.join("; ")}`);
  const finalShot = PNG.sync.read(await page.screenshot());
  const fraction = differingFraction(finalShot, golden);
  check(fraction <= GOLDEN.maxBadFraction * 5, `after switching back to high the picture differs from the golden image in ${(fraction * 100).toFixed(3)}% of pixels`);
  check(logs.length === 0, `browser logged problems while switching:\n  ${logs.join("\n  ")}`);
  console.log(JSON.stringify({ memoryGrowthKiB: (memoryAfter - memoryBefore) / 1024, switches: SWITCHES, finalDiffersFromGolden: fraction }));
} finally {
  await browser.close();
  server.close();
}

if (failures.length > 0) {
  console.error(`\nFAILED (${failures.length}):\n- ${failures.join("\n- ")}`);
  process.exit(1);
}
console.log("\nquality checks passed");
