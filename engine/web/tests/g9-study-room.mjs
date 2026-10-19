import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { driverAdvisory } from './support.mjs';

const root = new URL('../../../', import.meta.url).pathname;
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', '5197'], {
  cwd: root, stdout: 'ignore', stderr: 'inherit',
});
const browser = await chromium.launch({ args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
const reports = new URL('../../target/reports/g9/', import.meta.url).pathname;
const logs = [];
try {
  for (let i = 0; i < 100; i++) {
    try { if ((await fetch('http://127.0.0.1:5197')).ok) break; } catch {}
    await Bun.sleep(100);
  }
  const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
  page.on('pageerror', error => logs.push(error.message));
  page.on('console', message => {
    if (message.type() === 'error' && !driverAdvisory.test(message.text())) logs.push(message.text());
  });
  await page.goto('http://127.0.0.1:5197/?viewer=1&eval=1&fov=75&docname=g9-study-room');
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  const room = await readFile(`${root}engine/crates/ve-modeling/tests/gallery/visual/study_room.txt`, 'utf8');
  const emittingRoom = room
    .replace('emissive=[0.8, 1.1, 1.6])', 'emissive=[0.8, 1.1, 1.6], emits_light=true)')
    .replace('emissive=[0.25, 0.55, 1.0])', 'emissive=[0.25, 0.55, 1.0], emits_light=true)')
    .replace('emissive=[4, 3.2, 1.6])', 'emissive=[4, 3.2, 1.6], emits_light=true)');
  const texturedRoom = emittingRoom;
  for (const recipe of ["planks", "painted_wall", "wood", "fabric", "leather"]) {
    assert(texturedRoom.includes(`recipe="${recipe}"`), `${recipe} recipe was applied to the room fixture`);
  }
  const source = `${texturedRoom}
sun_main = light.sun@1(direction=[-0.4, -1, -0.2], color=[1, 0.92, 0.82], intensity=3)
window_fill = light.point@1(position=[-3, 3, -3], color=[0.62, 0.8, 1], intensity=8, range=8)
desk_spot = light.spot@1(position=[1.7, 1.2, 0.8], direction=[-0.6, -0.8, -0.4], color=[1, 0.68, 0.38], intensity=6, range=6, inner_angle=0.45, outer_angle=0.9)
output main_light = sun_main
output window_fill = window_fill
output desk_spot = desk_spot
`;
  const started = performance.now();
  const staged = await page.evaluate(text => window.__ve.doc.sendText(text, 'G9 study room profile'), source);
  assert(staged.pending, `study room staged: ${JSON.stringify(staged)}`);
  await page.waitForFunction(() => {
    const projection = JSON.parse(window.__runtime.eval_projection_status());
    return projection.committed?.quality === 'preview';
  }, null, { timeout: 180000, polling: 10 });
  const previewWallMs = performance.now() - started;
  const preview = await page.evaluate(() => ({
    revision: window.__ve.evaluation.state.revision,
    stats: window.__ve.evaluation.state.stats,
    projection: JSON.parse(window.__runtime.eval_projection_status()),
  }));
  assert.equal(preview.projection.committed?.quality, 'preview', 'preview wall time measures the Preview result');
  assert(preview.stats && Number.isFinite(preview.stats.elapsed_ms), 'worker reports Preview evaluation stats');
  await page.waitForFunction(() => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final',
    null, { timeout: 180000 });
  const finalWallMs = performance.now() - started;
  const final = await page.evaluate(() => ({
    state: window.__ve.evaluation.state,
    projection: JSON.parse(window.__runtime.eval_projection_status()),
  }));
  assert.equal(preview.revision, 1, 'preview accepted the one atomic room edit');
  assert.equal(final.state.revision, 1, 'final refined the same document revision');
  assert.equal(final.projection.committed?.quality, 'final', 'final projection accepted');
  assert.equal(final.projection.live_lights, 6, 'sun, point, spot and three emissive sources reached the committed viewport');
  assert(final.state.stats && Number.isFinite(final.state.stats.elapsed_ms), 'worker reports Final evaluation stats');
  assert.equal(final.state.lastFailure, null, 'no evaluation failure');
  const aimed = await page.evaluate(() => {
    return window.__runtime.viewer_look_at(0, 2.7, 6.1, 0, 0.9, 0);
  });
  assert(aimed, 'engine viewer accepted the in-room study-room review camera');
  const frameStart = await page.evaluate(() => window.__ve.read().frames);
  await page.waitForFunction(start => window.__ve.read().frames >= start + 12, frameStart, { timeout: 30000 });
  await mkdir(reports, { recursive: true });
  await page.screenshot({ path: `${reports}study-room.png` });
  await writeFile(`${reports}study-room-profile.json`, JSON.stringify({
    fixture: 'study_room.txt', fixtureCommands: 386, fixtureEncodedBytes: 43465,
    lightingCommands: 6, proceduralRecipes: ["planks", "painted_wall", "wood", "fabric", "leather"], previewWallMs, finalWallMs, preview, final,
  }, null, 2));
  assert.deepEqual(logs, [], 'no browser errors');
  console.log(`G9 study room: Preview ${previewWallMs.toFixed(1)} ms; Final ${finalWallMs.toFixed(1)} ms; worker ${preview.stats?.elapsed_ms}/${final.state.stats?.elapsed_ms} ms`);
} finally {
  await browser.close(); child.kill(); await child.exited;
}
