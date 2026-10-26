// The compact get_text, edit and diff tools through the real stack: MCP server over stdio, the editor page over its WebSocket.
// Needs the static server on 127.0.0.1:8801 (web/dist-bevy-gpu). bun tools/mcp-edit-diff-live-test.mjs
import { spawn } from "node:child_process";
import { chromium } from "../web/node_modules/playwright/index.mjs";

const server = spawn("bun", ["tools/mcp/server.mjs"], { env: { ...process.env, VE_MCP_PORT: "8796" }, stdio: ["pipe", "pipe", "inherit"] });
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
const check = (name, ok, detail) => { console.log(`${ok ? "ok  " : "FAIL"} ${name}${ok ? "" : `: ${String(detail).slice(0, 700)}`}`); if (!ok) failed++; };

await rpc("initialize", { protocolVersion: "2024-11-05" });
const browser = await chromium.launch({ channel: "chromium", args: ["--enable-unsafe-webgpu", "--enable-features=Metal,WebGPU", "--ignore-gpu-blocklist"] });
const page = await browser.newPage({ viewport: { width: 1000, height: 650 } });
const errors = [];
page.on("pageerror", (e) => errors.push(String(e).slice(0, 300)));
await page.goto("http://localhost:8801/?viewer=1&eval=1&mcp=8796&scene=open&quality=low&world=worlds/compact_test.txt");
await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 120000 });
await new Promise((r) => setTimeout(r, 4000));

let r = await call("get_text", {});
check("get_text is compact by default (no versions, pipes)", !r.isError && !r.text.includes("@1") && r.text.includes(" | "), r.text.slice(0, 400));
const canonical = await call("get_text", { format: "canonical" });
check("canonical get_text keeps versions", canonical.text.includes("@1") && canonical.text.length > r.text.length, canonical.text.slice(0, 200));

r = await call("edit", { dry_run: true, ops: [{ op: "set", node: "post_1", params: { size: [0.2, 1.2, 0.2] } }] });
check("edit dry_run reports it would apply", !r.isError && /would_apply|delta/.test(r.text), r.text);
r = await call("edit", { ops: [{ op: "set", node: "post_1", params: { size: [0.2, 1.2, 0.2] } }, { op: "rename", node: "crate_3_1", to: "crate_base" }] });
check("edit applies set and rename by label", !r.isError, r.text);
const after = await call("get_text", {});
check("the edit shows in the text", after.text.includes("0.2, 1.2, 0.2") && after.text.includes("crate_base"), after.text.slice(0, 600));
r = await call("edit", { ops: [{ op: "set", node: "nope", params: { size: [1, 1, 1] } }] });
check("an unknown label is an error that names candidates or says unknown", r.isError && /nope/.test(r.text), r.text);
r = await call("diff", {});
check("diff shows the changed lines since the previous revision", !r.isError && /post_1|crate_base/.test(r.text), r.text);
r = await call("diff", { since: 1 });
check("diff since revision 1 includes both edits", !r.isError && /post_1/.test(r.text) && /crate_base/.test(r.text), r.text);
r = await call("batch", { steps: [{ tool: "edit", args: { ops: [{ op: "delete", node: "crate_base" }] } }, { tool: "check", args: {} }] });
const batched = r.isError ? null : JSON.parse(r.text);
check("batch can run edit then check in one call", batched?.steps?.length === 2 && batched.steps.every((s) => s.ok === true), r.text.slice(0, 500));
check("no page errors", errors.length === 0, errors);

console.log(failed ? `\n${failed} check(s) failed` : "\nall checks passed");
await browser.close();
server.kill();
process.exit(failed ? 1 : 0);
