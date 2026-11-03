// solid.finish thickness reaches the screen: node tools/thickness-live-test.mjs
// worlds/thickness_test.txt has two identical thin green panels, thickness 0 on the left and 1 on the right. With the sun behind the panels (seen from the camera)
// the right one must be clearly brighter; with the sun in front of them it must be about the same. Which sun azimuth is behind depends on the camera, so the
// test tries 0 and 180 degrees and requires one of each outcome. Needs the static server on port 8801 (web/dist-bevy-gpu).
import { chromium } from "../web/node_modules/playwright/index.mjs";
const browser = await chromium.launch({ channel: "chromium", args: ["--enable-unsafe-webgpu", "--enable-features=Metal,WebGPU", "--ignore-gpu-blocklist"] });
const page = await browser.newPage({ viewport: { width: 1200, height: 750 } });
const errors = [];
page.on("pageerror", (e) => errors.push(String(e).slice(0, 300)));
await page.goto("http://127.0.0.1:8801/?viewer=1&eval=1&scene=open&world=worlds/thickness_test.txt");
await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 120000 });
await page.evaluate(async () => { window.__rt = await import("./ve_runtime.js"); });
const settled = (revision) => page.waitForFunction((r) => { const s = JSON.parse(window.__rt.eval_projection_status()); return (s.committed?.revision ?? 0) >= r && s.committed?.quality === "final"; }, revision, { timeout: 300000 });
await settled(1);
await page.keyboard.press("h");
await page.evaluate(() => { document.querySelectorAll("body > *:not(canvas)").forEach((e) => { e.style.display = "none"; }); window.__rt.viewer_configure(true, 50, false, 8); window.__rt.viewer_look_at(0, 1.2, 4.5, 0, 1.2, 0); });

// Mean luminance (0 to 255) of a square at the centre of each panel.
async function panelLuminance() {
  await page.waitForTimeout(3500);
  const shot = (await page.screenshot()).toString("base64");
  return page.evaluate(async (b64) => {
    const img = new Image();
    await new Promise((resolve) => { img.onload = resolve; img.src = `data:image/png;base64,${b64}`; });
    const canvas = document.createElement("canvas");
    canvas.width = img.width; canvas.height = img.height;
    const context = canvas.getContext("2d");
    context.drawImage(img, 0, 0);
    const mean = (cx) => {
      const { data } = context.getImageData(cx - 70, 305, 140, 140);
      let sum = 0;
      for (let i = 0; i < data.length; i += 4) sum += 0.2126 * data[i] + 0.7152 * data[i + 1] + 0.0722 * data[i + 2];
      return sum / (data.length / 4);
    };
    return { plain: mean(600 - 197), glow: mean(600 + 197) };
  }, shot);
}

const results = {};
let revision = 1;
for (const azimuth of [0, 180]) {
  if (azimuth !== 0) {
    await page.evaluate((a) => {
      const node = window.__ve.doc.state().document.nodes.find((n) => n.label === "sky");
      window.__ve.doc.send([{ set_param: { node: node.id, param: "sun_azimuth", value: a } }], "azimuth");
    }, azimuth);
    revision += 1;
    await settled(revision);
  }
  results[azimuth] = await panelLuminance();
  console.log(`sun azimuth ${azimuth}: plain ${results[azimuth].plain.toFixed(1)}, thickness ${results[azimuth].glow.toFixed(1)}, gain ${(results[azimuth].glow / Math.max(results[azimuth].plain, 1)).toFixed(2)}x`);
}
const gains = Object.values(results).map((r) => r.glow / Math.max(r.plain, 1));
let failed = 0;
const check = (name, ok, detail) => { console.log(`${ok ? "ok  " : "FAIL"} ${name}${ok ? "" : `: ${detail}`}`); if (!ok) failed++; };
check("with the sun behind the panel, thickness makes it clearly brighter", Math.max(...gains) >= 1.25, JSON.stringify(gains));
check("with the sun in front, thickness changes almost nothing", Math.min(...gains) <= 1.1, JSON.stringify(gains));
check("no page errors", errors.length === 0, JSON.stringify(errors));
await browser.close();
process.exit(failed ? 1 : 0);
