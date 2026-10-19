import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { driverAdvisory } from './support.mjs';

// B4: viewer grading reaches the renderer. The study room is staged and captured neutral, then with
// the mono preset; the two images must differ, and the status must report the clamped controls.
const root = new URL('../../../', import.meta.url).pathname;
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', '5199'], {
  cwd: root, stdout: 'ignore', stderr: 'inherit',
});
const browser = await chromium.launch({ args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
const reports = new URL('../../target/reports/g9/', import.meta.url).pathname;
const logs = [];
try {
  for (let i = 0; i < 100; i++) {
    try { if ((await fetch('http://127.0.0.1:5199')).ok) break; } catch {}
    await Bun.sleep(100);
  }
  const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
  page.on('pageerror', error => logs.push(error.message));
  page.on('console', message => {
    if (message.type() === 'error' && !driverAdvisory.test(message.text())) logs.push(message.text());
  });
  await page.goto('http://127.0.0.1:5199/?viewer=1&eval=1&fov=75&docname=g9-grading');
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  await mkdir(reports, { recursive: true });

  const room = await readFile(`${root}engine/crates/ve-modeling/tests/gallery/visual/study_room.txt`, 'utf8');
  const lights = `\nsun_main = light.sun@1(direction=[-0.4, -1, -0.2], color=[1, 0.92, 0.82], intensity=3)\noutput main_light = sun_main\n`;
  const stagedRoom = await page.evaluate(([source, name]) => window.__ve.doc.sendText(source, name), [`${room}\n${lights}`, 'G9 grading room']);
  assert(stagedRoom.pending, `room staged: ${JSON.stringify(stagedRoom)}`);
  await page.waitForFunction(
    () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final',
    null, { timeout: 180000 });
  assert(await page.evaluate(() => window.__runtime.viewer_look_at(0, 2.7, 6.1, 0, 0.9, 0)), 'review camera accepted');

  const status = () => page.evaluate(() => JSON.parse(window.__runtime.grading_status()));
  const shoot = async name => {
    const frameStart = await page.evaluate(() => window.__ve.read().frames);
    await page.waitForFunction(start => window.__ve.read().frames >= start + 12, frameStart, { timeout: 30000 });
    await page.screenshot({ path: `${reports}grading-${name}.png` });
  };

  assert.deepEqual(await status(), { exposure: 1, contrast: 1, saturation: 1, tint: [1, 1, 1] },
    'grading starts neutral');
  await shoot('neutral');
  const neutral = await readFile(`${reports}grading-neutral.png`);

  await page.evaluate(() => window.__runtime.set_grading_preset('mono'));
  assert.equal((await status()).saturation, 0, 'mono preset reached the engine');
  await shoot('mono');
  const mono = await readFile(`${reports}grading-mono.png`);
  assert(!neutral.equals(mono), 'the mono preset changes the rendered image');

  // Out-of-range controls are clamped, not rejected, and never reach the shader out of range.
  await page.evaluate(() => window.__runtime.set_grading(99, -3, 5, 2, 0.1, 0.5));
  const clamped = await status();
  assert.deepEqual(clamped, { exposure: 4, contrast: 0, saturation: 2, tint: [1.5, 0.5, 0.5] },
    'grading controls are clamped to their ranges');

  const rejected = await page.evaluate(() => {
    try { window.__runtime.set_grading_preset('sepia'); return false; } catch { return true; }
  });
  assert(rejected, 'an unknown preset name is rejected');

  // The in-room captures are the review images; the mono one shows the preset on screen.
  await page.evaluate(() => window.__runtime.set_grading_preset('warm'));
  await shoot('warm');
  assert.deepEqual(logs, [], 'no browser errors');
  await writeFile(`${reports}grading-profile.json`, JSON.stringify({
    neutral: { exposure: 1, contrast: 1, saturation: 1, tint: [1, 1, 1] },
    mono: { saturation: 0 },
    clamped,
  }, null, 2));
  console.log('G9 grading: neutral, mono and warm captured; clamped controls and unknown-preset rejection verified');
} finally {
  await browser.close(); child.kill(); await child.exited;
}
