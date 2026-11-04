import assert from 'node:assert/strict';
import { mkdirSync, writeFileSync } from 'node:fs';
import { chromium } from 'playwright';
import { driverAdvisory } from './support.mjs';
import { buildImport } from '../../tools/mcp/import-file.mjs';

// G5: the user's own mesh file. A small house written as a Z-up OBJ in millimetres (a file somebody made elsewhere: quads, a roof of triangles and
// texture coordinates) goes through the same path the `import_file` tool uses (file -> poly.import line -> apply_text). In the real page it must
// arrive 4 x 3 x 2.8 m tall in Y, stand on the floor, and then be editable like any mesh: a door is cut with poly.inset_faces + extrude.
const root = new URL('../../../', import.meta.url).pathname;
const scratch = '/tmp/claude-g5/';
mkdirSync(scratch, { recursive: true });
// Footprint 4000 x 3000 mm, walls 2000 mm, ridge 800 mm higher along X. Z is up in the file.
const obj = `# a house in millimetres, Z up
v 0 0 0
v 4000 0 0
v 4000 3000 0
v 0 3000 0
v 0 0 2000
v 4000 0 2000
v 4000 3000 2000
v 0 3000 2000
v 0 1500 2800
v 4000 1500 2800
vt 0 0
vt 1 0
vt 1 1
vt 0 1
f 1/1 4/2 3/3 2/4
f 1/1 2/2 6/3 5/4
f 2/1 3/2 7/3 6/4
f 3/1 4/2 8/3 7/4
f 4/1 1/2 5/3 8/4
f 5/1 6/2 10/3 9/4
f 7/1 8/2 9/3 10/4
f 6/1 7/2 10/3
f 8/1 5/2 9/3
`;
const file = `${scratch}house.obj`;
writeFileSync(file, obj);
const built = buildImport({ path: file, scale: 0.001, up: 'z', output: false });
assert(!built.error, built.error);
// The imported mesh is then edited like any other: the front wall (facing +Z here, after the Z-up file is turned to Y up) is pushed out 0.4 m.
const original = `${built.text}shown = solid.material@1(house, color=[0.8, 0.7, 0.55], roughness=0.8)
output imported = shown
`;
const edited = `${built.text}front = poly.select@1(house, element="face", by="normal", direction=[0, 0, 1], angle=0.3)
porch = poly.extrude_faces@1(house, front, offset=[0, 0, 0.4])
shown = solid.material@1(porch, color=[0.8, 0.7, 0.55], roughness=0.8)
output imported = shown
`;

const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', '5231', '--dir', process.env.VE_DIST_DIR || 'web/dist-bevy-gpu'], { cwd: root, stdout: 'ignore', stderr: 'ignore' });
const browser = await chromium.launch({ channel: process.env.PLAYWRIGHT_CHANNEL || 'chromium', args: ['--enable-unsafe-webgpu', '--enable-features=Metal,WebGPU'] });
const logs = [];
try {
  for (let i = 0; i < 100; i++) { try { if ((await fetch('http://127.0.0.1:5231')).ok) break; } catch {} await Bun.sleep(100); }
  const page = await browser.newPage({ viewport: { width: 900, height: 700 } });
  page.on('pageerror', error => logs.push(error.message));
  page.on('console', message => {
    if (message.type() !== 'error' || driverAdvisory.test(message.text()) || message.location().url.endsWith('/favicon.ico')) return;
    logs.push(`${message.text()} ${message.location().url}`);
  });
  const evaluate = async (text, label) => {
    await page.goto('http://127.0.0.1:5231/?viewer=1&eval=1&docname=g5-import');
    await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
    await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
    const staged = await page.evaluate(([t, l]) => window.__ve.doc.sendText(t, l), [text, label]);
    assert(staged.pending, `staged: ${JSON.stringify(staged).slice(0, 500)}`);
    await page.waitForFunction(
      () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final' || window.__ve.evaluation.state.failureCurrent,
      null, { timeout: 120000 });
  };
  const box = () => page.evaluate(() => {
    const lo = [Infinity, Infinity, Infinity], hi = [-Infinity, -Infinity, -Infinity];
    for (const part of window.__ve.evaluation.parts) for (let i = 0; i < part.positions.length; i += 3) for (let a = 0; a < 3; a++) { lo[a] = Math.min(lo[a], part.positions[i + a]); hi[a] = Math.max(hi[a], part.positions[i + a]); }
    return { lo, hi };
  });

  // 1. The file arrives in metres, Y up.
  await evaluate(original, 'import house');
  assert.equal(await page.evaluate(() => window.__ve.evaluation.state.lastFailure), null, 'evaluates');
  const size = await box();
  const dims = size.hi.map((v, a) => v - size.lo[a]);
  console.log('G5 imported size (m):', dims.map(v => v.toFixed(3)).join(' x '), 'low corner', size.lo.map(v => v.toFixed(3)).join(','));
  assert(Math.abs(dims[0] - 4) < 1e-3 && Math.abs(dims[1] - 2.8) < 1e-3 && Math.abs(dims[2] - 3) < 1e-3, `4 x 2.8 x 3 m (file X, Z, Y), got ${dims}`);
  assert(Math.abs(size.lo[1]) < 1e-3, 'the house stands on the floor (file z=0 is y=0)');
  const part = await page.evaluate(() => { const p = window.__ve.evaluation.parts[0]; return { triangles: p.indices.length / 3, uvs: p.uvs.length / 2 }; });
  assert.equal(part.triangles, 16, 'seven quads and two gable triangles');
  assert.equal(part.uvs, 48, 'the file\'s texture coordinates arrive, one per triangle corner');
  const look = () => page.evaluate(() => window.__runtime.viewer_look_at(6.5, 4.2, 8, 2, 1.2, -1.5));
  await look();
  await page.addStyleTag({ content: 'body > *:not(canvas) { display: none !important; }' });
  await page.waitForTimeout(1800);
  await page.screenshot({ path: `${scratch}house.png` });

  // 2. The same mesh, edited like any other: the front wall is pushed out, so the house grows 0.4 m in depth.
  await evaluate(edited, 'edit house');
  assert.equal(await page.evaluate(() => window.__ve.evaluation.state.lastFailure), null, 'the edit evaluates');
  const grown = await box();
  const depth = grown.hi[2] - grown.lo[2];
  console.log('G5 depth after extruding the front wall:', depth.toFixed(3), 'm');
  assert(Math.abs(depth - 3.4) < 1e-3, `3 m + 0.4 m, got ${depth}`);
  await look();
  await page.addStyleTag({ content: 'body > *:not(canvas) { display: none !important; }' });
  await page.waitForTimeout(1800);
  await page.screenshot({ path: `${scratch}house-edited.png` });
  console.log('G5 pictures:', `${scratch}house.png`, `${scratch}house-edited.png`);
  assert.deepEqual(logs, [], 'no console errors');
} finally { await browser.close(); child.kill(); await child.exited; }
