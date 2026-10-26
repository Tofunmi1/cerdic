// The playtest policy aim:<thing>:<targets> through the real stack (MCP server over stdio, the editor page over its WebSocket): it must win a course that
// can be won and report honestly on one that cannot. Needs the static server on 127.0.0.1:8801. bun tools/aim-playtest-live-test.mjs [good-world] [bad-world]
import { spawn } from "node:child_process";
import { chromium } from "../web/node_modules/playwright/index.mjs";

const good = process.argv[2] ?? "worlds/minigolf2.txt";
const bad = process.argv[3] ?? "worlds/minigolf6.txt";
const server = spawn("bun", ["tools/mcp/server.mjs"], { env: { ...process.env, VE_MCP_PORT: "8800" }, stdio: ["pipe", "pipe", "inherit"] });
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
const call = async (name, args) => { const r = await rpc("tools/call", { name, arguments: args }); return { text: (r.result?.content ?? []).map((c) => c.text ?? "").join("\n"), isError: r.result?.isError === true }; };
let failed = 0;
const check = (name, ok, detail) => { console.log(`${ok ? "ok  " : "FAIL"} ${name}${ok ? "" : `: ${String(detail).slice(0, 800)}`}`); if (!ok) failed++; };

await rpc("initialize", { protocolVersion: "2024-11-05" });
const browser = await chromium.launch({ channel: "chromium", args: ["--enable-unsafe-webgpu", "--enable-features=Metal,WebGPU", "--ignore-gpu-blocklist"] });
async function play(world) {
  const page = await browser.newPage({ viewport: { width: 1000, height: 650 } });
  const errors = [];
  page.on("pageerror", (e) => errors.push(String(e).slice(0, 300)));
  await page.goto(`http://localhost:8801/?viewer=1&eval=1&mcp=8800&scene=open&quality=low&world=${world}`);
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 120000 });
  await page.evaluate(async () => { window.__rt = await import("./ve_runtime.js"); });
  await page.waitForFunction(() => JSON.parse(window.__rt.eval_projection_status()).committed?.quality === "final", null, { timeout: 400000 });
  await page.waitForFunction(() => JSON.parse(window.__rt.game_status()).present, null, { timeout: 60000 });
  await page.keyboard.press("h");
  await new Promise((r) => setTimeout(r, 2500));
  const answer = await call("playtest", { policy: "aim:ball:cup1,cup2,cup3" });
  await page.close();
  return { answer, errors };
}

let r = await call("playtest", { policy: "aim:ball" });
check("a malformed aim policy is refused with the form", r.isError && /aim:/.test(r.text), r.text);

const g = await play(good);
check(`${good}: the policy runs`, !g.answer.isError, g.answer.text);
let body = null;
try { body = JSON.parse(g.answer.text); } catch { /* text */ }
check(`${good}: the course is won`, body?.status === "won" || /won/.test(body?.status ?? "") || /"won"\s*:\s*true/.test(g.answer.text), g.answer.text);
check(`${good}: it reports strokes per target`, /strokes/.test(g.answer.text), g.answer.text);
check(`${good}: no page errors`, g.errors.length === 0, g.errors);

const b = await play(bad);
let bodyBad = null;
try { bodyBad = JSON.parse(b.answer.text); } catch { /* text */ }
console.log("bad world answer:", b.answer.text.slice(0, 700));
check(`${bad}: the policy runs and says the course was not won (the fault is reported, not hidden)`, !b.answer.isError && !(bodyBad?.status === "won"), b.answer.text);

console.log(failed ? `\n${failed} check(s) failed` : "\nall checks passed");
await browser.close();
server.kill();
process.exit(failed ? 1 : 0);
