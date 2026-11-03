// Live check that particles collide with the scene's physics bodies (plan W4/W5): chips fall onto a static crate from above. With only the ground plane
// at y = 0 they would drop through the crate to the floor; with physics running they come to rest on its top (y = 1).
//   bun tools/fx-physics-live-test.mjs
// Needs the static server on 127.0.0.1:8801 serving web/dist-bevy-gpu with a built viewer.
import assert from "node:assert/strict";
import { chromium } from "../web/node_modules/playwright/index.mjs";

const browser = await chromium.launch({ channel: "chromium", args: ["--enable-unsafe-webgpu", "--enable-features=Metal,WebGPU", "--ignore-gpu-blocklist"] });
const page = await browser.newPage({ viewport: { width: 640, height: 400 } });
const problems = [];
page.on("pageerror", (e) => problems.push(String(e).slice(0, 300)));
page.on("console", (m) => { if (m.type() === "error" && !/404/.test(m.text())) problems.push(m.text().slice(0, 300)); });
await page.goto("http://localhost:8801/?viewer=1&eval=1&scene=stage&quality=low&world=worlds/fx_debris.txt");
await page.waitForFunction(() => globalThis.__ve?.evaluation?.parts?.length >= 4, null, { timeout: 120000 });
await page.evaluate(async () => { globalThis.__runtime = await import("./ve_runtime.js"); });
await page.waitForFunction(() => JSON.parse(globalThis.__runtime.eval_projection_status()).committed?.quality === "final", null, { timeout: 120000 });
await page.waitForTimeout(1000);
const call = (method, params = {}) => page.evaluate(async ([method, params]) => {
  const id = globalThis.__runtime.world_call(method, JSON.stringify(params));
  for (let i = 0; i < 200; i++) {
    const text = globalThis.__runtime.world_take(id);
    if (text) return JSON.parse(text);
    await new Promise((r) => setTimeout(r, 25));
  }
  return { error: "no answer" };
}, [method, params]);
const chips = async () => {
  const answer = (await call("fx.status")).result ?? {};
  return answer.parts?.find((p) => p.part === "dripper")?.emitters?.[0];
};

// Without physics (editing): the crate is not in the way, so chips reach the floor plane.
await page.waitForTimeout(5000);
let c = await chips();
assert.ok(c && c.alive > 20, `chips are alive: ${JSON.stringify(c)}`);
assert.ok(c.min[1] < 0.2, `with no physics running they fall to the ground plane: lowest ${c.min[1]}`);

// In play mode the crate stops them.
await page.evaluate(() => globalThis.__runtime.viewer_play_mode(true));
await page.waitForTimeout(6000);
c = await chips();
assert.ok(c.alive > 20, JSON.stringify(c));
// They fall within the crate's footprint (about 0.4 m across at 3 m up), so every chip is stopped by its top face, 1 m up (the chip's own size adds a little).
assert.ok(c.min[1] > 0.9, `in play mode the chips rest on the crate, never inside it or below: lowest ${c.min[1]} (x ${c.min[0]}..${c.max[0]})`);
assert.ok(c.max[1] > 1.5, `and fresh ones are still falling from above: highest ${c.max[1]}`);
await page.screenshot({ path: "/tmp/ve-look/fx_physics_live.png" });
assert.deepEqual(problems, [], "no page errors");
console.log(`ok: ${c.alive} chips, lowest ${c.min[1].toFixed(3)} m in play mode`);
await browser.close();
