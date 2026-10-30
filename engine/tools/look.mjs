// Looks at a world in the real viewer and saves pictures: the way to SEE what a build looks like (a model's own report is not evidence).
//   bun tools/look.mjs worlds/cottage_c.txt cot --cams '[{"position":[9,4,9],"target":[0,1.5,0],"label":"corner"}]'
//   bun tools/look.mjs worlds/apex_set3.txt a3 --scene open --tour           (a survey tour: the survey tool picks the cameras)
//   bun tools/look.mjs worlds/pegasus_studio.txt peg --scene stage --states '[{"grounded":false},{"grounded":true,"speed":4}]' --view iso
// The world path is relative to web/dist-bevy-gpu (put the file in worlds/ there). Needs the static server on 127.0.0.1:8801 and a built viewer.
// Pictures go to --out (default /tmp/ve-look) as <prefix>_<n>.jpg; the answer text of each snapshot is printed. A heavy world can take minutes to evaluate in the page.
import { spawn } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { chromium } from "../web/node_modules/playwright/index.mjs";

const args = process.argv.slice(2);
const flag = (name, fallback) => { const i = args.indexOf(name); if (i < 0) return fallback; const [, value] = args.splice(i, 2); return value; };
const has = (name) => { const i = args.indexOf(name); if (i < 0) return false; args.splice(i, 1); return true; };
const scene = flag("--scene", "open");
const quality = flag("--quality", "high");
const cams = JSON.parse(flag("--cams", "[]"));
const times = JSON.parse(flag("--times", "null"));
const states = JSON.parse(flag("--states", "null"));
const view = flag("--view", "iso");
const out = flag("--out", "/tmp/ve-look");
const port = flag("--mcp-port", "8830");
const width = Number(flag("--width", "512"));
const tour = has("--tour");
const [world, prefix = "look"] = args;
if (!world) { console.error("usage: bun tools/look.mjs <world under dist-bevy-gpu> [prefix] [--scene open|stage] [--cams JSON] [--tour] [--times JSON] [--states JSON] [--view iso]"); process.exit(2); }
mkdirSync(out, { recursive: true });

const server = spawn("bun", ["tools/mcp/server.mjs"], { env: { ...process.env, VE_MCP_PORT: port }, stdio: ["pipe", "pipe", "inherit"] });
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
const call = async (name, a) => { const r = await rpc("tools/call", { name, arguments: a }); const c = r.result?.content ?? []; return { text: c.map((x) => x.text ?? "").join("\n"), images: c.filter((x) => x.type === "image"), isError: r.result?.isError === true }; };
let count = 0;
const save = (label, r) => {
  if (r.isError) { console.log(`${label}: ERROR ${r.text.slice(0, 400)}`); return; }
  r.images.forEach((image) => { const file = `${out}/${prefix}_${++count}.jpg`; writeFileSync(file, Buffer.from(image.data, "base64")); console.log(`${label}: ${file}`); });
  console.log(r.text.slice(0, 300).replace(/\s+/g, " "));
};

await rpc("initialize", { protocolVersion: "2024-11-05" });
const browser = await chromium.launch({ channel: "chromium", args: ["--enable-unsafe-webgpu", "--enable-features=Metal,WebGPU", "--ignore-gpu-blocklist"] });
const page = await browser.newPage({ viewport: { width: 1100, height: 700 } });
page.on("pageerror", (e) => console.log("PAGEERR", String(e).slice(0, 300)));
await page.goto(`http://127.0.0.1:8801/?viewer=1&eval=1&mcp=${port}&scene=${scene}&quality=${quality}&bloom=on&world=${encodeURIComponent(world)}`);
await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 120000 });
await page.evaluate(async () => { window.__rt = await import("./ve_runtime.js"); });
await page.waitForFunction(() => JSON.parse(window.__rt.eval_projection_status()).committed?.quality === "final", null, { timeout: 480000 });
await new Promise((r) => setTimeout(r, 6000));
if (cams.length > 6) throw new Error("at most 6 cameras per sheet: pass fewer, or call again");
if (cams.length) save("cameras", await call("snapshot", { cameras: cams, width }));
if (tour) save("tour", await call("snapshot", { tour: true, width }));
if (times) save("times", await call("snapshot", { times, view }));
if (states) save("states", await call("snapshot", { states, view }));
if (!cams.length && !tour && !times && !states) save("default", await call("snapshot", { width }));
await browser.close();
server.kill();
