import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { copyFile, mkdir, readFile, writeFile } from 'node:fs/promises';
import { PNG } from 'pngjs';
import { driverAdvisory, serve } from './support.mjs';

// The reference scenes for the rendering work (docs: the AAA plan, phase 0). Each scene is staged through the real evaluator with a fixed
// camera and the animation paused at time 0, and the picture is saved to target/reports/ref/<name>.png. The previous picture of each scene
// is kept as <name>.previous.png and compared with the new one, so a rendering change shows up as a number as well as a picture.
//
//   bun engine/web/tests/ref-scenes.mjs [name ...]        render the named scenes (default: all)
//
// Numbers per scene: mean brightness, the share of near-black and near-white pixels (exposure), and the difference from the previous run
// (mean absolute channel difference and the share of pixels that changed by more than 8). Software rendering: frame time is not measured.
const reports = new URL('../../target/reports/ref/', import.meta.url).pathname;
const testsDir = new URL('./', import.meta.url).pathname;
const demosDir = new URL('../demos/', import.meta.url).pathname;

const scenes = [
  // `scene: 'open'` is a bare camera: the document brings its own sky, sun and ground. The others sit in the stage room.
  { name: 'materials', scene: 'open', text: () => readFile(`${testsDir}ref/materials.txt`, 'utf8'), camera: [0, 2.0, 5.4, 0, 0.75, 0.4] },
  { name: 'room', scene: 'open', text: () => readFile(`${testsDir}ref/room.txt`, 'utf8'), camera: [0.4, 1.7, 2.7, -0.2, 1.25, -2] },
  { name: 'outdoor', scene: 'open', text: () => readFile(`${testsDir}ref/outdoor.txt`, 'utf8'), camera: [-11, 7.5, 12, 0, 0.8, 0], wait: 12000 },
  { name: 'stone', text: () => readFile(`${demosDir}weathered_stone.txt`, 'utf8'), camera: [0.3, 1.5, 3.6, 0, 0.5, 0] },
  { name: 'characters', characters: true, camera: [2.4, 1.5, 7.2, 2.4, 0.9, 0], wait: 12000 },
];
const wanted = process.argv.slice(2);
const chosen = wanted.length ? scenes.filter(scene => wanted.includes(scene.name)) : scenes;
assert(chosen.length > 0, `no scene matches ${wanted.join(', ')}`);

const { server, port } = await serve();
const browser = await chromium.launch({
  channel: process.env.PLAYWRIGHT_CHANNEL || undefined,
  args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'],
});
const logs = [];
const manifest = {};
try {
  await mkdir(reports, { recursive: true });
  for (const scene of chosen) {
    const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
    page.on('pageerror', error => logs.push(`${scene.name}: ${error.message}`));
    page.on('console', message => {
      if (message.type() === 'error' && !driverAdvisory.test(message.text())) logs.push(`${scene.name}: ${message.text().slice(0, 200)}`);
    });
    await page.goto(`http://127.0.0.1:${port}/?viewer=1&scene=${scene.scene ?? 'stage'}&eval=1&docname=ref-${scene.name}-${Date.now()}`);
    await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
    await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
    let text;
    if (scene.characters) {
      // A biped, a winged quadruped and a serpent from the character maker, side by side.
      text = await page.evaluate(() => ['biped', 'quadruped', 'serpent'].map((plan, i) => JSON.parse(window.__runtime.creature_make(JSON.stringify({
        plan, name: plan, at: [i * 2.4, 0, 0], sliders: { size: plan === 'quadruped' ? 2.2 : plan === 'serpent' ? 1.6 : 1.7, wings: plan === 'quadruped', horns: plan === 'quadruped' },
      }))).text).join('\n'));
    } else text = await scene.text();
    const staged = await page.evaluate(([source, label]) => window.__ve.doc.sendText(source, label), [text, scene.name]);
    assert(staged.pending, `${scene.name} staged: ${JSON.stringify(staged).slice(0, 300)}`);
    const revision = staged.pending.committed.revision;
    await page.waitForFunction(r => {
      const committed = JSON.parse(window.__runtime.eval_projection_status()).committed;
      return (committed?.quality === 'final' && committed?.revision === r) || window.__ve.evaluation.state.failureCurrent;
    }, revision, { timeout: 400000 });
    const failure = await page.evaluate(() => window.__ve.evaluation.state.lastFailure);
    assert.equal(failure, null, `${scene.name} evaluation: ${JSON.stringify(failure)}`);
    // Let the baked maps upload, hold the animation at time 0, hide every panel and place the camera.
    await page.waitForTimeout(scene.wait ?? 6000);
    await page.evaluate(() => {
      const play = document.querySelector('#tl-play');
      if (play && play.textContent.trim() === 'Pause') play.click();
      const scrub = document.querySelector('#tl-scrub');
      if (scrub) { scrub.value = '0'; scrub.dispatchEvent(new Event('input', { bubbles: true })); }
    });
    await page.addStyleTag({ content: 'body > *:not(canvas) { visibility: hidden !important; }' });
    await page.evaluate(camera => window.__runtime.viewer_look_at(...camera), scene.camera);
    await page.waitForTimeout(3500);
    const shot = await page.screenshot();
    await page.close();

    const file = `${reports}${scene.name}.png`;
    let previous = null;
    try { previous = PNG.sync.read(await readFile(file)); await copyFile(file, `${reports}${scene.name}.previous.png`); } catch { /* the first run */ }
    await writeFile(file, shot);
    const now = PNG.sync.read(shot);
    let sum = 0, dark = 0, bright = 0, diff = 0, changed = 0;
    const pixels = now.width * now.height;
    for (let i = 0; i < now.data.length; i += 4) {
      const luma = 0.2126 * now.data[i] + 0.7152 * now.data[i + 1] + 0.0722 * now.data[i + 2];
      sum += luma;
      if (luma < 12) dark++;
      if (luma > 243) bright++;
      if (previous && previous.width === now.width && previous.height === now.height) {
        const d = Math.max(Math.abs(now.data[i] - previous.data[i]), Math.abs(now.data[i + 1] - previous.data[i + 1]), Math.abs(now.data[i + 2] - previous.data[i + 2]));
        diff += d;
        if (d > 8) changed++;
      }
    }
    manifest[scene.name] = {
      meanLuma: Math.round(sum / pixels * 10) / 10,
      nearBlackPct: Math.round(dark / pixels * 1000) / 10,
      nearWhitePct: Math.round(bright / pixels * 1000) / 10,
      ...(previous ? { meanDiff: Math.round(diff / pixels * 100) / 100, changedPct: Math.round(changed / pixels * 1000) / 10 } : { firstRun: true }),
    };
    console.log(`ref ${scene.name.padEnd(10)} ${JSON.stringify(manifest[scene.name])}`);
  }
  await writeFile(`${reports}manifest.json`, JSON.stringify({ at: new Date().toISOString(), scenes: manifest }, null, 2));
  assert.deepEqual(logs, [], 'no browser errors');
} finally {
  await browser.close();
  server.close();
}
