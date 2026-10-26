// Scripts saved in the document (`script.js` nodes, ADR 0065): node tools/script-persist-test.mjs
// A world carries a script node: the page starts it when the world loads, restarts it when its source changes, leaves it alone when an
// unrelated edit re-evaluates, stops it when the node is disabled or removed, and never touches scripts started with the `script` tool.
// (Document text is additive, so edits go through typed commands: set_param and remove_node.)
import { chromium } from "../web/node_modules/playwright/index.mjs";
import { readFileSync } from "node:fs";
const base = readFileSync(new URL("../web/dist-bevy-gpu/worlds/world_api_try.txt", import.meta.url), "utf8");
const browser = await chromium.launch({ channel: "chromium", args: ["--enable-unsafe-webgpu", "--enable-features=Metal,WebGPU", "--ignore-gpu-blocklist"] });
const page = await browser.newPage({ viewport: { width: 900, height: 600 } });
page.on("pageerror", (e) => console.log("PAGEERR", String(e).slice(0, 300)));
await page.goto("http://localhost:8801/?viewer=1&eval=1&scene=stage&quality=low");
await page.waitForFunction(() => window.__ve?.ready && window.__ve.scripts, null, { timeout: 120000 });
await page.evaluate(async () => { window.__rt = await import("./ve_runtime.js"); });
let failed = 0;
const check = (name, condition, detail) => { console.log(`${condition ? "ok  " : "FAIL"} ${name}${condition ? "" : `: ${JSON.stringify(detail).slice(0, 300)}`}`); if (!condition) failed++; };
const settle = async () => { await page.waitForTimeout(400); await page.waitForFunction(() => window.__ve.evaluation.state.worker === "idle", null, { timeout: 120000 }); await page.waitForTimeout(900); };
const addText = async (text) => { await page.evaluate((text) => window.__ve.doc.sendText(text, "test"), text); await settle(); };
const command = async (cmd) => {
  const outcome = await page.evaluate((cmd) => window.__ve.doc.send([cmd], "test"), cmd);
  await settle();
  return outcome;
};
const nodeId = (label) => page.evaluate((label) => window.__ve.doc.state().document.nodes.find((n) => n.label === label)?.id, label);
const setParam = async (label, param, value) => command({ set_param: { node: await nodeId(label), param, value } });
const removeNode = async (label) => command({ remove_node: { node: await nodeId(label) } });
const list = () => page.evaluate(() => window.__ve.scripts.list());
const logs = (name) => page.evaluate((name) => window.__ve.scripts.logs(name), name);
const crateY = () => page.evaluate(async () => {
  const id = window.__rt.world_call("world.get_components", JSON.stringify({ entity: "crate_middle", components: ["Transform"] }));
  for (let i = 0; i < 100; i++) { const a = window.__rt.world_take(id); if (a) return JSON.parse(a).result?.components?.Transform?.translation?.[1]; await new Promise((r) => setTimeout(r, 30)); }
});
const node = (label, source, name = label) => `${label} = script.js@1(source="${source.replaceAll('"', '\\"')}", name="${name}")\n`;
const bobSource = (height) => `log('bob ${height}'); on('frame', () => world.set('crate_middle', 'Transform', { translation: [0, ${height}, 0], rotation: [0, 0, 0, 1], scale: [1, 1, 1] }));`;

await addText(base + node("bob", bobSource(0.5)));
let scripts = await list();
check("loading a world with a script node starts the script", scripts.some((s) => s.name === "bob" && s.owner === "document"), scripts);
await page.waitForTimeout(600);
check("the script acts on the world (the crate is at 0.5)", Math.abs((await crateY()) - 0.5) < 0.01, await crateY());
const framesBefore = (await list()).find((s) => s.name === "bob").frames;

// An unrelated edit re-evaluates; the running script must not restart.
await addText("extra = solid.box@1(size=[0.1, 0.1, 0.1])\nextra_move = xform.move@1(extra, by=[3, 0, 3])\noutput extra = extra_move\n");
const framesAfter = (await list()).find((s) => s.name === "bob")?.frames ?? -1;
check("an unrelated edit does not restart the script (its frame count keeps growing)", framesAfter >= framesBefore, { framesBefore, framesAfter });

// A changed source restarts it.
await setParam("bob", "source", bobSource(1.2));
await page.waitForTimeout(600);
check("a changed source restarts the script with the new behaviour (the crate is at 1.2)", Math.abs((await crateY()) - 1.2) < 0.01, await crateY());
check("the restart logged the new text", (await logs("bob")).logs.includes("bob 1.2"), await logs("bob"));

// A script started with the tool is not touched by document changes.
await page.evaluate(() => window.__ve.scripts.start("tool_made", "on('frame', () => {});"));
await setParam("bob", "enabled", false);
scripts = await list();
check("a disabled script node stops the script", !scripts.some((s) => s.name === "bob"), scripts);
check("a script started with the tool is left alone", scripts.some((s) => s.name === "tool_made" && s.owner === "tool"), scripts);

await setParam("bob", "enabled", true);
check("enabling it again starts it", (await list()).some((s) => s.name === "bob"), await list());
await removeNode("bob");
check("removing the node stops the script", !(await list()).some((s) => s.name === "bob"), await list());

// Two scripts, one removed.
await addText(node("bob", bobSource(0.5)) + node("other", "log('other')"));
const two = (await list()).filter((s) => s.owner === "document").map((s) => s.name).sort();
check("two script nodes run two scripts", two.join() === "bob,other", two);
await removeNode("other");
const left = (await list()).filter((s) => s.owner === "document").map((s) => s.name);
check("removing one leaves the other", left.join() === "bob", left);
// Undo brings it back.
await page.evaluate(() => window.__ve.doc.undo());
await settle();
const back = (await list()).filter((s) => s.owner === "document").map((s) => s.name).sort();
check("undo of the removal starts the script again", back.join() === "bob,other", back);
console.log(failed ? `\n${failed} check(s) failed` : "\nall checks passed");
await browser.close();
process.exit(failed ? 1 : 0);
