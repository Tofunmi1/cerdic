import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir, writeFile } from 'node:fs/promises';
import { PNG } from 'pngjs';
import { driverAdvisory } from './support.mjs';

// G10: baked texture maps show in the Bevy viewport. The same unwrapped box is staged twice, once plain and
// once with a brick texture program (albedo and height); the viewport screenshots must differ clearly where
// the box is, and the textured one must show the pattern's colours (red brick, grey mortar), not one flat
// colour. The pictures are left in target/reports/g10/ to look at.
const root = new URL('../../../', import.meta.url).pathname;
const reports = new URL('../../target/reports/g10/', import.meta.url).pathname;
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', '5207'], {
  cwd: root, stdout: 'ignore', stderr: 'inherit',
});
const browser = await chromium.launch({
  channel: process.env.PLAYWRIGHT_CHANNEL || undefined,
  args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'],
});

const base = `box = solid.box@1(size=[1, 1, 1])
paint = solid.material@1(box, color=[1, 1, 1], roughness=0.7)
cage = poly.from_solid@1(paint)
flat = poly.merge_coplanar@1(cage)
seams = poly.select@1(flat, element="edge", by="sharp", angle=0.5)
uv = poly.unwrap_uv@1(flat, seams, method="angle", rotate=true)
solid = poly.to_solid@1(uv)
`;
const brick = 'b = bricks(columns=3, rows=6, mortar=0.08, bevel=0.05); albedo = colorize(b, gradient=[0:#6e6e6e, 0.45:#6e6e6e, 0.5:#a8432e, 1:#c4552f]); height = b';
const plain = `${base}output plain = solid\n`;
const textured = `${base}pattern = solid.texture@1(solid, program="${brick}")\noutput textured = pattern\n`;

const logs = [];
try {
  for (let i = 0; i < 100; i++) {
    try { if ((await fetch('http://127.0.0.1:5207')).ok) break; } catch {}
    await Bun.sleep(100);
  }
  await mkdir(reports, { recursive: true });
  // One fresh page per picture: a second document in the same page restarts revision numbers, which would
  // make "the new revision is committed" true of the old scene.
  const show = async (name, text) => {
    const page = await browser.newPage({ viewport: { width: 900, height: 700 } });
    page.on('pageerror', error => logs.push(error.message));
    page.on('console', message => {
      if (message.type() === 'warning' && /texture|bake|map/i.test(message.text())) console.log(`page warning: ${message.text().slice(0, 300)}`);
      if (message.type() === 'error' && !driverAdvisory.test(message.text())) logs.push(message.text());
    });
    await page.goto(`http://127.0.0.1:5207/?viewer=1&eval=1&docname=g10-viewport-${name}`);
    await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
    await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
    const staged = await page.evaluate(([source, label]) => window.__ve.doc.sendText(source, label), [text, name]);
    assert(staged.pending, `${name} staged: ${JSON.stringify(staged)}`);
    const revision = staged.pending.committed.revision;
    await page.waitForFunction(
      r => {
        const committed = JSON.parse(window.__runtime.eval_projection_status()).committed;
        return (committed?.quality === 'final' && committed?.revision === r) || window.__ve.evaluation.state.failureCurrent;
      },
      revision, { timeout: 180000 });
    const failure = await page.evaluate(() => window.__ve.evaluation.state.lastFailure);
    assert.equal(failure, null, `${name} evaluation: ${JSON.stringify(failure)}`);
    // Let the baked maps upload and a few frames render.
    await page.waitForTimeout(1500);
    const shot = await page.screenshot({ path: `${reports}viewport-${name}.png` });
    await page.close();
    return PNG.sync.read(shot);
  };
  const a = await show('plain', plain);
  const b = await show('textured', textured);

  // Where the picture differs from the plain box, count red-brick-like and mortar-grey-like pixels.
  let differing = 0, red = 0, grey = 0;
  for (let i = 0; i < a.data.length; i += 4) {
    const d = Math.max(Math.abs(a.data[i] - b.data[i]), Math.abs(a.data[i + 1] - b.data[i + 1]), Math.abs(a.data[i + 2] - b.data[i + 2]));
    if (d > 24) {
      differing++;
      const [r, g, bl] = [b.data[i], b.data[i + 1], b.data[i + 2]];
      if (r > g * 1.6 && r > bl * 1.6) red++;
      else if (Math.abs(r - g) < 14 && Math.abs(g - bl) < 14) grey++;
    }
  }
  const share = differing / (a.width * a.height);
  console.log(`G10 viewport texture: ${(share * 100).toFixed(1)}% of the picture changed; ${red} red-brick and ${grey} grey-mortar pixels among them`);
  // Over the shared array's capacity: 18 parts with four DISTINCT maps each (parts with an identical program and shape
  // would share maps and layers) need 72 of the 32 layers (1024 px each). The scene must
  // still show, and the author is told which parts lost maps (the exported file would still have them).
  const many = Array.from({ length: 18 }, (_, i) => `b${i} = solid.box@1(size=[0.2, 0.2, 0.2])\n`
    + `t${i} = solid.texture@1(b${i}, program="g = fbm(scale_x=${3 + i}, scale_y=4); albedo = g; roughness = g; height = g; emissive = g")\n`
    + `output o${i} = t${i}\n`).join('');
  const crowded = await browser.newPage({ viewport: { width: 900, height: 700 } });
  crowded.on('pageerror', error => logs.push(error.message));
  await crowded.goto('http://127.0.0.1:5207/?viewer=1&eval=1&docname=g10-viewport-budget');
  await crowded.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  await crowded.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  const crowdedStage = await crowded.evaluate(text => window.__ve.doc.sendText(text, 'budget'), many);
  assert(crowdedStage.pending, `budget staged: ${JSON.stringify(crowdedStage).slice(0, 300)}`);
  await crowded.waitForFunction(
    () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final' || window.__ve.evaluation.state.failureCurrent,
    null, { timeout: 240000 });
  const crowdedState = await crowded.evaluate(() => ({ failure: window.__ve.evaluation.state.lastFailure, parts: window.__ve.evaluation.parts.length,
    warnings: window.__ve.evaluation.state.warnings.filter(w => w.code === 'texture.viewport_budget').map(w => w.message) }));
  assert.equal(crowdedState.failure, null, `crowded scene evaluates: ${JSON.stringify(crowdedState.failure)}`);
  assert.equal(crowdedState.parts, 18);
  assert(crowdedState.warnings.length >= 1 && crowdedState.warnings.length <= 18, `some parts lose maps: ${crowdedState.warnings.length}`);
  assert(crowdedState.warnings[0].includes('.glb'), 'the warning says the export is unaffected');
  console.log(`G10 viewport texture budget: 18 textured parts, ${crowdedState.warnings.length} told they lost maps`);
  await crowded.close();

  assert(share > 0.04, 'the textured box looks clearly different from the plain one');
  assert(red > 500, 'red brick shows in the viewport');
  assert(grey > 100, 'grey mortar shows in the viewport');
  assert.deepEqual(logs, [], 'no browser errors');
} finally {
  await browser.close(); child.kill(); await child.exited;
}
