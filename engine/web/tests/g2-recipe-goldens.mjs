import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { PNG } from 'pngjs';
import { GOLDEN, differingFraction, driverAdvisory } from './support.mjs';

// G2: a golden picture for every recipe (kit). Each is built with its default arguments in the page, framed from one fixed three-quarter view that depends only on its measured size, and
// compared with `web/tests/golden/recipes/<name>.png`. A recipe whose picture drifts fails by name; a missing golden is recorded (with UPDATE_GOLDEN=1 every golden is re-recorded) and the run
// says so. Each picture must also show the recipe (not an empty room), and every recipe must differ from every other (the pictures are not all the same).
// Run: bun web/tests/g2-recipe-goldens.mjs [name ...]
const root = new URL('../../../', import.meta.url).pathname;
const goldenDir = new URL('./golden/recipes/', import.meta.url).pathname;
const reports = new URL('../../target/reports/g2/', import.meta.url).pathname;
const kitsRs = await readFile(new URL('../../crates/ve-modeling/src/kits.rs', import.meta.url), 'utf8');
const entry = /name: "([a-z_0-9]+)",\s*doc: "(?:[^"\\]|\\.)*",\s*source: include_str!\("\.\.\/kits\/([a-z_0-9]+)\.txt"\)/g;
const kits = [];
for (const [, name, file] of kitsRs.matchAll(entry)) {
  const source = await readFile(new URL(`../../crates/ve-modeling/kits/${file}.txt`, import.meta.url), 'utf8');
  // Only group kits (called like `item = name()`) are recipes with a picture; macro preludes are tested elsewhere.
  if (source.startsWith('group ')) kits.push({ name, source });
}
const wanted = process.argv.slice(2);
const chosen = wanted.length ? kits.filter(k => wanted.includes(k.name)) : kits;
assert(chosen.length >= (wanted.length || 27), `found ${chosen.length} recipes`);
await mkdir(goldenDir, { recursive: true });
await mkdir(reports, { recursive: true });
const update = process.env.UPDATE_GOLDEN === '1';
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', '5229', '--dir', process.env.VE_DIST_DIR || 'web/dist-bevy-gpu'], { cwd: root, stdout: 'ignore', stderr: 'inherit' });
const browser = await chromium.launch({ channel: process.env.PLAYWRIGHT_CHANNEL || 'chromium', args: ['--enable-unsafe-webgpu', '--enable-features=Metal,WebGPU'] });
const logs = [];
const recorded = [], drifted = [], empty = [];
const pictures = new Map();
try {
  for (let i = 0; i < 100; i++) { try { if ((await fetch('http://127.0.0.1:5229')).ok) break; } catch {} await Bun.sleep(100); }
  const page = await browser.newPage({ viewport: { width: 640, height: 360 } });
  page.on('pageerror', error => logs.push(error.message));
  page.on('console', message => {
    if (message.type() !== 'error' || driverAdvisory.test(message.text()) || message.location().url.endsWith('/favicon.ico')) return;
    logs.push(`${message.text()} ${message.location().url}`);
  });
  for (const kit of chosen) {
    await page.goto(`http://127.0.0.1:5229/?viewer=1&eval=1&docname=g2-${kit.name}`);
    await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
    await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
    const staged = await page.evaluate(text => window.__ve.doc.sendText(text, 'recipe'), `${kit.source}\nitem = ${kit.name}()\noutput item\n`);
    assert(staged.pending, `${kit.name}: staged: ${JSON.stringify(staged).slice(0, 400)}`);
    await page.waitForFunction(
      () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final' || window.__ve.evaluation.state.failureCurrent,
      null, { timeout: 120000 });
    assert.equal(await page.evaluate(() => window.__ve.evaluation.state.lastFailure), null, `${kit.name} evaluates`);
    // One framing rule for all: look at the middle of the measured box from a three-quarter angle, far enough to see all of it.
    const box = await page.evaluate(() => {
      const lo = [Infinity, Infinity, Infinity], hi = [-Infinity, -Infinity, -Infinity];
      for (const part of window.__ve.evaluation.parts) for (let i = 0; i < part.positions.length; i += 3) for (let a = 0; a < 3; a++) { lo[a] = Math.min(lo[a], part.positions[i + a]); hi[a] = Math.max(hi[a], part.positions[i + a]); }
      return { lo, hi };
    });
    const mid = box.lo.map((v, a) => (v + box.hi[a]) / 2);
    const reach = Math.max(...box.hi.map((v, a) => v - box.lo[a])) * 1.7 + 0.5;
    const eye = [mid[0] + reach * 0.62, mid[1] + reach * 0.42, mid[2] + reach * 0.78].map(v => Math.round(v * 1000) / 1000);
    await page.evaluate(([e, m]) => window.__runtime.viewer_look_at(e[0], e[1], e[2], m[0], m[1], m[2]), [eye, mid]);
    // Only the canvas: the page's panels, toolbar and timeline would put their own text and layout into every golden.
    await page.addStyleTag({ content: 'body > *:not(canvas) { display: none !important; }' });
    await page.waitForTimeout(1800);
    const bytes = await page.screenshot({ path: `${reports}${kit.name}.png` });
    const picture = PNG.sync.read(bytes);
    pictures.set(kit.name, picture);
    const goldenPath = `${goldenDir}${kit.name}.png`;
    if (update || !existsSync(goldenPath)) {
      await writeFile(goldenPath, bytes);
      recorded.push(kit.name);
      continue;
    }
    const golden = PNG.sync.read(await readFile(goldenPath));
    const bad = differingFraction(picture, golden);
    if (bad > GOLDEN.maxBadFraction) drifted.push(`${kit.name} (${(bad * 100).toFixed(2)}% of pixels differ)`);
  }
  // The pictures show something: the share of pixels unlike the picture's own corner colours.
  for (const [name, p] of pictures) {
    let unlike = 0, total = 0;
    const corner = [0, 1, 2].map(c => p.data[c]);
    for (let i = 0; i < p.data.length; i += 16) { total++; if (Math.max(...[0, 1, 2].map(c => Math.abs(p.data[i + c] - corner[c]))) > 25) unlike++; }
    if (unlike / total < 0.04) empty.push(`${name} (${(unlike / total * 100).toFixed(1)}% differs from the background)`);
  }
  console.log(`G2 recipe goldens: ${chosen.length} recipes, ${recorded.length} recorded${recorded.length ? ` (${recorded.join(', ')})` : ''}, ${drifted.length} drifted, ${empty.length} empty`);
  assert.deepEqual(empty, [], 'every picture shows its recipe');
  assert.deepEqual(drifted, [], 'every recipe still looks like its golden (run with UPDATE_GOLDEN=1 to accept a change you meant)');
  if (chosen.length > 5) {
    // Different recipes look different: no two goldens are the same picture.
    const names = [...pictures.keys()];
    for (let i = 0; i < names.length; i++) for (let j = i + 1; j < names.length; j++) {
      assert(differingFraction(pictures.get(names[i]), pictures.get(names[j])) > 0.01, `${names[i]} and ${names[j]} look alike`);
    }
  }
  console.log('G2 browser errors:', JSON.stringify(logs));
  assert.deepEqual(logs, [], 'no browser errors');
} finally { await browser.close(); child.kill(); await child.exited; }
