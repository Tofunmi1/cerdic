// The water.fill 3D volume and its coupled rendered surface: steps, disturbance, propagation and damping.
// Needs the static server on 127.0.0.1:8801 (web/dist-bevy-gpu). bun tools/water-sim-live-test.mjs
import { spawn } from "node:child_process";
import { chromium } from "../web/node_modules/playwright/index.mjs";

const server = spawn("bun", ["tools/mcp/server.mjs"], { env: { ...process.env, VE_MCP_PORT: "8814" }, stdio: ["pipe", "pipe", "inherit"] });
let buffer = "";
const waiting = new Map();
let nextId = 0;
server.stdout.setEncoding("utf8");
server.stdout.on("data", (chunk) => {
  buffer += chunk;
  let at;
  while ((at = buffer.indexOf("\n")) >= 0) {
    const line = buffer.slice(0, at).trim();
    buffer = buffer.slice(at + 1);
    if (line) { const m = JSON.parse(line); waiting.get(m.id)?.(m); waiting.delete(m.id); }
  }
});
const rpc = (method, params) => new Promise((resolve) => { const id = ++nextId; waiting.set(id, resolve); server.stdin.write(JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n"); });
const call = async (name, args) => { const r = await rpc("tools/call", { name, arguments: args }); return { text: (r.result?.content ?? []).map((x) => x.text ?? "").join("\n"), isError: r.result?.isError === true }; };
const world = async (method, params) => { const r = await call("world", { method, params }); try { return JSON.parse(r.text); } catch { return { ok: false, error: r.text }; } };
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const worldPath = process.env.WATER_WORLD ?? "worlds/sim_test.txt";
const center = Number(process.env.WATER_CENTER ?? 50);
let failed = 0;
const check = (name, ok, detail) => { console.log(`${ok ? "ok  " : "FAIL"} ${name}${ok ? "" : `: ${String(detail).slice(0, 500)}`}`); if (!ok) failed++; };

await rpc("initialize", { protocolVersion: "2024-11-05" });
const browser = await chromium.launch({ channel: "chromium", args: ["--enable-unsafe-webgpu", "--enable-features=Metal,WebGPU", "--ignore-gpu-blocklist"] });
const page = await browser.newPage({ viewport: { width: 900, height: 600 } });
const errors = [];
page.on("pageerror", (e) => errors.push(String(e).slice(0, 300)));
await page.goto(`http://localhost:8801/?viewer=1&eval=1&mcp=8814&scene=open&quality=high&world=${encodeURIComponent(worldPath)}`);
await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 120000 });
await sleep(8000);

const s0 = await world("water.status", {});
check("water.status reports one volumetric filled basin", s0.ok && s0.result?.parts?.length === 1 && s0.result.parts[0].solver === "3d_pbf_reference" && s0.result.parts[0].particles > 0, JSON.stringify(s0).slice(0, 400));
const part = s0.result?.parts?.[0] ?? {};
await sleep(1000);
const s1 = await world("water.status", {});
const p1 = s1.result?.parts?.[0] ?? {};
check("the simulated part keeps its identity while the scene is unchanged", p1.id === part.id, `${part.id} -> ${p1.id}`);
check("the simulation steps with the animation clock (at least 30 steps in a second)", (p1.steps ?? 0) - (part.steps ?? 0) >= 30, `${part.steps} -> ${p1.steps}`);
check("a quiet lake has no ripples", (p1.max_abs ?? 1) < 0.003, p1.max_abs);

const poke = await world("water.disturb", { x: center, z: center, radius: 1.2, amount: 0.36, splash: false });
check("water.disturb reaches one wet part", poke.ok === true && poke.result?.disturbed === 1, JSON.stringify(poke));
await sleep(300);
const a = (await world("water.status", {})).result?.parts?.[0] ?? {};
check("right after the poke there are ripples (max displacement above 2 cm)", (a.max_abs ?? 0) > 0.02, JSON.stringify(a));
const h1 = (await world("water.height", { x: center + 5, z: center })).result?.simulation;
await sleep(2000);
const h2 = (await world("water.height", { x: center + 5, z: center })).result?.simulation;
check("the simulated surface 5 m from the poke changes as the ripples pass", Math.abs(h2 - h1) > 0.002, `${h1} -> ${h2}`);
const b = (await world("water.status", {})).result?.parts?.[0] ?? {};
check("the 3D solver conserves its particle count", b.particles === a.particles && b.particles > 0, `${a.particles} -> ${b.particles}`);
await sleep(9000);
const c = (await world("water.status", {})).result?.parts?.[0] ?? {};
check("damping calms the lake (a tenth of the first ripple's height after 10 s or less)", (c.max_abs ?? 1) < 0.5 * (a.max_abs ?? 0), `${a.max_abs} -> ${c.max_abs}`);

// the picture changes too: two snapshots of the rippling surface differ, with the splash switched off
const cam = { position: [center, 3, center - 7], target: [center, -0.3, center] };
await world("water.disturb", { x: center, z: center, radius: 2.5, amount: 0.25, splash: false });
await sleep(400);
await call("snapshot", { camera: cam, name: "ripple", image: false });
await sleep(600);
const second = await call("snapshot", { camera: cam, name: "ripple", image: false });
const fraction = Number(/"changedFraction":\s*([0-9.e-]+)/.exec(second.text)?.[1] ?? 0);
check("the rendered surface changes between two moments of a spreading ripple", fraction > 0.002, second.text.slice(0, 300));
check("no page errors", errors.length === 0, errors);

console.log(failed ? `\n${failed} check(s) failed` : "\nall checks passed");
await browser.close();
server.kill();
process.exit(failed ? 1 : 0);
