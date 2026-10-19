// Browser integration fixture for every typed G9 procedural surface recipe.
import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { PNG } from 'pngjs';
import { mkdir, writeFile } from 'node:fs/promises';
import { driverAdvisory } from './support.mjs';

const root = new URL('../../../', import.meta.url).pathname;
// 5201: g9-environment owns 5198 and the two run in the same parallel pool.
const port = 5201;
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', String(port)], {
  cwd: root, stdout: 'ignore', stderr: 'inherit',
});
const browser = await chromium.launch({ args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
const reports = new URL('../../target/reports/g9/', import.meta.url).pathname;
const logs = [];
const recipes = ['none', 'wood', 'planks', 'brushed_metal', 'leather', 'fabric', 'brick', 'concrete', 'marble', 'painted_wall'];
try {
  for (let i = 0; i < 100; i++) {
    try { if ((await fetch(`http://127.0.0.1:${port}`)).ok) break; } catch {}
    await Bun.sleep(100);
  }
  const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
  page.on('pageerror', error => logs.push(error.message));
  page.on('console', message => {
    if (message.type() === 'error' && !driverAdvisory.test(message.text())) logs.push(message.text());
  });
  await page.goto(`http://127.0.0.1:${port}/?viewer=1&eval=1&fov=90&docname=g9-material-gallery`);
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  await page.waitForFunction(() => window.__ve.read().viewer?.started, null, { timeout: 30000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });

  const commands = recipes.map((recipe, index) => {
    const x = (index % 5 - 2) * 1.5;
    const z = (Math.floor(index / 5) - 0.5) * 1.8;
    const label = recipe.replaceAll('_', ' ');
    const colour = recipe === 'brushed_metal' ? '[0.55, 0.62, 0.7]' : '[0.72, 0.52, 0.34]';
    return `b${index} = solid.box@1(size=[1.2,1.2,1.2])\nm${index} = solid.material@1(b${index}, color=${colour}, roughness=0.72, recipe="${recipe}", texture_scale=1.4)\np${index} = xform.move@1(m${index}, by=[${x},0,${z}])\noutput ${label.replaceAll(' ', '_')} = p${index}`;
  }).join('\n');
  const staged = await page.evaluate(text => window.__ve.doc.sendText(text, 'G9 procedural material gallery'), commands);
  assert(staged.pending, `recipe gallery staged: ${JSON.stringify(staged)}`);
  await page.waitForFunction(() => window.__ve.evaluation.state.revision === 1
    && window.__ve.evaluation.state.worker === 'idle'
    && JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final', null, { timeout: 180000 });
  const state = await page.evaluate(() => ({
    revision: window.__ve.evaluation.state.revision,
    quality: JSON.parse(window.__runtime.eval_projection_status()).committed?.quality,
    failure: window.__ve.evaluation.state.lastFailure,
  }));
  assert.equal(state.revision, 1, 'all recipe examples arrive in one revision');
  assert.equal(state.quality, 'final');
  assert.equal(state.failure, null);
  const bounds = await page.evaluate(() => window.__ve.evaluation.bounds);
  assert(bounds, 'the engine accepted bounds for the material gallery');
  const framed = await page.evaluate(bounds => window.__runtime.viewer_frame(...bounds.min, ...bounds.max), bounds);
  assert(framed, `engine viewer accepted framing bounds: ${JSON.stringify(bounds)}`);
  const aimed = await page.evaluate(() => {
    return window.__runtime.viewer_look_at(0, 3.5, 6.1, 0, 0, 0);
  });
  assert(aimed, 'engine viewer accepted the in-room review camera');
  const frameStart = await page.evaluate(() => window.__ve.read().frames);
  await page.waitForFunction(start => window.__ve.read().frames >= start + 12, frameStart, { timeout: 30000 });
  const engineStatus = await page.evaluate(() => window.__ve.read());
  const projection = await page.evaluate(() => JSON.parse(window.__runtime.eval_projection_status()));
  assert.equal(projection.live_meshes, recipes.length, 'all ten recipe meshes are committed to the engine scene');
  await mkdir(reports, { recursive: true });
  await page.screenshot({ path: `${reports}material-recipes.png` });
  await page.evaluate(() => {
    for (const child of document.body.children) if (child.id !== 've') child.style.setProperty('display', 'none', 'important');
  });
  const canvasShot = PNG.sync.read(await page.screenshot());
  let visiblePixels = 0;
  const backdrop = [canvasShot.data[0], canvasShot.data[1], canvasShot.data[2]];
  for (let i = 0; i < canvasShot.data.length; i += 4) {
    if (Math.max(Math.abs(canvasShot.data[i] - backdrop[0]), Math.abs(canvasShot.data[i + 1] - backdrop[1]), Math.abs(canvasShot.data[i + 2] - backdrop[2])) > 20) visiblePixels++;
  }
  assert(visiblePixels > 2000, `engine viewer rendered visible scene pixels (${visiblePixels}); bounds=${JSON.stringify(bounds)} projection=${JSON.stringify(projection)} status=${JSON.stringify(engineStatus)}`);
  await writeFile(`${reports}material-recipes.json`, JSON.stringify({ recipes, state, bounds, projection, engineStatus, visiblePixels, visualReview: 'pending' }, null, 2));
  assert.deepEqual(logs, [], 'no browser or shader errors');
  console.log(`G9 material gallery: ${recipes.length} recipes accepted in Final`);
} finally {
  await browser.close(); child.kill(); await child.exited;
}
