// A set of 300 placements is drawn as one retained, culled instance set, not 300 entities. It must look exactly like the same scene
// drawn entity by entity (a no-op wave on the set keeps it on that path), cull what is out of view, and be hoverable by pointing at it.
import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir } from 'node:fs/promises';
import { PNG } from 'pngjs';
import { driverAdvisory } from './support.mjs';

const root = new URL('../../../', import.meta.url).pathname;
const reports = new URL('../../target/reports/g11/', import.meta.url).pathname;
const port = 5256;
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', String(port)], {
  cwd: root, stdout: 'ignore', stderr: 'inherit',
});
const browser = await chromium.launch({ args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
const errors = [];
// 20 by 15 boxes from the origin toward +x and +z: 5.7 m by 4.2 m, centred on (2.85, 0, 2.1).
const field = `b = solid.box@1(size=[0.2, 0.6, 0.2])
up = xform.move@1(b, by=[0, 0.3, 0])
paint = solid.material@1(up, color=[0.9, 0.3, 0.1], roughness=0.7)
field = inst.array@1(paint, count_x=20, count_y=1, count_z=15, spacing=[0.3, 0, 0.3])`;
// A wave with no amplitude changes nothing, but a set with a track is drawn entity by entity.
const scenes = {
  bulk: `${field}\noutput field = field`,
  entities: `${field}\nstill = anim.wave@1(field, property="rotation", amplitude=[0, 0, 0], period=4)\noutput field = still`,
};
try {
  for (let i = 0; i < 100; i++) {
    try { if ((await fetch(`http://127.0.0.1:${port}`)).ok) break; } catch {}
    await Bun.sleep(100);
  }
  await mkdir(reports, { recursive: true });
  const open = async (name, look) => {
    const page = await browser.newPage({ viewport: { width: 800, height: 520 } });
    page.on('pageerror', error => errors.push(error.message));
    page.on('console', message => {
      if (message.type() === 'error' && !driverAdvisory.test(message.text())) errors.push(message.text());
    });
    await page.goto(`http://127.0.0.1:${port}/?viewer=1&eval=1&quality=high&scene=stage`);
    await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
    await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
    const staged = await page.evaluate(([text, label]) => window.__ve.doc.sendText(text, label), [scenes[name], name]);
    assert(staged.pending, `${name} staged: ${JSON.stringify(staged)}`);
    await page.waitForFunction(
      () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final'
        || window.__ve.evaluation.state.failureCurrent,
      null, { timeout: 120000 });
    assert.equal(await page.evaluate(() => window.__ve.evaluation.state.lastFailure), null, `${name} evaluates`);
    await page.evaluate(args => window.__runtime.viewer_look_at(...args), look);
    await page.waitForTimeout(3000);
    return page;
  };
  const hide = page => page.evaluate(() => {
    const canvas = document.querySelector('canvas');
    document.querySelectorAll('body *').forEach(element => {
      if (element !== canvas && !element.contains(canvas) && !canvas.contains(element)) element.style.visibility = 'hidden';
    });
  });
  const sets = async page => JSON.parse(await page.evaluate(() => window.__runtime.status())).render.sets;
  const ahead = [2.85, 5.5, 6.5, 2.85, 0, 2.1];

  // 1. The same picture either way.
  const picture = async name => {
    const page = await open(name, ahead);
    const counts = await sets(page);
    await hide(page);
    await page.waitForTimeout(400);
    const image = PNG.sync.read(await page.screenshot({ path: `${reports}instance-sets-${name}.png` }));
    await page.close();
    return { image, counts };
  };
  const bulk = await picture('bulk');
  const entities = await picture('entities');
  assert.equal(bulk.counts.sets, 1, `the 300 placements are one retained set (${JSON.stringify(bulk.counts)})`);
  assert.equal(bulk.counts.copies, 300);
  assert(bulk.counts.chunks_visible > 0 && bulk.counts.copies_visible === 300, `all of it is in view (${JSON.stringify(bulk.counts)})`);
  assert.equal(entities.counts.sets, 0, 'the animated version stays entity by entity');
  let orange = 0, differing = 0, worst = 0;
  const { data: a } = bulk.image, { data: b } = entities.image;
  for (let i = 0; i < a.length; i += 4) {
    if (a[i] > a[i + 1] + 60 && a[i] > a[i + 2] + 80) orange++;
    const d = Math.max(Math.abs(a[i] - b[i]), Math.abs(a[i + 1] - b[i + 1]), Math.abs(a[i + 2] - b[i + 2]));
    worst = Math.max(worst, d);
    if (d > 6) differing++;
  }
  assert(orange > 3000, `the boxes are on screen (${orange} orange pixels)`);
  assert(differing / (a.length / 4) < 0.001, `the retained set draws the same picture as entities (${differing} pixels differ, worst ${worst})`);

  // 2. Looking away culls it; looking at it draws it. And the pointer finds it.
  const page = await open('bulk', ahead);
  const seen = await sets(page);
  await page.evaluate(() => window.__runtime.viewer_look_at(2.85, 5.5, 6.5, 2.85, 5, 40));
  await page.waitForTimeout(1500);
  const away = await sets(page);
  assert.equal(away.chunks_visible, 0, `nothing of the field is drawn when it is behind the camera (${JSON.stringify(away)})`);
  assert(seen.chunks_visible > 0 && away.chunks === seen.chunks, 'the chunks are kept, only their visibility changes');
  await page.evaluate(args => window.__runtime.viewer_look_at(...args), ahead);
  await page.waitForTimeout(1500);
  await hide(page);
  await page.waitForTimeout(300);
  const view = PNG.sync.read(await page.screenshot());
  let x = 0, y = 0, n = 0;
  for (let j = 0; j < view.height; j++) for (let i = 0; i < view.width; i++) {
    const k = (j * view.width + i) * 4;
    if (view.data[k] > view.data[k + 1] + 60 && view.data[k] > view.data[k + 2] + 80) { x += i; y += j; n++; }
  }
  assert(n > 3000, 'a box to point at');
  await page.mouse.move(Math.round(x / n), Math.round(y / n));
  await page.waitForTimeout(900);
  const hover = await page.evaluate(() => JSON.stringify(window.__ve.read().interact?.hover ?? null));
  assert(hover.includes('field'), `pointing at a box hovers the set (${hover})`);
  await page.mouse.move(5, 5);
  await page.waitForTimeout(600);
  assert(!(await page.evaluate(() => JSON.stringify(window.__ve.read().interact?.hover ?? null))).includes('field'), 'and pointing away lets go');
  await page.close();
  assert.deepEqual(errors, [], 'no shader, WebGPU or browser errors');
  console.log(`G11 instance sets: 300 placements as one set, picture identical to entities (${differing} px differ), culled to ${away.chunks_visible} of ${away.chunks} chunks when behind the camera, hover ok`);
} finally {
  await browser.close();
  child.kill();
  await child.exited;
}
