// Game entities drawn as their own animated models (ADR 0066): node tools/model-instance-test.mjs [world] [outdir]
// Loads worlds/scuttler_game.txt (three walker kinds on one robot hierarchy, each walking its own way, halting at 4 s with a jump), samples
// the game's entity positions and takes a picture every second, and checks: each entity is drawn as a model (not a plain copy), they move apart,
// the legs of each walk only while it moves (their poses differ between entities), and the anim trigger reaches every entity.
import { chromium } from "../web/node_modules/playwright/index.mjs";
import { mkdirSync } from "node:fs";
const world = process.argv[2] ?? "worlds/scuttler_game.txt";
const outDir = process.argv[3] ?? "/tmp/model-instance";
mkdirSync(outDir, { recursive: true });
const browser = await chromium.launch({ channel: "chromium", args: ["--enable-unsafe-webgpu", "--enable-features=Metal,WebGPU", "--ignore-gpu-blocklist"] });
const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
page.on("pageerror", (e) => console.log("PAGEERR", String(e).slice(0, 300)));
page.on("console", (m) => { if (m.type() === "error" || /panick/i.test(m.text())) console.log("CONSOLE", m.text().slice(0, 300)); });
await page.goto(`http://localhost:8801/?viewer=1&eval=1&scene=stage&quality=low&world=${world}`);
await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 120000 });
await page.evaluate(async () => { window.__rt = await import("./ve_runtime.js"); });
await page.waitForFunction(() => JSON.parse(window.__rt.eval_projection_status()).committed?.quality === "final", null, { timeout: 300000 });
await page.keyboard.press("h");
await page.evaluate(() => window.__rt.viewer_look_at(0, 5.5, 9, 0, 0.3, 1));
let failed = 0;
const check = (name, condition, detail) => { console.log(`${condition ? "ok  " : "FAIL"} ${name}${condition ? "" : `: ${JSON.stringify(detail).slice(0, 400)}`}`); if (!condition) failed++; };
const call = (method, params = {}) => page.evaluate(async ({ method, params }) => {
  const id = window.__rt.world_call(method, JSON.stringify(params));
  for (let i = 0; i < 100; i++) { const a = window.__rt.world_take(id); if (a) return JSON.parse(a); await new Promise((r) => setTimeout(r, 30)); }
}, { method, params });
const status = () => page.evaluate(() => JSON.parse(window.__rt.game_status()));
const start = await status();
check("the game is loaded with three entities", start.present && start.entities.length === 3, start);
const samples = [];
// The leg and the body of each entity's own copy, sampled every ~150 ms: legs move while it walks, the body rises when the jump arrives.
const fine = { leg: new Map(), body: new Map() };
const grab = async (clock) => {
  for (const e of start.entities) {
    const leg = await call("world.get_components", { entity: `lf_upper#${e.id}`, components: ["Transform"] });
    const body = await call("world.get_components", { entity: `robot#${e.id}`, components: ["Transform"] });
    const push = (map, r) => { const list = map.get(e.id) ?? []; list.push({ clock, t: r?.result?.components?.Transform }); map.set(e.id, list); };
    push(fine.leg, leg); push(fine.body, body);
  }
};
const began = Date.now();
for (let t = 0; t < 7; t++) {
  const s = await status();
  samples.push({ t, entities: s.entities.map((e) => ({ id: e.id, p: e.position })), vars: s.vars, drawn: s.drawn, effects: s.effects?.length ?? 0 });
  await page.screenshot({ path: `${outDir}/t${t}.png` });
  for (let k = 0; k < 4; k++) { await grab((Date.now() - began) / 1000); await page.waitForTimeout(100); }
}
console.log(JSON.stringify(samples.map((s) => ({ t: s.t, drawn: s.drawn, pos: s.entities.map((e) => e.p.map((v) => +v.toFixed(2))) }))));
check("the entities are not drawn as plain copies", samples.every((s) => s.drawn === 0), samples.map((s) => s.drawn));
const found = [];
for (const e of start.entities) found.push({ id: e.id, hit: await call("world.find", { name: `robot#${e.id}` }) });
check("each entity has its own clone of the robot part (named robot#id)", found.every((f) => f.hit.result?.entities?.length === 1), found);
const stillAuthored = await call("world.find", { name: "robot" });
check("the authored robot is still in the scene (hidden, as the prototype)", stillAuthored.result?.entities?.length === 1, stillAuthored);
const proto = stillAuthored.result?.entities?.[0]?.entity;
const vis = proto ? await call("world.get_components", { entity: proto, components: ["Visibility"] }) : null;
check("and it is hidden", vis?.result?.components?.Visibility === "Hidden", vis);
const spread = (list, pick) => { const v = list.map(pick).filter((x) => Number.isFinite(x)); return v.length ? Math.max(...v) - Math.min(...v) : 0; };
for (const e of start.entities) {
  const leg = fine.leg.get(e.id) ?? [], body = fine.body.get(e.id) ?? [];
  const walking = leg.filter((s) => s.clock < 3.5);
  const rot = (i) => (s) => s.t?.rotation?.[i];
  const moveW = [0, 1, 2].reduce((sum, i) => sum + spread(walking, rot(i)), 0);
  check(`entity ${e.id}: its leg is posed over time while it walks`, moveW > 0.05, { moveW, n: walking.length, first: walking[0], last: walking.at(-1) });
  const jumpRise = spread(body.filter((s) => s.clock > 3.5), (s) => s.t?.translation?.[1]);
  check(`entity ${e.id}: the jump trigger reached its own machine (its body moves up after the halt)`, jumpRise > 0.03, { jumpRise, n: body.length });
}
console.log("leg rotation in the 3rd sample, per entity:", start.entities.map((e) => JSON.stringify((fine.leg.get(e.id) ?? [])[2]?.t?.rotation)).join(" | "));
const moved = (id) => { const a = samples[0].entities.find((e) => e.id === id).p, b = samples[3].entities.find((e) => e.id === id).p; return Math.hypot(b[0] - a[0], b[2] - a[2]); };
check("each entity walked at least 2.5 m in 3 s", start.entities.every((e) => moved(e.id) > 2.5), start.entities.map((e) => moved(e.id)));
const ends = samples.at(-1).entities.map((e) => e.p);
check("they stopped (the halt rule ran)", Math.hypot(ends[0][0] - samples.at(-2).entities[0].p[0], ends[0][2] - samples.at(-2).entities[0].p[2]) < 0.05, { last: samples.at(-1), before: samples.at(-2) });
console.log(failed ? `\n${failed} check(s) failed` : "\nall checks passed");
await browser.close();
process.exit(failed ? 1 : 0);
