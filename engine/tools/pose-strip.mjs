// A pose strip: one image of an animated world at several moments, so a model (or a person) can SEE a clip it wrote.
//   node tools/pose-strip.mjs <world.txt> <out.png> "0,0.25,0.5,0.75" ["ex,ey,ez,tx,ty,tz"]
// The world is loaded in the viewer on :8801 (served from engine/web/dist-bevy-gpu, worlds in worlds/). Each time is seconds into the clip
// (`viewer_animation(false, t)` pauses the clock there); the camera stays where it is, or goes to the given eye and target.
// Set STAGE=1 for the studio room, OPEN=1 for the open scene (default: the studio room, which suits a single character).
import { chromium } from "../web/node_modules/playwright/index.mjs";
import { writeFileSync } from "node:fs";

const [world, out, timesArg, view] = process.argv.slice(2);
if (!world || !out || !timesArg) {
  console.error('usage: node tools/pose-strip.mjs <world.txt> <out.png> "0,0.25,0.5" ["ex,ey,ez,tx,ty,tz"]');
  process.exit(2);
}
const times = timesArg.split(",").map(Number);
const scene = process.env.OPEN ? "scene=open&" : process.env.STAGE === "0" ? "" : "scene=stage&";
const browser = await chromium.launch({ channel: "chromium", args: ["--enable-unsafe-webgpu", "--enable-features=Metal,WebGPU", "--ignore-gpu-blocklist"] });
// A wide window: at a narrow one the editor panels do not fold away and cover the character.
const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
await page.goto(`http://localhost:8801/?viewer=1&eval=1&${scene}quality=high&bloom=on&world=${world}`);
await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 120000 });
await page.evaluate(async () => { window.__runtime = await import("./ve_runtime.js"); });
await page.waitForFunction(() => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === "final", null, { timeout: 500000 });
await page.waitForTimeout(6000);
await page.keyboard.press("h"); // hide the panels so the strip shows the character, not the editor
if (view) {
  await page.evaluate((a) => window.__runtime.viewer_look_at(...a), view.split(",").map(Number));
  await page.waitForTimeout(1500);
}
// PARAMS='[{"speed":0},{"speed":1.5}]' (one object per time): a world with an `anim.machine` is not scrubbed to a time but let run for 1.5
// seconds with that frame's parameters set, so the strip shows the machine's blend at each setting (the time label is the frame's number).
const params = process.env.PARAMS ? JSON.parse(process.env.PARAMS) : null;
const frames = [];
for (const [i, t] of times.entries()) {
  if (params) {
    // Per frame: parameters to set, `trigger` to raise, and `wait` (milliseconds the machine runs before the picture, default 1500).
    const entry = params[i] ?? {};
    await page.evaluate(({ set }) => {
      for (const [name, value] of Object.entries(set)) {
        if (name === "wait") continue;
        if (name === "trigger") window.__runtime.animation_trigger("", value);
        else if (typeof value === "boolean") window.__runtime.animation_set_bool("", name, value);
        else window.__runtime.animation_set_float("", name, value);
      }
      window.__runtime.viewer_animation(true, -1);
    }, { set: entry });
    await page.waitForTimeout(entry.wait ?? 1500);
  } else {
    await page.evaluate((seconds) => window.__runtime.viewer_animation(false, seconds), t);
    await page.waitForTimeout(900);
  }
  // The left 900 px: the editor panels sit at the right edge and would cover half the character.
  frames.push((await page.screenshot({ type: "png", clip: { x: 0, y: 0, width: 900, height: 720 } })).toString("base64"));
}
// Stitch the frames side by side in a scratch page, with the time written under each.
const strip = await browser.newPage({ viewport: { width: 400 * frames.length, height: 345 } });
await strip.setContent("<body style='margin:0;background:#111'><canvas id=c></canvas></body>");
const png = await strip.evaluate(async ({ frames, times }) => {
  const canvas = document.getElementById("c");
  canvas.width = 400 * frames.length;
  canvas.height = 345;
  const g = canvas.getContext("2d");
  g.font = "16px monospace";
  g.fillStyle = "#fff";
  for (const [i, data] of frames.entries()) {
    const image = new Image();
    await new Promise((resolve) => { image.onload = resolve; image.src = `data:image/png;base64,${data}`; });
    g.drawImage(image, i * 400, 0, 400, 320);
    g.fillText(`t = ${times[i]} s`, i * 400 + 8, 338);
  }
  return canvas.toDataURL("image/png").split(",")[1];
}, { frames, times });
writeFileSync(out, Buffer.from(png, "base64"));
console.log(`wrote ${out} (${frames.length} poses)`);
await browser.close();
