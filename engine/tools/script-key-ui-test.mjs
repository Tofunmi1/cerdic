// Scripts hear keys and draw a screen UI: node tools/script-key-ui-test.mjs   (needs the static server on port 8801 serving web/dist-bevy-gpu)
// A script listens for KeyQ, draws a text and a bar, flashes and shakes the screen, asks where the player looks from, and sends one refused spec. The page must deliver the
// key down and up, draw the elements, refuse the bad colour with a sentence, answer view.pose(), and remove the UI when the script stops.
import { chromium } from "../web/node_modules/playwright/index.mjs";
const browser = await chromium.launch({ channel: "chromium", args: ["--enable-unsafe-webgpu", "--enable-features=Metal,WebGPU", "--ignore-gpu-blocklist"] });
const page = await browser.newPage({ viewport: { width: 1000, height: 650 } });
const errors = [];
page.on("pageerror", (e) => errors.push(String(e).slice(0, 300)));
await page.goto("http://127.0.0.1:8801/?viewer=1&eval=1&scene=open&world=worlds/sky_only.txt");
await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 120000 });
await page.waitForTimeout(1500);
const started = await page.evaluate(() => window.__ve.scripts.start("keyui", `
  on('key:KeyQ', (e) => log('KeyQ', e.down ? 'down' : 'up'));
  on('key', (e) => { if (e.code === 'KeyE') log('any', e.code); });
  ui.set('title', { kind: 'text', text: 'hello', x: 0.5, y: 0.1, size: 30, color: '#ffcc00' });
  ui.set('hp', { kind: 'bar', x: 0.1, y: 0.9, w: 0.3, h: 16, value: 0.5, color: '#cc3333', label: 'HP' });
  ui.flash('#ff0000', 0.3, 0.4);
  ui.shake(6, 0.3);
  ui.set('bad', { kind: 'text', text: 'x', color: 'url(javascript:alert(1))' });
  const p = await view.pose();
  log('pose', JSON.stringify(p));
`));
await page.waitForTimeout(1200);
await page.keyboard.press("KeyQ");
await page.keyboard.press("KeyE");
await page.waitForTimeout(500);
const logs = await page.evaluate(() => window.__ve.scripts.logs("keyui"));
const dom = await page.evaluate(() => {
  const root = document.getElementById("ve-script-ui");
  const text = [...root.children].map((c) => c.textContent).filter(Boolean);
  const bar = [...root.children].find((c) => c.textContent === "HP");
  return { count: root.children.length, text, barWidth: bar?.children[1]?.style.width };
});
let failed = 0;
const check = (name, ok, detail) => { console.log(`${ok ? "ok  " : "FAIL"} ${name}${ok ? "" : `: ${detail}`}`); if (!ok) failed++; };
check("the script started", started.ok === true, JSON.stringify(started));
check("a key down and a key up reach on('key:KeyQ')", logs.logs.includes("KeyQ down") && logs.logs.includes("KeyQ up"), JSON.stringify(logs));
check("on('key') hears every key", logs.logs.some((l) => l.startsWith("any KeyE")), JSON.stringify(logs.logs));
check("the text and the bar are drawn", dom.text.includes("hello") && dom.text.includes("HP"), JSON.stringify(dom));
check("the bar's fill is half its width", dom.barWidth === "50%", dom.barWidth);
check("a refused spec says what is wrong with the color", logs.errors.some((e) => /color/.test(e)), JSON.stringify(logs.errors));
check("view.pose() answers a position", logs.logs.some((l) => /^pose .*"position":\[[-\d.e]+,[-\d.e]+,[-\d.e]+\]/.test(l)), JSON.stringify(logs.logs));
await page.evaluate(() => window.__ve.scripts.stop("keyui"));
await page.waitForTimeout(200);
const after = await page.evaluate(() => [...document.getElementById("ve-script-ui").children].filter((c) => c.textContent).length);
check("stopping the script removes its UI", after === 0, `${after} elements left`);
check("no page errors", errors.length === 0, JSON.stringify(errors));
await browser.close();
process.exit(failed ? 1 : 0);
