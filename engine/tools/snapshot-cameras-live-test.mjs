// snapshot with explicit cameras through the real stack (MCP server over stdio, the editor page over its WebSocket).
// Needs the static server on 127.0.0.1:8801 (web/dist-bevy-gpu). bun tools/snapshot-cameras-live-test.mjs
import { spawn } from "node:child_process";
import { chromium } from "../web/node_modules/playwright/index.mjs";

const server = spawn("bun", ["tools/mcp/server.mjs"], { env: { ...process.env, VE_MCP_PORT: "8798" }, stdio: ["pipe", "pipe", "inherit"] });
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
const call = async (name, args) => { const r = await rpc("tools/call", { name, arguments: args }); const content = r.result?.content ?? []; return { text: content.map((c) => c.text ?? "").join("\n"), images: content.filter((c) => c.type === "image"), isError: r.result?.isError === true }; };
let failed = 0;
const check = (name, ok, detail) => { console.log(`${ok ? "ok  " : "FAIL"} ${name}${ok ? "" : `: ${String(detail).slice(0, 600)}`}`); if (!ok) failed++; };

await rpc("initialize", { protocolVersion: "2024-11-05" });
const browser = await chromium.launch({ channel: "chromium", args: ["--enable-unsafe-webgpu", "--enable-features=Metal,WebGPU", "--ignore-gpu-blocklist"] });
const page = await browser.newPage({ viewport: { width: 1000, height: 650 } });
const errors = [];
page.on("pageerror", (e) => errors.push(String(e).slice(0, 300)));
await page.goto("http://localhost:8801/?viewer=1&eval=1&mcp=8798&scene=open&quality=low&world=worlds/cottage_a.txt");
await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 120000 });
await new Promise((r) => setTimeout(r, 5000));

await call("snapshot", { name: "base", image: false });
await new Promise((r) => setTimeout(r, 1500));
const before = await call("snapshot", { name: "base", image: false });
const settled = /"changedFraction"\s*:\s*0(\.0+)?\s*[,}\n]/.test(before.text);
check("the scene is stable before the test (two baselines agree)", settled, before.text.slice(0, 300));
let r = await call("snapshot", { cameras: [
  { position: [8, 4, 8], target: [0, 1.5, 0], label: "corner" },
  { position: [0, 2, 9], target: [0, 1.3, 0], label: "front" },
  { position: [0, 14, 0.1], target: [0, 0, 0], label: "above" },
] });
check("three cameras give one picture", !r.isError && r.images.length === 1, r.text);
check("the answer names the three cameras", ["corner", "front", "above"].every((l) => r.text.includes(l)), r.text);
const sheetBytes = Buffer.from(r.images[0]?.data ?? "", "base64").length;
check("the contact sheet is a real picture", sheetBytes > 3000, sheetBytes);
const frac = (t) => Number(/"changedFraction"\s*:\s*([0-9.eE+-]+)/.exec(t)?.[1] ?? Number.NaN);
const afterSheet = await call("snapshot", { name: "base", image: false });
check("the view is restored after a camera sheet", frac(afterSheet.text) < 0.01, `changedFraction ${frac(afterSheet.text)}`);
r = await call("snapshot", { camera: { position: [6, 3, 6], target: [0, 1.5, 0] } });
check("one camera gives one picture", !r.isError && r.images.length === 1, r.text);
const afterOne = await call("snapshot", { name: "base", image: false });
check("the view is restored after a single camera", frac(afterOne.text) < 0.01, `changedFraction ${frac(afterOne.text)}`);
const single = Buffer.from(r.images[0]?.data ?? "", "base64");
r = await call("snapshot", { cameras: [{ position: [0, 2, 9], target: [0, 1.3, 0] }, { position: [9, 2, 0], target: [0, 1.3, 0] }] });
const pair = Buffer.from(r.images[0]?.data ?? "", "base64");
check("different cameras give different pictures", !single.equals(pair), "");
const after = await call("snapshot", { name: "base", image: false });
check("the view is restored after the cameras (no change against the earlier baseline)", !after.isError && /"changed"\s*:\s*(false|0)|share[^\n]*0(\.0+)?\b|unchanged|no change/i.test(after.text), `before=${before.text.slice(0, 200)} after=${after.text.slice(0, 300)}`);
r = await call("snapshot", { cameras: [{ position: [0, 5], target: [0, 0, 0] }] });
check("a bad camera is refused with the shape", r.isError && /position/.test(r.text), r.text);
check("no page errors", errors.length === 0, errors);

console.log(failed ? `\n${failed} check(s) failed` : "\nall checks passed");
await browser.close();
server.kill();
process.exit(failed ? 1 : 0);
