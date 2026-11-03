// A skinned character (body, clothes, eyes skinned to a skeleton) can be a game creature: node tools/skinned-model-live-test.mjs [world]
// The kind's part is the skeleton root; the game must draw its own skinned copy per entity (not leave the authored body standing
// still), the copy must follow the entity as it chases the hero, and its bones must be moved by the creature's clips.
import { chromium } from "../web/node_modules/playwright/index.mjs";
const world = process.argv[2] ?? "worlds/zombie.txt";
const browser = await chromium.launch({ channel: "chromium", args: ["--enable-unsafe-webgpu", "--enable-features=Metal,WebGPU", "--ignore-gpu-blocklist"] });
const page = await browser.newPage({ viewport: { width: 900, height: 600 } });
const errors = [];
page.on("pageerror", (e) => errors.push(String(e).slice(0, 300)));
await page.goto(`http://127.0.0.1:8801/?viewer=1&eval=1&world=${world}`);
await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 120000 });
await page.evaluate(async () => { window.__rt = await import("./ve_runtime.js"); });
await page.waitForFunction(() => JSON.parse(window.__rt.eval_projection_status()).committed?.quality === "final", null, { timeout: 400000 }).catch(() => {});
await page.waitForFunction(() => JSON.parse(window.__rt.game_status()).present, null, { timeout: 60000 });
let failed = 0;
const check = (name, ok, detail) => { console.log(`${ok ? "ok  " : "FAIL"} ${name}${ok ? "" : `: ${JSON.stringify(detail).slice(0, 400)}`}`); if (!ok) failed++; };
const status = () => page.evaluate(() => JSON.parse(window.__rt.game_status()));
const warnings = await page.evaluate(() => window.__ve.evaluation.state.warnings.map((w) => w.code));
check("no skinned part is parented (that drops its skin)", !warnings.includes("rig.skin_not_rigid"), warnings);
await page.evaluate(() => window.__rt.viewer_look_at(0, 1.6, 7, 0, 1.2, -2));
await page.waitForTimeout(600);
const first = await status();
check("the creature is drawn as a model with skinned parts", (first.model_skins ?? 0) >= 3, { model_skins: first.model_skins });
const start = first.entities.find((e) => /zombie/.test(e.kind))?.position;
await page.waitForTimeout(1800);
const later = await status();
const at = later.entities.find((e) => /zombie/.test(e.kind))?.position;
const moved = start && at ? Math.hypot(at[0] - start[0], at[2] - start[2]) : 0;
check("it ran toward the hero", moved > 2.5, { start, at });
// The skinned copy must change shape while it moves: two crops of the screen centre a moment apart cannot be identical.
const crop = async () => (await page.screenshot({ clip: { x: 250, y: 100, width: 400, height: 400 } })).toString("base64");
const a = await crop();
await page.waitForTimeout(250);
const b = await crop();
check("the picture of the copy changes between frames", a !== b, "identical frames");
check("no page errors", errors.length === 0, errors);
await browser.close();
process.exit(failed ? 1 : 0);
