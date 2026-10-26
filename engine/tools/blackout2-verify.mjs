// Plays Blackout 2 through simulated hands: node tools/blackout2-verify.mjs [world] [outdir]
// Seats the first fuse (door 1 opens), walks into the second room, lets a creature notice and catch you (a life is lost), then grabs a crate
// and THROWS it at the creature with a swinging hand (the hand moves every frame, so the release velocity is real): the creature must be stunned,
// the crate gone, an explosion drawn. Prints what it saw; exits non-zero when a check fails.
import { chromium } from "../web/node_modules/playwright/index.mjs";
import { mkdirSync } from "node:fs";
const world = process.argv[2] ?? "worlds/blackout2.txt";
const outDir = process.argv[3] ?? "/tmp/blackout2";
mkdirSync(outDir, { recursive: true });
const browser = await chromium.launch({ channel: "chromium", args: ["--enable-unsafe-webgpu", "--enable-features=Metal,WebGPU", "--ignore-gpu-blocklist"] });
const page = await browser.newPage({ viewport: { width: 1100, height: 700 } });
const logs = [];
page.on("pageerror", (e) => logs.push(String(e).slice(0, 300)));
await page.goto(`http://localhost:8801/?viewer=1&eval=1&scene=open&quality=low&world=${world}`);
await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 120000 });
await page.evaluate(async () => { window.__rt = await import("./ve_runtime.js"); });
await page.waitForFunction(() => JSON.parse(window.__rt.eval_projection_status()).committed?.quality === "final", null, { timeout: 400000 });
await page.waitForFunction(() => JSON.parse(window.__rt.game_status()).present, null, { timeout: 60000 });
await page.keyboard.press("h");
let failed = 0;
const check = (name, condition, detail) => { console.log(`${condition ? "ok  " : "FAIL"} ${name}${condition ? "" : `: ${JSON.stringify(detail).slice(0, 500)}`}`); if (!condition) failed++; };
const status = () => page.evaluate(() => JSON.parse(window.__rt.game_status()));
const kinds = (s, re) => s.entities.filter((e) => re.test(e.kind));
const pause = (ms) => page.waitForTimeout(ms);
const hand = async (at, grip = false, trigger = false) => {
  await page.evaluate(([a, g, t]) => window.__rt.game_set_hands(JSON.stringify([{ side: "right", position: a, grip: g, trigger: t }])), [at, grip, trigger]);
  await pause(120);
};
const standAt = ([x, z], look = [x, z + 5]) => page.evaluate(([px, pz, lx, lz]) => window.__rt.viewer_look_at(px, 1.6, pz, lx, 1.2, lz), [x, z, look[0], look[1]]);

// 1. Door 1: carry the first fuse to its socket.
await hand([-2.6, 1.0, -2.3]); await hand([-2.6, 1.0, -2.3], true); await hand([0, 1.3, 0], true); await hand([1.5, 1.3, 2.7], true); await pause(200); await hand([1.5, 1.3, 2.7], false);
let s = await status();
check("the first fuse is seated and door 1 is open", s.vars.fuses === 1, s.vars);
await hand([0, -10, 0]);

// 2. Into the second room: a creature should notice, chase and catch.
await standAt([0, 4.6], [0, 11]);
const lives0 = s.vars.lives;
let caught = false;
for (let i = 0; i < 60 && !caught; i++) { await pause(500); s = await status(); caught = s.vars.lives < lives0; }
await page.screenshot({ path: `${outDir}/caught.png` });
check("a creature caught the hero and cost a life", caught, s.vars);
const hall = kinds(s, /scuttler_hall/)[0]?.position;

// 3. Throw a crate at the nearest creature, with a hand that moves every frame.
const result = await page.evaluate(async () => {
  const rt = window.__rt;
  const frame = () => new Promise((r) => requestAnimationFrame(r));
  const get = () => JSON.parse(rt.game_status());
  const setHand = (p, grip) => rt.game_set_hands(JSON.stringify([{ side: "right", position: p, grip, trigger: false }]));
  const sub = (a, b) => a.map((v, i) => v - b[i]);
  const len = (a) => Math.hypot(...a);
  // The crate nearest the second room's creature (it must be lying on the floor, not already thrown).
  let st = get();
  const creature = st.entities.find((e) => /scuttler_hall/.test(e.kind));
  const crates = st.entities.filter((e) => /throwable_crate/.test(e.kind));
  if (!creature || !crates.length) return { error: "no creature or no crates", creature, crates: crates.length };
  crates.sort((a, b) => len(sub(a.position, creature.position)) - len(sub(b.position, creature.position)));
  const crate = crates[0];
  const stunnedBefore = st.vars;
  // Grip it where it lies, lift it, then swing toward the creature each frame and let go.
  let at = [crate.position[0], crate.position[1] + 0.05, crate.position[2]];
  for (let i = 0; i < 3; i++) { setHand(at, false); await frame(); }
  for (let i = 0; i < 4; i++) { setHand(at, true); await frame(); }
  const up = [at[0], 1.15, at[2]];
  for (let i = 1; i <= 8; i++) { setHand(at.map((v, k) => v + (up[k] - v) * i / 8), true); await frame(); }
  at = up;
  const trail = [];
  for (let i = 0; i < 12; i++) {
    st = get();
    const c = st.entities.find((e) => /scuttler_hall/.test(e.kind)) ?? creature;
    const aim = [c.position[0], 0.55, c.position[2]];
    const dir = sub(aim, at); const d = len(dir);
    const step = Math.min(0.14, d);
    at = at.map((v, k) => v + dir[k] / d * step);
    setHand(at, true);
    trail.push({ hand: at.map((v) => +v.toFixed(2)), to: +d.toFixed(2) });
    await frame();
  }
  setHand(at, false);
  const after = [];
  for (let i = 0; i < 90; i++) {
    await frame();
    st = get();
    const cr = st.entities.filter((e) => /throwable_crate/.test(e.kind)).length;
    const c = st.entities.find((e) => /scuttler_hall/.test(e.kind));
    after.push({ steps: st.steps, crates: cr, c: c?.position, bursts: st.bursts?.requested, particles: st.bursts?.particles, vars: st.vars });
  }
  return { crate: crate.kind, crateStart: crate.position, creature: creature.position, trail, after, stunnedBefore };
});
if (result.error) { check("a creature and crates exist", false, result); }
else {
  const last = result.after.at(-1);
  console.log("crate", result.crate, "start", JSON.stringify(result.crateStart), "creature", JSON.stringify(result.creature), "last", JSON.stringify(last).slice(0, 300));
  check("the thrown crate is gone (it hit)", last.crates === 5, { crates: last.crates, trail: result.trail.slice(-3) });
  check("an explosion burst was requested", (last.bursts ?? 0) >= 1, last);
  check("particles were drawn at some point", Math.max(...result.after.map((a) => a.particles ?? 0)) > 20, result.after.slice(0, 5));
  const thrownVar = Object.entries(last.vars).filter(([k, v]) => /^thrown/.test(k) && v >= 1);
  check("the game counted the throw", thrownVar.length >= 1, last.vars);
  // The creature stands still for a while after the hit.
  const hitIndex = result.after.findIndex((a) => a.crates === 5);
  const frozen = result.after.slice(hitIndex + 3, hitIndex + 50).filter((a) => a.c);
  const drift = frozen.length > 1 ? Math.hypot(frozen.at(-1).c[0] - frozen[0].c[0], frozen.at(-1).c[2] - frozen[0].c[2]) : 99;
  check("the creature stands still after being hit", hitIndex >= 0 && drift < 0.15, { hitIndex, drift });
}
await page.screenshot({ path: `${outDir}/after.png` });
check("no page errors", logs.length === 0, logs);
console.log(failed ? `\n${failed} check(s) failed` : "\nall checks passed");
await browser.close();
process.exit(failed ? 1 : 0);
