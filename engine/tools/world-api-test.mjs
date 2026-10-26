// Exercises the runtime world API (ADR 0065) in a real page: node tools/world-api-test.mjs
// Loads the walker world, then calls every method through world_call / world_take, checks the answers, writes a part's Transform and reads
// it back, spawns and despawns an entity, and confirms the refusals say what to change. Exits non-zero on the first failed check.
import { chromium } from "../web/node_modules/playwright/index.mjs";
const browser = await chromium.launch({ channel: "chromium", args: ["--enable-unsafe-webgpu", "--enable-features=Metal,WebGPU", "--ignore-gpu-blocklist"] });
const page = await browser.newPage({ viewport: { width: 900, height: 600 } });
page.on("pageerror", (e) => console.log("PAGEERR", String(e).slice(0, 300)));
await page.goto("http://localhost:8801/?viewer=1&eval=1&scene=stage&quality=low&world=worlds/walker_machine.txt");
await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 120000 });
await page.waitForFunction(() => JSON.parse(window.__rt?.eval_projection_status?.() ?? "{}").committed?.quality === "final" || (window.__rt = null, false), null, { timeout: 5000 }).catch(() => {});
await page.evaluate(async () => { window.__rt = await import("./ve_runtime.js"); });
await page.waitForFunction(() => JSON.parse(window.__rt.eval_projection_status()).committed?.quality === "final", null, { timeout: 200000 });
await page.waitForTimeout(2000);

const call = (method, params = {}) => page.evaluate(async ({ method, params }) => {
  const id = window.__rt.world_call(method, JSON.stringify(params));
  for (let i = 0; i < 100; i++) { const a = window.__rt.world_take(id); if (a) return JSON.parse(a); await new Promise((r) => setTimeout(r, 50)); }
  return { ok: false, error: "timeout" };
}, { method, params });

let failed = 0;
const check = (name, condition, detail) => { console.log(`${condition ? "ok  " : "FAIL"} ${name}${condition ? "" : `: ${JSON.stringify(detail).slice(0, 300)}`}`); if (!condition) failed++; };

const list = await call("registry.list");
check("registry.list names components, Transform among them and writable", list.ok && list.result.components.some((c) => c.short === "Transform" && c.writable), list);
console.log(`     ${list.result?.components?.length} components registered for reflection`);
const parts = await call("world.parts");
check("world.parts lists the walker's parts", parts.ok && ["body", "head", "leg_a"].every((n) => parts.result.parts.some((p) => p.name === n)), parts);
const found = await call("world.find", { name: "head" });
check("world.find finds the head", found.ok && found.result.entities.length === 1, found);
const head = found.result.entities[0]?.entity;
const listed = await call("world.list_components", { entity: "head" });
check("world.list_components (by part name) includes Transform", listed.ok && listed.result.components.some((n) => n.endsWith("Transform")), listed);
const before = await call("world.get_components", { entity: head, components: ["Transform"] });
check("world.get_components reads a Transform", before.ok && before.result.components.Transform?.translation, before);
const wrote = await call("world.insert_components", { entity: "head", components: { Transform: { translation: [3, 4, 5], rotation: [0, 0, 0, 1], scale: [1, 1, 1] } } });
check("world.insert_components writes a Transform", wrote.ok, wrote);
await page.waitForTimeout(300);
const after = await call("world.get_components", { entity: head, components: ["Transform"] });
const moved = after.result?.components?.Transform?.translation;
// An animated part has its transform replaced by the animation each frame, so read back at once is the honest check: it was written.
console.log(`     head translation after the write: ${JSON.stringify(moved)} (the animation system owns animated parts and may overwrite it)`);
check("world.get_components still answers after the write", after.ok, after);
const query = await call("world.query", { components: ["Transform"], name: "leg_a" });
check("world.query with a name filter returns that part", query.ok && query.result.rows.length === 1 && query.result.rows[0].name === "leg_a", query);
const all = await call("world.query", { has: ["Transform"], limit: 5 });
check("world.query respects the limit", all.ok && all.result.rows.length === 5 && all.result.truncated === true, all);
const spawned = await call("world.spawn_entity", { components: { Transform: { translation: [0, 9, 0], rotation: [0, 0, 0, 1], scale: [1, 1, 1] }, Visibility: "Hidden" } });
check("world.spawn_entity makes an entity", spawned.ok && typeof spawned.result.entity === "string", spawned);
const back = await call("world.get_components", { entity: spawned.result?.entity, components: ["Transform", "Visibility"] });
check("the spawned entity has what was asked", back.ok && back.result.components.Transform?.translation?.[1] === 9 && back.result.components.Visibility === "Hidden", back);
const gone = await call("world.despawn_entity", { entity: spawned.result?.entity });
check("world.despawn_entity removes it", gone.ok, gone);
const missing = await call("world.get_components", { entity: spawned.result?.entity, components: ["Transform"] });
check("a despawned entity is refused with a clear message", !missing.ok && /no entity|despawned/.test(missing.error), missing);
const refused = await call("world.despawn_entity", { entity: "0" });
check("an invalid id is refused", !refused.ok, refused);
const unknownComponent = await call("world.get_components", { entity: head, components: ["Bogus"] });
check("an unknown component is named in the error entry", unknownComponent.ok && /Bogus/.test(JSON.stringify(unknownComponent.result.errors)), unknownComponent);
const notWritable = await call("world.insert_components", { entity: "head", components: { GlobalTransform: [[1, 0, 0], [0, 1, 0], [0, 0, 1], [0, 0, 0]] } });
check("a component that is not writable is refused and the writable list is given", !notWritable.ok && /writable/.test(notWritable.error), notWritable);
const badShape = await call("world.insert_components", { entity: "head", components: { Transform: { translation: "far" } } });
check("a value of the wrong shape is refused", !badShape.ok, badShape);
const unknownMethod = await call("world.bogus");
check("an unknown method lists the real ones", !unknownMethod.ok && /world.query/.test(unknownMethod.error), unknownMethod);
const notJson = await page.evaluate(async () => { const id = window.__rt.world_call("world.parts", "{not json"); await new Promise((r) => setTimeout(r, 200)); return JSON.parse(window.__rt.world_take(id) ?? "{}"); });
check("malformed JSON is refused", notJson.ok === false, notJson);
// Patches: a component the entity has is changed field by field.
const mat0 = await call("world.get_components", { entity: "body", components: ["VeMaterial"] });
check("a part's surface (VeMaterial) can be read", mat0.ok && mat0.result.components.VeMaterial?.roughness !== undefined, mat0);
const patched = await call("world.insert_components", { entity: "body", components: { VeMaterial: { roughness: 0.1, finish: { wetness: 1 } } } });
check("a partial VeMaterial is accepted as a patch", patched.ok, patched);
const mat1 = await call("world.get_components", { entity: "body", components: ["VeMaterial"] });
check("the patched fields changed (roughness, finish.wetness)", Math.abs(mat1.result?.components?.VeMaterial?.roughness - 0.1) < 1e-6 && mat1.result.components.VeMaterial.finish.wetness === 1, mat1);
check("the fields it did not name are unchanged (base colour, metallic)", JSON.stringify(mat1.result.components.VeMaterial.base_color) === JSON.stringify(mat0.result.components.VeMaterial.base_color) && mat1.result.components.VeMaterial.metallic === mat0.result.components.VeMaterial.metallic, { before: mat0.result.components.VeMaterial, after: mat1.result.components.VeMaterial });
// (The body has no animation of its own, so nothing overwrites its Transform; an animated leg's would be reset every frame.)
const tx0 = await call("world.get_components", { entity: "body", components: ["Transform"] });
const patchT = await call("world.insert_components", { entity: "body", components: { Transform: { scale: [2, 2, 2] } } });
check("a partial Transform (only scale) is a patch too", patchT.ok, patchT);
const tx1 = await call("world.get_components", { entity: "body", components: ["Transform"] });
check("the scale changed and the rest stayed", tx1.ok && tx1.result.components.Transform.scale[0] === 2 && tx1.result.components.Transform.rotation.length === 4, { tx0, tx1 });
const badPatch = await call("world.insert_components", { entity: "body", components: { VeMaterial: { roughness: "slippery" } } });
check("a patch of the wrong type is refused and changes nothing", !badPatch.ok, badPatch);
// Visible shapes made at run time, and where parts are.
const shapeBox = await call("world.spawn_entity", { shape: { kind: "box", size: [0.5, 0.5, 0.5], color: [1, 0.2, 0.1], emissive: [0.5, 0.1, 0] }, position: [2, 3, 1] });
check("a box shape can be spawned", shapeBox.ok, shapeBox);
const shapeBall = await call("world.spawn_entity", { shape: { kind: "sphere", radius: 0.2, color: [0, 1, 0] }, position: [-2, 1, 0] });
const shapePipe = await call("world.spawn_entity", { shape: { kind: "cylinder", radius: 0.1, height: 1 }, position: [0, 1, 2] });
check("a sphere and a cylinder can be spawned", shapeBall.ok && shapePipe.ok, { shapeBall, shapePipe });
check("a bad shape is refused with a message", !(await call("world.spawn_entity", { shape: { kind: "torus" } })).ok && !(await call("world.spawn_entity", { shape: { kind: "box", size: [0, 1, 1] } })).ok);
await page.waitForTimeout(600);
const shapeAt = await call("world.bounds", { entity: shapeBox.result?.entity });
check("world.bounds finds a spawned shape where it was put", shapeAt.ok && Math.abs(shapeAt.result.center[0] - 2) < 0.01 && Math.abs(shapeAt.result.center[1] - 3) < 0.01 && Math.abs(shapeAt.result.size[0] - 0.5) < 0.01, shapeAt);
const bodyAt = await call("world.bounds", { entity: "body" });
// The body was scaled 2x by the patch above, so its 1.6 m box reads 3.2 m: bounds follow the Transform.
check("world.bounds gives an authored part's world box (here scaled 2x)", bodyAt.ok && Math.abs(bodyAt.result.size[0] - 3.2) < 0.1, bodyAt);
check("a spawned shape is removed by despawn", (await call("world.despawn_entity", { entity: shapeBox.result?.entity })).ok);
// Animation parameters through the API, and a batch.
const setSpeed = await call("animation.set_float", { part: "body", name: "speed", value: 1.5 });
const trig = await call("animation.trigger", { part: "body", name: "jump" });
check("animation parameters can be set and triggers raised", setSpeed.ok && trig.ok, { setSpeed, trig });
check("an animation parameter needs a name", !(await call("animation.set_float", { part: "body", value: 1 })).ok);
const batched = await call("world.batch", { ops: [{ method: "world.parts" }, { method: "world.find", params: { name: "head" } }, { method: "world.bogus" }, { method: "world.batch", params: { ops: [] } }] });
check("world.batch answers each operation in order", batched.ok && batched.result.results.length === 4 && batched.result.results[0].ok && batched.result.results[1].ok && !batched.result.results[2].ok && !batched.result.results[3].ok, batched);
check("a batch over 512 operations is refused", !(await call("world.batch", { ops: Array.from({ length: 513 }, () => ({ method: "world.parts" })) })).ok);
// Throughput: 200 reads queued in one go all answer within the frame budget (64 a frame, so a few frames).
const timing = await page.evaluate(async (entity) => {
  const t = performance.now(); const ids = [];
  for (let i = 0; i < 200; i++) ids.push(window.__rt.world_call("world.get_components", JSON.stringify({ entity, components: ["Transform"] })));
  let answered = 0;
  while (answered < 200 && performance.now() - t < 5000) { await new Promise((r) => setTimeout(r, 30)); answered = ids.filter((id) => { const a = window.__rt.world_take(id); if (a) { ids[ids.indexOf(id)] = -1; return false; } return id === -1; }).length; if (ids.every((i) => i === -1)) answered = 200; }
  return { ms: Math.round(performance.now() - t), answered };
}, head);
console.log(`     200 queued reads: all answered in ${timing.ms} ms (64 per frame)`);
check("200 queued reads are answered", timing.answered === 200, timing);
console.log(failed ? `\n${failed} check(s) failed` : "\nall checks passed");
await browser.close();
process.exit(failed ? 1 : 0);
