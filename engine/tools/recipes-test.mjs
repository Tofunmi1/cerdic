// ADR 0069 follow-up: the `start_from` (working skeletons to edit) and `recipes` (save and reuse a group) tools of the MCP server.
// Spawns the server with no page and a temporary recipes folder. bun tools/recipes-test.mjs
import { spawn } from "node:child_process";
import { mkdtempSync, rmSync, readdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const dir = mkdtempSync(join(tmpdir(), "ve-recipes-"));
let server;
let buffer = "";
const waiting = new Map();
let nextId = 0;
function start() {
  server = spawn("bun", ["tools/mcp/server.mjs"], { env: { ...process.env, VE_MCP_PORT: "8793", VE_RECIPES_DIR: dir }, stdio: ["pipe", "pipe", "inherit"] });
  buffer = "";
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
}
const rpc = (method, params) => new Promise((resolve) => { const id = ++nextId; waiting.set(id, resolve); server.stdin.write(JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n"); });
const call = async (name, args) => { const r = await rpc("tools/call", { name, arguments: args }); return { text: (r.result?.content ?? []).map((c) => c.text ?? "").join("\n"), isError: r.result?.isError === true }; };
let failed = 0;
const check = (name, ok, detail) => { console.log(`${ok ? "ok  " : "FAIL"} ${name}${ok ? "" : `: ${String(detail).slice(0, 500)}`}`); if (!ok) failed++; };

start();
await rpc("initialize", { protocolVersion: "2024-11-05" });
const tools = (await rpc("tools/list", {})).result.tools;
for (const name of ["start_from", "recipes"]) check(`${name} is a tool with a short definition`, tools.some((t) => t.name === name && JSON.stringify(t).length <= 1100), tools.map((t) => t.name));
const total = tools.reduce((n, t) => n + JSON.stringify(t).length, 0);
check("all tool definitions together stay under 13,000 characters", total <= 13000, total);
check("tool order is the same on every call (cache-stable)", JSON.stringify((await rpc("tools/list", {})).result.tools.map((t) => t.name)) === JSON.stringify(tools.map((t) => t.name)), "");

// start_from
let r = await call("start_from", {});
let list = r.isError ? null : JSON.parse(r.text);
const names = (list?.templates ?? []).map((t) => t.name);
check("start_from lists templates with an about line each", names.length >= 4 && list.templates.every((t) => typeof t.about === "string" && t.about.length > 20), r.text.slice(0, 400));
for (const expected of ["rolling-course", "creature-game", "chase-arena", "district"]) check(`template ${expected} exists`, names.includes(expected), names);
for (const name of names) {
  r = await call("start_from", { template: name });
  const body = r.isError ? null : JSON.parse(r.text);
  check(`template ${name} gives its text and what to change`, body && typeof body.text === "string" && body.text.length > 500 && /output /.test(body.text) && Array.isArray(body.change) && body.change.length >= 2, r.text.slice(0, 300));
}
r = await call("start_from", { template: "no-such" });
check("an unknown template is an error that lists the names", r.isError && names.every((n) => r.text.includes(n)), r.text.slice(0, 300));
r = await call("start_from", { template: names[0], apply: true });
check("apply:true needs the editor page and says so", r.isError && /editor page/i.test(r.text), r.text.slice(0, 300));

// recipes
const GROUP = `group crate_stack
  box = solid.box(size=[0.5, 0.5, 0.5])
  top = xform.move(box, by=[0, 0.5, 0])
  both = solid.union(box, top)
  expose width = box.size
  expose lift = top.by
  result both
end`;
r = await call("recipes", { action: "list" });
check("recipes starts empty", !r.isError && JSON.parse(r.text).recipes.length === 0, r.text);
r = await call("recipes", { action: "save", name: "crate_stack", text: GROUP, about: "two crates, one on the other" });
check("save stores a recipe", !r.isError, r.text);
r = await call("recipes", { action: "list" });
const saved = JSON.parse(r.text).recipes;
check("list shows the name, about and exposed parameters", saved.length === 1 && saved[0].name === "crate_stack" && saved[0].params.includes("width") && saved[0].params.includes("lift"), r.text);
r = await call("recipes", { action: "show", name: "crate_stack" });
check("show returns the source and a call line to paste", !r.isError && r.text.includes("group crate_stack") && /crate_stack\(.*width=/.test(r.text), r.text.slice(0, 400));
check("the recipe is a file in the recipes folder", readdirSync(dir).some((f) => f.includes("crate_stack")), readdirSync(dir));
server.kill();
start();
await rpc("initialize", { protocolVersion: "2024-11-05" });
r = await call("recipes", { action: "list" });
check("recipes survive a server restart", JSON.parse(r.text).recipes.some((x) => x.name === "crate_stack"), r.text);
for (const [label, args] of [
  ["a name with capitals is refused", { action: "save", name: "Bad Name", text: GROUP }],
  ["text that is not one group is refused", { action: "save", name: "plain", text: "a = solid.box(size=[1,1,1])" }],
  ["a group whose name differs from the recipe name is refused", { action: "save", name: "other", text: GROUP }],
  ["a name that is a built-in kit is refused", { action: "save", name: "door", text: GROUP.replace("crate_stack", "door") }],
  ["show of an unknown recipe is an error", { action: "show", name: "nope" }],
]) { r = await call("recipes", args); check(label, r.isError, r.text); }
r = await call("recipes", { action: "delete", name: "crate_stack" });
check("delete removes it", !r.isError && JSON.parse((await call("recipes", { action: "list" })).text).recipes.length === 0, r.text);
const guide = (await call("get_guide", { topic: "recipes" })).text;
check("a guide topic explains start_from and recipes and run examples", /start_from/.test(guide) && /recipes/.test(guide) && /tools\./.test(guide), guide.slice(0, 300));

console.log(failed ? `\n${failed} check(s) failed` : "\nall checks passed");
server.kill();
rmSync(dir, { recursive: true, force: true });
process.exit(failed ? 1 : 0);
