import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFile, mkdir, writeFile } from 'node:fs/promises';
import { chromium } from 'playwright';
import { driverAdvisory } from './support.mjs';

// B8: reproducible fixed-view captures from the shipped Engine Viewer. Owner scores are collected in
// the generated review page and deliberately remain blank until the owner inspects the images.
const root = new URL('../../../', import.meta.url).pathname;
const port = 5203;
const reports = new URL('../../target/reports/g9/', import.meta.url).pathname;
const reviewDir = `${reports}review/`;
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', String(port)], {
  cwd: root, stdout: 'ignore', stderr: 'inherit',
});
let browser = await chromium.launch({ args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
const logs = [];
const light = `review_sun = light.sun@1(direction=[-0.4, -1, -0.3], color=[1, 0.92, 0.82], intensity=3)\noutput review_light = review_sun\n`;
const fixtures = [
  { name: 'pistol', file: 'engine/crates/ve-modeling/tests/gallery/visual/pistol_prop.txt', target: [0, 0.075, 0], distance: 0.55 },
  { name: 'study-room', file: 'engine/crates/ve-modeling/tests/gallery/visual/study_room.txt', target: [0, 0.9, 0], distance: 5.7 },
  { name: 'field-creature', file: 'engine/crates/ve-modeling/tests/gallery/field_creature.txt', target: [0, 0.9, 0], distance: 3.4 },
  { name: 'ceramic-vase', file: 'engine/crates/ve-modeling/tests/gallery/ceramic_vase.txt', target: [0, 0.2, 0], distance: 0.68 },
  { name: 'kit-composition', file: null, target: [0, 0.85, 0.5], distance: 4.3 },
];
const views = [
  ['front', [0, 0.85, 1]], ['back', [0, 0.85, -1]], ['left', [-1, 0.85, 0]], ['right', [1, 0.85, 0]],
  ['top', [0, 1, 0.025]], ['three-quarter', [1, 0.72, 1]],
];
const sha256 = value => createHash('sha256').update(value).digest('hex');
const kitSource = async name => readFile(`${root}engine/crates/ve-modeling/kits/${name}.txt`, 'utf8');
const sourceFor = async fixture => {
  if (fixture.name === 'kit-composition') {
    const [desk, shelf, window] = await Promise.all(['desk', 'shelf', 'window'].map(kitSource));
    return `${desk}\n${shelf}\n${window}\ndesk_obj = desk()\nshelf_obj = shelf()\nwindow_obj = window()\n`
      + 'desk_place = xform.move@1(desk_obj, by=[-1.2, 0, 0])\n'
      + 'shelf_place = xform.move@1(shelf_obj, by=[1.1, 0, -0.2])\n'
      + 'scatter_proto = solid.box@1(size=[0.12, 0.08, 0.1])\n'
      + 'scatter_mat = solid.material@1(scatter_proto, color=[0.45, 0.42, 0.38], roughness=0.8)\n'
      + 'scatter = inst.scatter@1(scatter_mat, count=36, width=2.8, depth=1.8, min_spacing=0.28, yaw_jitter=180deg, scale_jitter=0.4, seed=4)\n'
      + 'scatter_place = xform.move@1(scatter, by=[0, 0.05, 1.1])\n'
      + 'output desk = desk_place\noutput shelf = shelf_place\noutput window = window_obj\noutput scatter = scatter_place\n';
  }
  return readFile(`${root}${fixture.file}`, 'utf8');
};
const gitInfo = () => {
  const revision = Bun.spawnSync(['git', 'rev-parse', 'HEAD'], { cwd: root }).stdout.toString().trim();
  const changes = Bun.spawnSync(['git', 'status', '--porcelain'], { cwd: root }).stdout.toString().trim();
  const dirtyFiles = changes ? changes.split('\n').map(line => line.slice(3)) : [];
  return { revision, dirty: dirtyFiles.length > 0, dirtyFiles };
};
const htmlEscape = value => value.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('"', '&quot;');

async function comparisonCapture({ source, name, target, distance, image }) {
  const started = performance.now();
  const comparisonBrowser = await chromium.launch({ args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
  const page = await comparisonBrowser.newPage({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: 1 });
  page.on('pageerror', error => logs.push(error.message));
  page.on('console', message => { if (message.type() === 'error' && !driverAdvisory.test(message.text())) logs.push(message.text()); });
  try {
    await page.goto(`http://127.0.0.1:${port}/?viewer=1&eval=1&fov=60&docname=g9-review-${name}`);
    await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
    await page.evaluate(() => {
      const nativeSetTimeout = window.setTimeout.bind(window);
      window.setTimeout = (callback, delay, ...args) => nativeSetTimeout(delay === 400 ? () => {} : callback, delay, ...args);
    });
    await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
    const staged = await page.evaluate(([text, label]) => window.__ve.doc.sendText(text, label), [source, name]);
    assert(staged.pending, `${name} staged: ${JSON.stringify(staged)}`);
    await page.waitForFunction(() => JSON.parse(window.__runtime.eval_projection_status()).committed?.revision === 1
      && JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'preview'
      && window.__ve.evaluation.state.worker === 'idle', null, { timeout: 180000 });
    await page.evaluate(() => window.__ve.evaluation.request('final'));
    await page.waitForFunction(() => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final'
      && window.__ve.evaluation.state.worker === 'idle', null, { timeout: 180000 });
    const direction = [1, 0.72, 1];
    const length = Math.hypot(...direction);
    const eye = direction.map((value, axis) => target[axis] + value / length * distance);
    assert(await page.evaluate(([camera, aim]) => window.__runtime.viewer_look_at(...camera, ...aim), [eye, target]), `${name}: comparison camera accepted`);
    await page.addStyleTag({ content: '#doc-panel,#viewer-hud,#viewer-gizmo-controls,#viewer-gizmo,#crosshair,#enter-vr{display:none!important}' });
    const start = await page.evaluate(() => window.__ve.read().frames);
    await page.waitForFunction(frame => window.__ve.read().frames >= frame + 3, start, { timeout: 30000 });
    await page.screenshot({ path: `${reviewDir}${image}` });
    const projection = await page.evaluate(() => ({
      triangles: window.__ve.read().render.main.triangles,
      quality: JSON.parse(window.__runtime.eval_projection_status()).committed?.quality,
    }));
    assert.equal(projection.quality, 'final', `${name}: comparison captured Final`);
    assert(projection.triangles > 0, `${name}: comparison rendered geometry`);
    return { image, sha256: sha256(source), triangles: projection.triangles, cameraPosition: eye,
      cameraTarget: target, quality: projection.quality, totalWallMs: performance.now() - started };
  } finally {
    await page.close();
    // Chromium exits after close is requested, but Playwright's disconnected promise can remain
    // pending on macOS SwiftShader after a screenshot. Do not hold the review run on that promise.
    void comparisonBrowser.close().catch(() => {});
  }
}

function reviewHtml(manifest, selectedFixture = null) {
  const criteria = ['edge readability', 'material distinction', 'lighting/contact depth', 'silhouette', 'composition'];
  const fixtures = selectedFixture ? manifest.fixtures.filter(fixture => fixture.name === selectedFixture) : manifest.fixtures;
  const scenes = fixtures.map(fixture => {
    const images = fixture.captures.map(capture => `<figure><img src="${capture.image}" alt="${fixture.name} ${capture.view} ${capture.quality}"><figcaption>${capture.view} · ${capture.quality}</figcaption></figure>`).join('');
    const inputs = criteria.map((criterion, index) => `<label>${criterion}<input type="number" min="1" max="5" step="1" data-score="${index}" aria-label="${fixture.name}: ${criterion}"></label>`).join('');
    return `<section><h2>${htmlEscape(fixture.name)}</h2><p class="meta">Fixture SHA-256: <code>${fixture.sha256}</code> · ${fixture.captures.length} captures</p><div class="views">${images}</div><div class="scores">${inputs}<label>Notes<textarea data-notes aria-label="${fixture.name}: review notes"></textarea></label></div></section>`;
  }).join('');
  const pairs = (manifest.comparisons ?? []).map(pair =>
    `<section><h2>${htmlEscape(pair.title)}</h2><p>${htmlEscape(pair.description)}</p><div class="pair"><figure><img src="${pair.before.image}" alt="${htmlEscape(pair.title)} baseline"><figcaption>Before · ${pair.before.triangles} triangles</figcaption></figure><figure><img src="${pair.after.image}" alt="${htmlEscape(pair.title)} changed"><figcaption>After · ${pair.after.triangles} triangles</figcaption></figure></div></section>`).join('') + [
    ['B3 · authored environment', '../environment-study-room-before.png', '../environment-study-room-after.png'],
    ['B4 · grading', '../grading-neutral.png', '../grading-warm.png'],
    ['B6 · chamfer', '../chamfer-sharp.png', '../chamfer-cut.png'],
  ].map(([title, before, after]) => `<section><h2>${title} · focused capture</h2><div class="pair"><figure><img src="${before}" alt="${title} baseline"><figcaption>Baseline</figcaption></figure><figure><img src="${after}" alt="${title} changed"><figcaption>Changed</figcaption></figure></div></section>`).join('');
  return `<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width"><title>Spatial Lab · G9 visual review</title>
<style>body{font:15px/1.5 system-ui,sans-serif;margin:24px auto;max-width:1500px;padding:0 20px;background:#11151b;color:#f3f5f8}h1,h2{line-height:1.2}p,figcaption,.meta{color:#bbc3cf}section{margin:26px 0;padding:18px;border:1px solid #48515d;border-radius:12px;background:#1b222b}.views{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:10px}.pair{display:grid;grid-template-columns:1fr 1fr;gap:12px}figure{margin:0}img{display:block;width:100%;height:auto;border-radius:6px;background:#303640}figcaption{padding:5px 2px}.scores{display:grid;grid-template-columns:repeat(5,minmax(140px,1fr));gap:10px;margin-top:16px}.scores label{display:grid;gap:5px;text-transform:capitalize}.scores textarea{grid-column:1/-1;min-height:64px}input,textarea,button{font:inherit;color:inherit;background:#11151b;border:1px solid #687484;border-radius:6px;padding:8px}button{cursor:pointer;background:#304c6a}code{overflow-wrap:anywhere}.pending{border-left:4px solid #e6b65b;padding:8px 12px}@media(max-width:800px){.views{grid-template-columns:repeat(2,1fr)}.scores{grid-template-columns:repeat(2,1fr)}}</style>
<h1>Spatial Lab · G9 visual review</h1><p class="pending"><strong>Owner review pending.</strong> Scores are intentionally blank. Confirm the regenerated engine/XR pixel goldens as part of this review. SwiftShader captures are for visual comparison only; they are not Quest performance evidence.</p>
<p>Revision ${htmlEscape(manifest.git.revision)}${manifest.git.dirty ? ' (workspace had uncommitted changes)' : ''}. Renderer: ${manifest.renderer}. Fixture hashes and capture measurements are in <code>review-manifest.json</code>.</p>
<button id="download">Download entered scores as JSON</button>${scenes}<h2>Same-camera feature comparisons</h2>${pairs}
<script>document.querySelector('#download').addEventListener('click',()=>{const data=[...document.querySelectorAll('section h2')].map(h=>{const s=h.closest('section');return {scene:h.textContent,scores:[...s.querySelectorAll('[data-score]')].map(i=>i.value?Number(i.value):null),notes:s.querySelector('[data-notes]')?.value??''}}).filter(x=>x.scores.some(v=>v!==null)||x.notes);const a=document.createElement('a');a.href=URL.createObjectURL(new Blob([JSON.stringify({reviewer:'owner',scores:data},null,2)],{type:'application/json'}));a.download='g9-owner-review.json';a.click();URL.revokeObjectURL(a.href)})</script></html>`;
}

try {
  for (let i = 0; i < 100; i++) {
    try { if ((await fetch(`http://127.0.0.1:${port}`)).ok) break; } catch {}
    await Bun.sleep(100);
  }
  await mkdir(reviewDir, { recursive: true });
  const manifest = { generatedAt: new Date().toISOString(), git: gitInfo(), renderer: 'Chromium SwiftShader', fixtures: [] };
  for (const fixture of fixtures) {
    const source = await sourceFor(fixture);
    const program = `${source}\n${light}`;
    const page = await browser.newPage({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: 1 });
    page.on('pageerror', error => logs.push(error.message));
    page.on('console', message => { if (message.type() === 'error' && !driverAdvisory.test(message.text())) logs.push(message.text()); });
    const captures = [];
    const started = performance.now();
    try {
      await page.goto(`http://127.0.0.1:${port}/?viewer=1&eval=1&fov=60&docname=g9-review-${fixture.name}`);
      await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
      // The viewer's normal idle upgrade replaces Preview with Final after 400 ms. Hold that
      // one timer in this capture-only page so both quality tiers can be photographed deliberately.
      await page.evaluate(() => {
        const nativeSetTimeout = window.setTimeout.bind(window);
        window.setTimeout = (callback, delay, ...args) => nativeSetTimeout(
          delay === 400 ? () => {} : callback, delay, ...args,
        );
      });
      await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
      const staged = await page.evaluate(([text, name]) => window.__ve.doc.sendText(text, name), [program, `G9 review ${fixture.name}`]);
      assert(staged.pending, `${fixture.name} staged: ${JSON.stringify(staged)}`);
      await page.waitForFunction(() => window.__ve.evaluation.state.revision === 1
        && JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'preview', null, { timeout: 180000 });
      const previewFailure = await page.evaluate(() => window.__ve.evaluation.state.lastFailure);
      const bounds = await page.evaluate(() => window.__ve.evaluation.bounds);
      assert(bounds, `${fixture.name}: preview has visible bounds${previewFailure ? ` (${previewFailure.code})` : ''}`);
      assert.equal(previewFailure, null, `${fixture.name}: Preview evaluation succeeded`);
      const centre = fixture.target;
      const distance = fixture.distance;
      await page.addStyleTag({ content: '#doc-panel,#viewer-hud,#viewer-gizmo-controls,#viewer-gizmo,#crosshair,#enter-vr{display:none!important}' });
      const frame = async (view, direction, quality) => {
        const directionLength = Math.hypot(...direction);
        const eye = direction.map((component, axis) => centre[axis] + component / directionLength * distance);
        assert(await page.evaluate(([camera, target]) => window.__runtime.viewer_look_at(...camera, ...target), [eye, centre]),
          `${fixture.name}/${view}: fixed camera accepted`);
        if (quality === 'preview') {
          await page.evaluate(() => window.__ve.evaluation.request('preview'));
          await page.waitForFunction(() => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'preview'
            && window.__ve.evaluation.state.worker === 'idle', null, { timeout: 180000 });
        }
        const frames = await page.evaluate(() => window.__ve.read().frames);
        await page.waitForFunction(start => window.__ve.read().frames >= start + 3, frames, { timeout: 30000 });
        const filename = `${fixture.name}-${view}-${quality}.png`;
        const image = filename;
        await page.screenshot({ path: `${reviewDir}${filename}` });
        const state = await page.evaluate(() => ({
          triangles: window.__ve.read().render.main.triangles,
          stats: window.__ve.evaluation.state.stats,
          quality: JSON.parse(window.__runtime.eval_projection_status()).committed?.quality,
        }));
        assert.equal(state.quality, quality, `${fixture.name}/${view}: captured the requested ${quality} projection`);
        assert(state.triangles > 0, `${fixture.name}/${view}/${quality}: scene rendered triangles`);
        captures.push({ view, cameraPosition: eye, cameraTarget: centre, quality, image,
          renderedTriangles: state.triangles, evaluationWallMs: state.stats?.elapsed_ms ?? null });
      };
      for (const [view, direction] of views) await frame(view, direction, 'preview');
      await page.evaluate(() => window.__ve.evaluation.request('final'));
      await page.waitForFunction(() => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final'
        && window.__ve.evaluation.state.worker === 'idle', null, { timeout: 180000 });
      for (const [view, direction] of views) await frame(view, direction, 'final');
      const file = fixture.file ?? 'engine/crates/ve-modeling/kits/{desk,shelf,window}.txt + generated scatter';
      manifest.fixtures.push({ name: fixture.name, file, sha256: sha256(program), previewFailure, totalWallMs: performance.now() - started, captures });
      console.log(`G9 review ${fixture.name}: ${captures.length} fixed-view captures`);
    } finally {
      await page.close();
    }
  }
  await browser.close();
  browser = null;
  const room = await readFile(`${root}engine/crates/ve-modeling/tests/gallery/visual/study_room.txt`, 'utf8');
  const kit = await sourceFor(fixtures.find(fixture => fixture.name === 'kit-composition'));
  const comparison = async ({ id, title, description, before, after, target, distance }) => {
    const beforeCapture = await comparisonCapture({ source: before, name: `${id}-before`, target, distance, image: `${id}-before.png` });
    const afterCapture = await comparisonCapture({ source: after, name: `${id}-after`, target, distance, image: `${id}-after.png` });
    manifest.comparisons.push({ id, title, description, before: beforeCapture, after: afterCapture });
  };
  manifest.comparisons = [];
  await comparison({
    id: 'b1-authored-light', title: 'B1 · authored light off/on',
    description: 'Same study-room geometry, material recipes, and three-quarter camera; the only change is the authored sun.',
    before: room, after: `${room}\n${light}`, target: [0, 0.9, 0], distance: 5.7,
  });
  const recipePattern = /recipe="[a-z_]+"/g;
  const recipeFreeRoom = room.replace(recipePattern, 'recipe="none"');
  assert.notEqual(recipeFreeRoom, room, 'study-room comparison fixture contains procedural material recipes');
  await comparison({
    id: 'b5-material-recipes', title: 'B5 · procedural recipes off/on',
    description: 'Same study-room geometry, authored sun, and camera; procedural recipes are set to none in the baseline.',
    before: `${recipeFreeRoom}\n${light}`, after: `${room}\n${light}`, target: [0, 0.9, 0], distance: 5.7,
  });
  const scatterPattern = /scatter_proto = [\s\S]*?output scatter = scatter_place\n/;
  const kitWithoutScatter = kit.replace(scatterPattern, '');
  assert.notEqual(kitWithoutScatter, kit, 'kit-composition fixture contains the scatter feature');
  await comparison({
    id: 'b7-scatter', title: 'B7 · scatter off/on',
    description: 'Same desk, shelf, window, authored sun, and camera; only the seeded scatter field is added in the after image.',
    before: `${kitWithoutScatter}\n${light}`, after: `${kit}\n${light}`, target: [0, 0.85, 0.5], distance: 4.3,
  });
  assert.deepEqual(logs, [], 'no browser or shader errors');
  await writeFile(`${reviewDir}review-manifest.json`, JSON.stringify(manifest, null, 2));
  await writeFile(`${reviewDir}index.html`, reviewHtml(manifest));
  for (const fixture of manifest.fixtures) {
    await writeFile(`${reviewDir}${fixture.name}.html`, reviewHtml(manifest, fixture.name));
  }
  console.log(`G9 review package: ${manifest.fixtures.length} fixtures, ${manifest.fixtures.reduce((n, f) => n + f.captures.length, 0)} captures`);
} finally {
  if (browser) await browser.close();
  child.kill(); await child.exited;
}
