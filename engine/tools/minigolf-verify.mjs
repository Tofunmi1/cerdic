// Plays a 3-hole rolling-ball game through simulated swings: node tools/minigolf-verify.mjs [world] [outdir]
// A controller aims from the ball to the current cup and sizes the swing from the ground itself: ground.profile gives the surface friction and the height along
// the line, and the speed that arrives at the cup at 1 m/s comes from integrating v^2 backwards (friction plus 5/7 g times the slope). It swings frame by frame so
// the head has a real velocity, waits for the ball to rest, and aims again. Reports strokes per hole against par, whether the game advanced and whether it was won.
import { chromium } from "../web/node_modules/playwright/index.mjs";
import { mkdirSync } from "node:fs";
const world = process.argv[2] ?? "worlds/minigolf.txt";
const outDir = process.argv[3] ?? "/tmp/minigolf";
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
await page.waitForTimeout(2500);
let failed = 0;
const check = (name, condition, detail) => { console.log(`${condition ? "ok  " : "FAIL"} ${name}${condition ? "" : `: ${JSON.stringify(detail).slice(0, 500)}`}`); if (!condition) failed++; };
const status = () => page.evaluate(() => JSON.parse(window.__rt.game_status()));
const ballOf = (s) => s.entities.find((e) => e.kind === "ball")?.position;
const cupOf = (s) => s.entities.find((e) => e.kind === `cup${s.vars.hole}`)?.position;
const call = (method, params) => page.evaluate(async ([method, params]) => {
  const id = window.__rt.world_call(method, JSON.stringify(params));
  for (let i = 0; i < 200; i++) { const a = window.__rt.world_take(id); if (a) return JSON.parse(a); await new Promise((r) => setTimeout(r, 30)); }
}, [method, params]);

// The swing speed that arrives at the cup at `arrive` m/s: v^2 grows backwards along the line by 2 * (surface decel + 5/7 g * uphill slope) * ds.
const speedFor = async (ball, cup, arrive = 1.0) => {
  const r = await call("ground.profile", { from: [ball[0], ball[2]], to: [cup[0], cup[2]], samples: 200 });
  if (!r?.ok) throw new Error(`ground.profile: ${JSON.stringify(r)}`);
  const pts = r.result.points;
  let v2 = arrive * arrive;
  for (let i = pts.length - 1; i > 0; i--) {
    const a = pts[i - 1], b = pts[i], ds = b.s - a.s;
    const slope = a.height != null && b.height != null ? (b.height - a.height) / ds : 0;
    const decel = (a.decel + b.decel) / 2;
    v2 += 2 * Math.max(0.05, decel + (5 / 7) * 9.8 * slope) * ds;
  }
  return { speed: Math.sqrt(Math.max(v2, 0.25)), voids: pts.filter((p) => p.height == null).length, surfaces: [...new Set(pts.map((p) => p.surface_name))] };
};

// One swing: the hand moves along `dir` (unit x, z) at `speed` m/s through the ball, starting 0.6 m behind it; the head is 0.9 m out along -Z of the hand.
const swing = (ball, dir, speed) => page.evaluate(async ([ball, dir, speed]) => {
  const rt = window.__rt;
  const frame = () => new Promise((r) => requestAnimationFrame(r));
  const setHand = (p) => rt.game_set_hands(JSON.stringify([{ side: "right", position: p, grip: false, trigger: false, rotation: [0, 0, 0, 1] }]));
  const at = (t) => [ball[0] + dir[0] * t, ball[1], ball[2] + 0.9 + dir[1] * t];
  let t = -0.6;
  setHand(at(t)); for (let i = 0; i < 8; i++) await frame();
  const step = speed / 60;
  while (t < 0.5) { t += step; setHand(at(t)); await frame(); }
  setHand([ball[0], 1.2, ball[2] + 2]);
  for (let i = 0; i < 4; i++) await frame();
}, [ball, dir, speed]);

const restBall = async (previous) => {
  let last = null, still = 0;
  for (let i = 0; i < 160; i++) {
    await page.waitForTimeout(250);
    const s = await status();
    const b = ballOf(s);
    if (!b || s.vars.hole !== previous) return s;
    if (last && Math.hypot(b[0] - last[0], b[2] - last[2]) < 0.002) { if (++still >= 5) return s; } else still = 0;
    last = b;
  }
  return status();
};

const s0 = await status();
console.log("ground:", JSON.stringify(s0.ground));
const results = [];
for (let hole = 1; hole <= 3; hole++) {
  let s = await status();
  check(`hole ${hole}: the game is on hole ${hole} with par ${[2, 3, 3][hole - 1]}`, s.vars.hole === hole && s.vars.par === [2, 3, 3][hole - 1], s.vars);
  await page.screenshot({ path: `${outDir}/hole${hole}-tee.png` });
  const strokesBefore = s.vars.total;
  let done = false, lastPlan = null;
  // The shot size is planned from the ground profile; when a shot overshoots the cup or the ball is reset by the out rule, the next shot is scaled down,
  // and when it stops short it is scaled up, so a hole is not failed by one fixed miss repeated.
  let scale = 1;
  for (let stroke = 1; stroke <= 14 && !done; stroke++) {
    s = await status();
    const ball = ballOf(s), cup = cupOf(s);
    if (!ball || !cup) { await page.waitForTimeout(500); continue; }
    const dx = cup[0] - ball[0], dz = cup[2] - ball[2], dist = Math.hypot(dx, dz);
    const dir = [dx / dist, dz / dist];
    const plan = await speedFor(ball, cup);
    lastPlan = plan;
    const speed = Math.min(16, plan.speed * scale);
    await swing(ball, dir, speed);
    const after = await restBall(hole);
    const b = ballOf(after);
    if (after.vars.hole !== hole || after.status === "won") { done = true; break; }
    if (b) {
      const reach = ((b[0] - ball[0]) * dx + (b[2] - ball[2]) * dz) / (dist * dist);
      const reset = Math.hypot(b[0] - ball[0], b[2] - ball[2]) < 0.05;
      scale *= reset ? 0.7 : reach > 1.05 ? 0.8 : reach < 0.9 ? 1.15 : 1;
    }
    console.log(`  hole ${hole} stroke ${stroke}: dist ${dist.toFixed(2)} speed ${speed.toFixed(2)} (surfaces ${plan.surfaces.join("/")}, voids ${plan.voids}) -> ${b ? b.map((v) => v.toFixed(2)) : "gone (out?)"} strokes ${after.vars.strokes}`);
  }
  s = await status();
  const strokes = s.vars.total - strokesBefore;
  results.push({ hole, strokes });
  console.log(`hole ${hole} done in ${strokes} strokes (par ${[2, 3, 3][hole - 1]}); now hole ${s.vars.hole}, status ${s.status}`);
  check(`hole ${hole}: the ball went in the cup`, done, { vars: s.vars, lastPlan });
  await page.screenshot({ path: `${outDir}/hole${hole}-done.png` });
}
const final = await status();
console.log("final", final.status, JSON.stringify(final.vars), "hud", JSON.stringify(final.hud?.lines));
check("the game is won after the third hole", final.status === "won", final.vars);
check("no page errors", logs.length === 0, logs);
console.log(failed ? `\n${failed} check(s) failed` : "\nall checks passed");
await browser.close();
process.exit(failed ? 1 : 0);
