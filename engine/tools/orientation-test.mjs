// Fewer calls to get oriented: get_guide takes several topics and an operator selection in one call, snapshot takes cameras, and the server
// introduces itself with instructions. Spawns the MCP server with no page. bun tools/orientation-test.mjs
import { spawn } from "node:child_process";

const server = spawn("bun", ["tools/mcp/server.mjs"], { env: { ...process.env, VE_MCP_PORT: "8794" }, stdio: ["pipe", "pipe", "inherit"] });
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

const init = await rpc("initialize", { protocolVersion: "2024-11-05" });
const instructions = init.result?.instructions ?? "";
check("the server introduces itself with instructions", instructions.length > 200 && instructions.length < 2500, instructions.length);
for (const word of ["batch", "run", "start_from", "edit", "snapshot", "survey", "get_guide"]) check(`the instructions mention ${word}`, instructions.includes(word), instructions.slice(0, 300));
check("the instructions say to start from a template and to look with one snapshot call of several cameras", /start_from/.test(instructions) && /cameras/.test(instructions), instructions.slice(0, 400));

// get_guide: several topics at once
const one = await call("get_guide", { topic: "curves" });
const two = await call("get_guide", { topics: ["curves", "terrain"] });
check("get_guide topics returns every topic in one answer", !two.isError && two.text.includes(one.text.slice(0, 120)) && two.text.length > one.text.length + 400, two.text.slice(0, 200));
check("each topic in the answer is headed by its name", /curves/i.test(two.text) && /terrain/i.test(two.text) && /^#{1,3} .*curves/im.test(two.text), two.text.slice(0, 300));
const bad = await call("get_guide", { topics: ["curves", "nope_topic"] });
check("an unknown topic in the list is an error that lists the valid ones", bad.isError && /nope_topic/.test(bad.text) && /terrain/.test(bad.text), bad.text.slice(0, 300));
const core = await call("get_guide", {});
check("the core guide (no topic) still works and lists topics", !core.isError && /topic/i.test(core.text), core.text.slice(0, 200));

// get_guide + operators in one call needs the page for the operator list; without it the guide part must still come back
const both = await call("get_guide", { topics: ["curves"], operators: { family: "solid" } });
check("get_guide with an operators selection returns the guide part even without a page and says the operators need the editor", !both.isError && both.text.includes(one.text.slice(0, 120)) && /editor page|operators/i.test(both.text), both.text.slice(0, 300));
const schema = (await rpc("tools/list", {})).result.tools;
const guideTool = schema.find((t) => t.name === "get_guide");
check("get_guide's schema has topics and operators", guideTool && JSON.stringify(guideTool.inputSchema).includes("topics") && JSON.stringify(guideTool.inputSchema).includes("operators"), JSON.stringify(guideTool?.inputSchema));

// snapshot: cameras
const snap = schema.find((t) => t.name === "snapshot");
const props = JSON.stringify(snap?.inputSchema ?? {});
check("snapshot takes cameras (position, target, label) and a single camera", props.includes("cameras") && props.includes("camera") && props.includes("target") && props.includes("position"), props);
let r = await call("snapshot", { cameras: Array.from({ length: 7 }, () => ({ position: [0, 5, 5], target: [0, 0, 0] })) });
check("more than 6 cameras is refused with the limit", r.isError && /6/.test(r.text), r.text.slice(0, 300));
r = await call("snapshot", { cameras: [{ position: [0, 5], target: [0, 0, 0] }] });
check("a camera with a bad position is refused before asking the page", r.isError && /position/.test(r.text), r.text.slice(0, 300));
r = await call("snapshot", { camera: { position: [0, 5, 5], target: [0, 0, 0] }, views: ["front"] });
check("camera together with views is refused (pick one)", r.isError && /camera|views/.test(r.text), r.text.slice(0, 300));
r = await call("snapshot", { cameras: [{ position: [0, 5, 5], target: [0, 0, 0], label: "air" }] });
check("a valid camera request goes to the page (and fails only because none is connected)", r.isError && /editor page/i.test(r.text), r.text.slice(0, 300));

const total = schema.reduce((n, t) => n + JSON.stringify(t).length, 0);
check("all tool definitions still stay under 13,000 characters", total <= 13000, total);

console.log(failed ? `\n${failed} check(s) failed` : "\nall checks passed");
server.kill();
process.exit(failed ? 1 : 0);
