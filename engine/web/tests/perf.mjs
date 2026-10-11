// Benchmark scenes and the frame-time governor. Headless Chromium renders in software, so absolute
// frame times here mean nothing; what is checked is that the scenes are the size the plan asks for,
// the statistics are coherent, and the governor reacts to load in the right direction. The window's
// refresh target is lowered (?hz=5) so a slow renderer still has "frames over budget" to react to.
import { chromium } from "playwright";
import { driverAdvisory, serve } from "./support.mjs";

const SPECS = {
  bench1: { instances: 45, triangles: 50_000 },
  bench2: { instances: 155, triangles: 200_000 },
  bench3: { instances: 305, triangles: 400_000 },
};
const failures = [];
const check = (ok, message) => { if (!ok) failures.push(message); };

const { server, port } = await serve();
const browser = await chromium.launch({ args: ["--enable-unsafe-swiftshader", "--ignore-gpu-blocklist"] });

async function open(query) {
  const page = await browser.newPage({ viewport: { width: 480, height: 270 } });
  const logs = [];
  page.on("console", (m) => {
    if (["error", "warning"].includes(m.type()) && !driverAdvisory.test(m.text())) logs.push(`${m.type()}: ${m.text()}`);
  });
  page.on("pageerror", (e) => logs.push(`pageerror: ${e.message}`));
  await page.goto(`http://127.0.0.1:${port}/?${query}`);
  await page.waitForFunction(() => window.__ve?.ready || window.__ve?.error, null, { timeout: 120000 });
  return { page, logs };
}

try {
  // Scene sizes.
  for (const [scene, spec] of Object.entries(SPECS)) {
    const { page, logs } = await open(`scene=${scene}`);
    await page.waitForFunction(() => window.__ve.read().perf.total_frames >= 3, null, { timeout: 120000 });
    const status = await page.evaluate(() => window.__ve.read());
    check(status.scene?.instances === spec.instances, `[${scene}] ${status.scene?.instances} instances, expected ${spec.instances}`);
    check(status.scene?.mesh_assets === spec.instances, `[${scene}] ${status.scene?.mesh_assets} mesh assets, expected one per instance`);
    const ratio = status.scene.triangles / spec.triangles;
    check(ratio > 0.9 && ratio < 1.1, `[${scene}] ${status.scene.triangles} triangles, expected about ${spec.triangles}`);
    // The renderer's own counts must reconcile with the scene: a mono view draws every instance once
    // (nothing is batched here) and rasterises exactly the scene's triangles; the shadow pass repeats
    // them when shadows are on.
    await page.waitForFunction(() => window.__ve.read().render.main.draws > 0, null, { timeout: 60000 });
    const counted = (await page.evaluate(() => window.__ve.read())).render;
    check(counted.main.draws === spec.instances, `[${scene}] main pass submitted ${counted.main.draws} draws, expected ${spec.instances}`);
    check(counted.main.triangles === status.scene.triangles, `[${scene}] main pass drew ${counted.main.triangles} triangles, scene has ${status.scene.triangles}`);
    const shadowsOn = status.quality.shadow_resolution > 0;
    check(counted.shadow.draws === (shadowsOn ? spec.instances : 0), `[${scene}] shadow pass submitted ${counted.shadow.draws} draws (shadows ${shadowsOn ? "on" : "off"})`);
    check(status.errors.length === 0, `[${scene}] engine errors: ${status.errors.join("; ")}`);
    check(logs.length === 0, `[${scene}] browser logged problems:\n  ${logs.join("\n  ")}`);
    console.log(JSON.stringify({ scene, ...status.scene }));
    await page.close();
  }

  // Statistics are coherent and respect the configured refresh target.
  {
    const { page } = await open("scene=kernel&hz=90");
    await page.waitForFunction(() => window.__ve.read().perf.total_frames >= 40, null, { timeout: 60000 });
    const perf = (await page.evaluate(() => window.__ve.read())).perf;
    check(perf.target_hz === 90, `target_hz is ${perf.target_hz}, expected 90`);
    check(perf.p50_ms > 0 && perf.p95_ms >= perf.p50_ms && perf.max_ms >= perf.p95_ms, `incoherent percentiles ${JSON.stringify(perf)}`);
    check(perf.fps > 0 && perf.window_frames > 0, "no fps reported");
    check(perf.governor.enabled === false && perf.governor.changes.length === 0, "the governor acted while disabled");
    await page.close();
  }

  // Overload: the stress scene against a 5 Hz budget must walk the tier down, high to medium to low
  // (the budget is artificially long so a slow software renderer still misses it).
  {
    const { page } = await open("scene=bench3&quality=high&governor=1&hz=5");
    await page.waitForFunction(() => window.__ve.read().quality.tier === "low", null, { timeout: 150000 })
      .catch(() => failures.push("the governor never reached the low tier under overload"));
    const status = await page.evaluate(() => window.__ve.read());
    const tiers = status.perf.governor.changes.map((c) => c.tier);
    // It steps down at once, and may spend a trial step back up when a level looks affordable (a
    // paced loop cannot show spare capacity), but it must end on low without thrashing.
    check(tiers[0] === "medium" && tiers.at(-1) === "low" && tiers.length <= 6, `tier changes under overload were ${JSON.stringify(tiers)}`);
    check(status.perf.governor.enabled === true, "governor not enabled");
    console.log(JSON.stringify({ overload: tiers, p95: status.perf.p95_ms }));
    await page.close();
  }

  // Headroom: a light scene starting on low climbs back up, one step at a time.
  {
    const { page } = await open("scene=kernel&quality=low&governor=1&hz=5");
    await page.waitForFunction(() => window.__ve.read().quality.tier === "high", null, { timeout: 150000 })
      .catch(() => failures.push("the governor never climbed to the high tier with plenty of headroom"));
    const status = await page.evaluate(() => window.__ve.read());
    const tiers = status.perf.governor.changes.map((c) => c.tier);
    check(tiers.join(",") === "medium,high", `tier changes with headroom were ${JSON.stringify(tiers)}`);
    console.log(JSON.stringify({ headroom: tiers }));
    await page.close();
  }
} finally {
  await browser.close();
  server.close();
}

if (failures.length > 0) {
  console.error(`\nFAILED (${failures.length}):\n- ${failures.join("\n- ")}`);
  process.exit(1);
}
console.log("\nperf checks passed");
