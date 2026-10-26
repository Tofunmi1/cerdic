// ADR 0069/0061: the tool surface a model carries in every turn stays small, and the label-based `edit` and `diff` tools exist.
// Spawns the MCP server (no editor page connected). bun tools/tool-surface-test.mjs
import { spawn } from "node:child_process";

const server = spawn("bun", ["tools/mcp/server.mjs"], { env: { ...process.env, VE_MCP_PORT: "8792" }, stdio: ["pipe", "pipe", "inherit"] });
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
const check = (name, ok, detail) => { console.log(`${ok ? "ok  " : "FAIL"} ${name}${ok ? "" : `: ${String(detail).slice(0, 500)}`}`); if (!ok) failed++; };

await rpc("initialize", { protocolVersion: "2024-11-05" });
const tools = (await rpc("tools/list", {})).result.tools;
const sizes = tools.map((t) => [t.name, JSON.stringify(t).length]);
const total = sizes.reduce((n, [, s]) => n + s, 0);
check("every tool definition is under 1,300 characters", sizes.every(([, s]) => s <= 1300), JSON.stringify(sizes.filter(([, s]) => s > 1300)));
check("all tool definitions together are under 13,000 characters", total <= 13000, total);
for (const name of ["edit", "diff", "batch", "run", "survey"]) check(`${name} is a tool`, tools.some((t) => t.name === name), tools.map((t) => t.name));
const edit = tools.find((t) => t.name === "edit");
check("edit takes a list of operations", JSON.stringify(edit?.inputSchema ?? {}).includes("ops"), JSON.stringify(edit?.inputSchema));
// what moved out of the descriptions is in the guide
for (const topic of ["script_api", "snapshot_modes", "survey"]) {
  const r = await call("get_guide", { topic });
  check(`get_guide ${topic} has the detail`, !r.isError && r.text.length > 800, r.text.slice(0, 200));
}
const script = (await call("get_guide", { topic: "script_api" })).text;
check("script_api lists the script functions", ["world.parts", "world.batch", "on(\"frame\"", "every(", "random()"].every((s) => script.includes(s)), script.slice(0, 300));
const snap = (await call("get_guide", { topic: "snapshot_modes" })).text;
check("snapshot_modes explains times, states, views and focus", ["times", "states", "views", "focus"].every((s) => snap.includes(s)), snap.slice(0, 300));
const survey = (await call("get_guide", { topic: "survey" })).text;
check("survey explains coverage, snapshot arguments, tour limits and skipped instances", ["reachable", "snapshot_args", "skipped_instances", "two sheets", "1 to 12"].every((s) => survey.includes(s)), survey);
const surveyTool = tools.find((t) => t.name === "survey");
check("survey exposes auto, walk and orbit modes", JSON.stringify(surveyTool?.inputSchema.properties.mode?.enum) === JSON.stringify(["auto", "walk", "orbit"]), surveyTool);
check("survey exposes the standing camera options", ["eye_height", "spacing", "max_views", "yaws", "target_coverage"].every((key) => key in (surveyTool?.inputSchema.properties ?? {})), surveyTool);
check("snapshot exposes survey tours", Boolean(tools.find((t) => t.name === "snapshot")?.inputSchema.properties.tour), tools);
check("snapshot tours expose survey modes", JSON.stringify(tools.find((t) => t.name === "snapshot")?.inputSchema.properties.tour.properties.mode.enum) === JSON.stringify(["auto", "walk", "orbit"]), tools);
for (const [args, range] of [
  [{ eye_height: 0 }, "0.1 to 5"], [{ spacing: -1 }, "0.25 to 100"],
  [{ max_views: 65 }, "1 to 64"], [{ yaws: 1.5 }, "1 to 32"],
  [{ target_coverage: 2 }, "0 to 1"],
  [{ mode: "invalid" }, "auto, walk, orbit"],
]) {
  const r = await call("survey", args);
  check(`survey validates ${Object.keys(args)[0]} before asking the page`, r.isError && r.text.includes(range) && !r.text.includes("No editor page"), r.text);
}
let tour = await call("snapshot", { tour: { max_views: 13 } });
check("tour validates its two-sheet limit before asking the page", tour.isError && tour.text.includes("1 to 12"), tour.text);
tour = await call("snapshot", { tour: true, cameras: [] });
check("tour rejects competing capture modes before asking the page", tour.isError && /tour/.test(tour.text) && /cameras/.test(tour.text), tour.text);
for (const [name, args] of [["survey", { spacing: 3, max_views: 8 }], ["snapshot", { tour: true }], ["survey", { mode: "orbit" }], ["snapshot", { tour: { mode: "walk" } }]]) {
  const r = await call(name, args);
  check(`valid ${name} reaches the page relay`, r.isError && /No editor page/.test(r.text), r.text);
}
// edit and diff refuse cleanly without a page and validate arguments before asking it
let r = await call("edit", { ops: [] });
check("edit with no operations is refused with the shapes it accepts", r.isError && /set|rename|delete|replace/.test(r.text), r.text);
r = await call("edit", { ops: [{ op: "explode", node: "a" }] });
check("edit names the operations it knows when given an unknown one", r.isError && /set/.test(r.text) && /explode/.test(r.text), r.text);
r = await call("diff", { since: "not a number" });
check("diff validates its argument", r.isError, r.text);

console.log(failed ? `\n${failed} check(s) failed` : "\nall checks passed");
server.kill();
process.exit(failed ? 1 : 0);
