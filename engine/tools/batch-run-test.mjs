// ADR 0069: the `batch` and `run` tools of the MCP server. Spawns the server (no editor page connected), speaks MCP over stdio and uses
// the tools that need no page (get_guide) plus ones that fail without a page (get_text) to check ordering, stopping, limits and errors.
//   bun tools/batch-run-test.mjs
import { spawn } from "node:child_process";

const server = spawn("bun", ["tools/mcp/server.mjs"], { env: { ...process.env, VE_MCP_PORT: "8791", VE_RUN_TIMEOUT_MS: "1500" }, stdio: ["pipe", "pipe", "inherit"] });
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
    if (!line) continue;
    const message = JSON.parse(line);
    waiting.get(message.id)?.(message);
    waiting.delete(message.id);
  }
});
const rpc = (method, params) => new Promise((resolve) => {
  const id = ++nextId;
  waiting.set(id, resolve);
  server.stdin.write(JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n");
});
const call = async (name, args) => {
  const reply = await rpc("tools/call", { name, arguments: args });
  const text = (reply.result?.content ?? []).filter((c) => c.type === "text").map((c) => c.text).join("\n");
  return { text, isError: reply.result?.isError === true, content: reply.result?.content ?? [] };
};
let failed = 0;
const check = (name, condition, detail) => {
  console.log(`${condition ? "ok  " : "FAIL"} ${name}${condition ? "" : `: ${String(detail).slice(0, 600)}`}`);
  if (!condition) failed++;
};

await rpc("initialize", { protocolVersion: "2024-11-05" });
const tools = (await rpc("tools/list", {})).result.tools;
check("batch and run are listed", ["batch", "run"].every((n) => tools.some((t) => t.name === n)), tools.map((t) => t.name));
const describe = (n) => tools.find((t) => t.name === n)?.description ?? "";
check("their descriptions are short", describe("batch").length < 900 && describe("run").length < 1500, [describe("batch").length, describe("run").length]);
check("the world tool description no longer lists every runtime method", describe("world").length < 1200, describe("world").length);
const world = await call("get_guide", { topic: "world_api" });
check("get_guide world_api carries the runtime methods", /world\.bounds/.test(world.text) && /ground\.profile/.test(world.text) && /world\.batch/.test(world.text), world.text.slice(0, 200));

// batch: ordered steps, one entry each
let r = await call("batch", { steps: [{ tool: "get_guide", args: { topic: "curves" } }, { tool: "get_guide", args: {} }] });
let body = JSON.parse(r.text);
check("batch answers one entry per step, in order", Array.isArray(body.steps) && body.steps.length === 2 && body.steps.every((s) => s.ok === true), r.text.slice(0, 300));
check("each entry carries the step's text", /curve/i.test(body.steps[0].text ?? "") && (body.steps[1].text ?? "").length > 200, JSON.stringify(body.steps.map((s) => (s.text ?? "").length)));

// stops at the first error by default and says which step
r = await call("batch", { steps: [{ tool: "get_guide", args: { topic: "curves" } }, { tool: "get_text", args: {} }, { tool: "get_guide", args: {} }] });
body = JSON.parse(r.text);
check("batch stops at the first failing step", body.steps.length === 2 && body.steps[1].ok === false && body.stopped_at === 1, r.text.slice(0, 400));
check("a failing step's entry has the reason", /editor page/i.test(body.steps[1].error ?? ""), r.text.slice(0, 400));
r = await call("batch", { stop_on_error: false, steps: [{ tool: "get_text", args: {} }, { tool: "get_guide", args: { topic: "curves" } }] });
body = JSON.parse(r.text);
check("stop_on_error false runs every step", body.steps.length === 2 && body.steps[0].ok === false && body.steps[1].ok === true, r.text.slice(0, 400));

// limits and refusals
r = await call("batch", { steps: Array.from({ length: 33 }, () => ({ tool: "get_guide", args: {} })) });
check("more than 32 steps is refused", r.isError && /32/.test(r.text), r.text.slice(0, 300));
r = await call("batch", { steps: [{ tool: "batch", args: { steps: [] } }] });
check("batch inside batch is refused", r.isError || /not.*inside/i.test(r.text), r.text.slice(0, 300));
r = await call("batch", { steps: [{ tool: "no_such_tool", args: {} }] });
body = r.isError ? null : JSON.parse(r.text);
check("an unknown tool is a failing step, not a crash", r.isError || (body.steps[0].ok === false && /unknown tool/.test(body.steps[0].error)), r.text.slice(0, 300));
r = await call("batch", { max_chars: 100, steps: [{ tool: "get_guide", args: {} }] });
body = JSON.parse(r.text);
check("max_chars cuts a step's text and marks it", body.steps[0].text.length <= 140 && body.steps[0].truncated === true, JSON.stringify(body.steps[0]).slice(0, 300));

// run: code mode
r = await call("run", { code: "const g = await tools.get_guide({ topic: 'curves' }); return { n: g.length > 100, kind: typeof g };" });
body = JSON.parse(r.text);
check("run calls a tool and returns a value", body.result?.n === true && body.result?.kind === "string", r.text.slice(0, 300));
r = await call("run", { code: "const out = []; for (const t of ['curves', 'terrain', 'poly']) { out.push((await tools.get_guide({ topic: t })).length > 50); } log('looped', out.length); return out;" });
body = JSON.parse(r.text);
check("run loops over tool calls in one request and returns logs", JSON.stringify(body.result) === "[true,true,true]" && /looped 3/.test(JSON.stringify(body.logs)), r.text.slice(0, 300));
r = await call("run", { code: "return await tools.get_text({});" });
check("a tool error inside run is thrown to the code and reported", r.isError && /editor page/i.test(r.text), r.text.slice(0, 300));
r = await call("run", { code: "try { await tools.get_text({}); return 'no'; } catch (e) { return 'caught: ' + e.message; }" });
body = JSON.parse(r.text);
check("the code can catch a tool error", /^caught:/.test(body.result ?? ""), r.text.slice(0, 300));
r = await call("run", { code: "return 'x'.repeat(20000);" });
body = JSON.parse(r.text);
check("a long result is cut to 8,000 characters and marked", JSON.stringify(body.result).length <= 8100 && body.truncated === true, r.text.length);
r = await call("run", { code: "for (let i = 0; i < 300; i++) await tools.get_guide({}); return 1;" });
check("more than 200 tool calls in one run is refused", r.isError && /200/.test(r.text), r.text.slice(0, 300));
r = await call("run", { code: "return typeof process + typeof require + typeof fetch + typeof Bun + typeof globalThis.process;" });
body = JSON.parse(r.text);
check("the code sees no process, require, fetch or Bun", body.result === "undefinedundefinedundefinedundefinedundefined", r.text.slice(0, 300));
r = await call("run", { code: "await tools.batch({ steps: [] }); return 1;" });
check("run may not call batch or run", r.isError, r.text.slice(0, 300));
r = await call("run", { code: "while (true) {}" });
check("an endless loop is stopped with a clear error", r.isError && /time|seconds/i.test(r.text), r.text.slice(0, 300));
r = await call("run", { code: "return (" });
check("a syntax error is reported, not a crash", r.isError && /syntax|unexpected/i.test(r.text), r.text.slice(0, 300));

console.log(failed ? `\n${failed} check(s) failed` : "\nall checks passed");
server.kill();
process.exit(failed ? 1 : 0);
