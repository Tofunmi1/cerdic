// tools/mcp/hints.mjs: short tips the server appends to a tool answer when a model's call pattern shows it is missing a cheaper way.
// bun tools/hints-test.mjs
import { spawn } from "node:child_process";
import { createHints } from "./mcp/hints.mjs";

let failed = 0;
const check = (name, ok, detail) => { console.log(`${ok ? "ok  " : "FAIL"} ${name}${ok ? "" : `: ${String(detail).slice(0, 500)}`}`); if (!ok) failed++; };

let h = createHints();
check("a first get_guide gives no tip", h.note("get_guide", { topic: "curves" }, false) === null, "");
let tip = h.note("get_guide", { topic: "terrain" }, false);
check("a second single-topic get_guide tips topics and operators in one call", typeof tip === "string" && tip.startsWith("Tip:") && /topics/.test(tip) && /operators/.test(tip), tip);
check("a tip is short", tip.length <= 260, tip.length);
check("the same tip does not repeat in a session", h.note("get_guide", { topic: "poly" }, false) === null, "");
h = createHints();
h.note("get_guide", { topics: ["core"] }, false);
tip = h.note("list_operators", {}, false);
check("list_operators after get_guide tips get_guide's operators option", /get_guide/.test(tip ?? "") && /operators/.test(tip ?? ""), tip);
h = createHints();
check("set_view alone gives no tip", h.note("set_view", { position: [0, 0, 0], target: [1, 1, 1] }, false) === null, "");
tip = h.note("snapshot", {}, false);
check("snapshot right after set_view tips cameras", /cameras/.test(tip ?? ""), tip);
h = createHints();
h.note("apply_text", { text: "a" }, false);
h.note("apply_text", { text: "b" }, false);
tip = h.note("apply_text", { text: "c" }, false);
check("three apply_text calls in a row tip batch or run", /batch|run/.test(tip ?? ""), tip);
h = createHints();
h.note("apply_text", { text: "a" }, false);
h.note("batch", { steps: [] }, false);
h.note("apply_text", { text: "b" }, false);
check("using batch resets the apply_text count", h.note("apply_text", { text: "c" }, false) === null, "");
h = createHints();
h.note("get_nodes", {}, false);
h.note("get_nodes", {}, false);
tip = h.note("get_nodes", {}, false);
check("a third get_nodes tips find or get_text", /find|get_text/.test(tip ?? ""), tip);
h = createHints();
h.note("apply_text", { text: "a" }, true);
h.note("apply_text", { text: "a" }, true);
tip = h.note("apply_text", { text: "a" }, true);
check("three failing apply_text calls in a row tip dry_run and the guide's compact topic", /dry_run/.test(tip ?? ""), tip);
h = createHints();
h.note("get_guide", { topic: "a" }, false);
h.note("get_guide", { topic: "b" }, false);
h.reset();
h.note("get_guide", { topic: "a" }, false);
check("reset starts a new session", /topics/.test(h.note("get_guide", { topic: "b" }, false) ?? ""), "");

// through the server: the second single-topic get_guide answer ends with the tip
const server = spawn("bun", ["tools/mcp/server.mjs"], { env: { ...process.env, VE_MCP_PORT: "8799" }, stdio: ["pipe", "pipe", "inherit"] });
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
const callTool = async (name, args) => { const r = await rpc("tools/call", { name, arguments: args }); return (r.result?.content ?? []).map((c) => c.text ?? "").join("\n"); };
await rpc("initialize", { protocolVersion: "2024-11-05" });
const first = await callTool("get_guide", { topic: "curves" });
const second = await callTool("get_guide", { topic: "terrain" });
check("the server appends the tip to the second answer only", !first.includes("Tip:") && /Tip:.*topics/.test(second.split("\n").slice(-3).join("\n")), second.slice(-300));
const third = await callTool("get_guide", { topic: "poly" });
check("and not again", !third.includes("Tip:"), third.slice(-200));
const batched = await callTool("batch", { steps: [{ tool: "get_guide", args: { topic: "curves" } }] });
check("a batch answer stays valid JSON (tips never break structured answers)", (() => { try { JSON.parse(batched); return true; } catch { return false; } })(), batched.slice(-200));
server.kill();

console.log(failed ? `\n${failed} check(s) failed` : "\nall checks passed");
process.exit(failed ? 1 : 0);
