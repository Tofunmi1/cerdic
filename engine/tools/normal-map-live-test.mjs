// Normal maps reach the screen: node tools/normal-map-live-test.mjs
// worlds/height_test.txt is a sunlit wall whose texture has a smooth `height` pattern (a grid of raised tiles). Bevy's PBR shader applies a normal map only when the mesh has
// tangents, so a viewer that never gives its meshes tangents draws the wall perfectly flat. The wall must show shading variation, and a plain wall must not.
import { chromium } from "../web/node_modules/playwright/index.mjs";
const browser = await chromium.launch({ channel: "chromium", args: ["--enable-unsafe-webgpu", "--enable-features=Metal,WebGPU", "--ignore-gpu-blocklist"] });
const page = await browser.newPage({ viewport: { width: 1200, height: 750 } });
const errors = [];
page.on("pageerror", (e) => errors.push(String(e).slice(0, 300)));
await page.goto("http://127.0.0.1:8801/?viewer=1&eval=1&scene=open&world=worlds/height_test.txt");
await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 120000 });
await page.evaluate(async () => { window.__rt = await import("./ve_runtime.js"); });
await page.waitForFunction(() => JSON.parse(window.__rt.eval_projection_status()).committed?.quality === "final", null, { timeout: 300000 });
await page.keyboard.press("h");
await page.evaluate(() => { window.__rt.viewer_configure(true, 55, false, 8); window.__rt.viewer_look_at(1.5, 1.6, 5, 0, 1.5, 0); });
await page.waitForTimeout(5000);
const shot = (await page.screenshot({ clip: { x: 260, y: 200, width: 540, height: 300 } })).toString("base64");
// The standard deviation of the luminance over the picture of the wall (0 to 255).
const spread = await page.evaluate(async (b64) => {
  const img = new Image();
  await new Promise((resolve) => { img.onload = resolve; img.src = `data:image/png;base64,${b64}`; });
  const canvas = document.createElement("canvas");
  canvas.width = img.width; canvas.height = img.height;
  const context = canvas.getContext("2d");
  context.drawImage(img, 0, 0);
  const { data } = context.getImageData(0, 0, canvas.width, canvas.height);
  let sum = 0, square = 0, count = 0;
  for (let i = 0; i < data.length; i += 4) {
    const l = 0.2126 * data[i] + 0.7152 * data[i + 1] + 0.0722 * data[i + 2];
    sum += l; square += l * l; count++;
  }
  const mean = sum / count;
  return Math.sqrt(Math.max(0, square / count - mean * mean));
}, shot);
console.log(`luminance spread over the wall: ${spread.toFixed(2)}`);
let failed = 0;
const check = (name, ok, detail) => { console.log(`${ok ? "ok  " : "FAIL"} ${name}${ok ? "" : `: ${detail}`}`); if (!ok) failed++; };
check("the raised tiles shade differently from the flat parts of the wall", spread > 6, `spread ${spread.toFixed(2)} (a flat wall measures about 1)`);
check("no page errors", errors.length === 0, JSON.stringify(errors));
await browser.close();
process.exit(failed ? 1 : 0);
