// Every start_from template applies in the editor page and passes `check` without errors, through the MCP server and the real page.
// Needs the static server on 127.0.0.1:8801 (web/dist-bevy-gpu). bun tools/templates-live-test.mjs
import { spawn } from "node:child_process";
import { chromium } from "../web/node_modules/playwright/index.mjs";

const server = spawn("bun", ["tools/mcp/server.mjs"], { env: { ...process.env, VE_MCP_PORT: "8797" }, stdio: ["pipe", "pipe", "inherit"] });
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
const check = (name, ok, detail) => { console.log(`${ok ? "ok  " : "FAIL"} ${name}${ok ? "" : `: ${String(detail).slice(0, 600)}`}`); if (!ok) failed++; };

await rpc("initialize", { protocolVersion: "2024-11-05" });
const browser = await chromium.launch({ channel: "chromium", args: ["--enable-unsafe-webgpu", "--enable-features=Metal,WebGPU", "--ignore-gpu-blocklist"] });
const names = JSON.parse((await call("start_from", {})).text).templates.map((t) => t.name);
for (const name of names) {
  const page = await browser.newPage({ viewport: { width: 900, height: 600 } });
  const errors = [];
  page.on("pageerror", (e) => errors.push(String(e).slice(0, 300)));
  await page.goto("http://localhost:8801/?viewer=1&eval=1&mcp=8797&scene=open&quality=low");
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 120000 });
  await new Promise((r) => setTimeout(r, 3000));
  const applied = await call("start_from", { template: name, apply: true });
  check(`${name}: applies`, !applied.isError, applied.text);
  const verdict = await call("check", {});
  let body = null;
  try { body = JSON.parse(verdict.text); } catch { /* text */ }
  const errs = Array.isArray(body?.problems ?? body?.issues ?? body) ? (body.problems ?? body.issues ?? body).filter((p) => p.severity === "error") : [];
  check(`${name}: check has no errors`, !verdict.isError && errs.length === 0, verdict.text);
  check(`${name}: no page errors`, errors.length === 0, errors);
  await page.close();
}
console.log(failed ? `\n${failed} check(s) failed` : "\nall checks passed");
await browser.close();
server.kill();
process.exit(failed ? 1 : 0);
