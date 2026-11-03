// Live check of animation layers and events (plan W2): a body walks (a bob and an arm swing), `step` events fire at 0.25 s and 0.75 s of each loop,
// and a `wave` overlay masked to the arm takes the arm over when the `waving` parameter goes up and lets go when it drops.
//   bun tools/animation-layers-live-test.mjs
// Needs the static server on 127.0.0.1:8801 serving web/dist-bevy-gpu with a built viewer.
import assert from "node:assert/strict";
import { chromium } from "../web/node_modules/playwright/index.mjs";

const browser = await chromium.launch({ channel: "chromium", args: ["--enable-unsafe-webgpu", "--enable-features=Metal,WebGPU", "--ignore-gpu-blocklist"] });
const page = await browser.newPage({ viewport: { width: 640, height: 400 } });
const problems = [];
page.on("pageerror", (e) => problems.push(String(e).slice(0, 300)));
page.on("console", (m) => { if (m.type() === "error" && !/404/.test(m.text())) problems.push(m.text().slice(0, 300)); });
await page.goto("http://localhost:8801/?viewer=1&eval=1&scene=stage&quality=low&world=worlds/layers_test.txt");
await page.waitForFunction(() => globalThis.__ve?.evaluation?.parts?.length >= 3, null, { timeout: 120000 });
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
const result = (answer) => answer.result ?? answer;
const centre = async (name) => result(await call("world.bounds", { entity: name })).center;
const events = () => page.evaluate(() => JSON.parse(globalThis.__runtime.world_events_since(0)).events.filter((e) => e.name === "anim.step"));

// Footsteps: the clip is one second long, so about two per second, from the machine's part.
await page.waitForTimeout(4000);
const steps = await events();
assert.ok(steps.length >= 3, `footsteps were reported: ${steps.length}`);
assert.ok(steps.every((e) => e.part === "body" && e.clip === "walk"), JSON.stringify(steps.slice(0, 2)));
const rate = steps.length;
await page.waitForTimeout(3000);
const later = (await events()).length;
assert.ok(later > rate, "they keep coming");

// The wave: the highest the arm gets over a few seconds (samples can miss a valley, never the peak of a 2 s loop with two humps), with and without
// the overlay. Walking swings the arm 10 degrees (up to about 1.41 m at the arm's middle); the wave lifts it 70 degrees (about 1.53 m).
const peak = async () => {
  let top = -Infinity;
  for (let i = 0; i < 24; i++) {
    top = Math.max(top, (await centre("arm"))[1]);
    await page.waitForTimeout(110);
  }
  return top;
};
const walking = await peak();
assert.ok(walking < 1.45, `walking keeps the arm low: ${walking.toFixed(3)}`);
await call("animation.set_float", { part: "body", name: "waving", value: 1 });
await page.waitForTimeout(800);
const waving = await peak();
assert.ok(waving > 1.48, `the wave raises the arm: ${waving.toFixed(3)} m against ${walking.toFixed(3)} walking`);
await page.screenshot({ path: "/tmp/ve-look/layers_live.png" });
await call("animation.set_float", { part: "body", name: "waving", value: 0 });
await page.waitForTimeout(1200);
const after = await peak();
assert.ok(after < 1.45, `the arm lets go when the parameter drops: ${after.toFixed(3)}`);
assert.deepEqual(problems, [], "no page errors");
console.log(`ok: ${later} footsteps; arm peak walking ${walking.toFixed(3)}, waving ${waving.toFixed(3)}, after ${after.toFixed(3)}`);
await browser.close();
