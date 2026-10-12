// G3 acceptance in the browser: the gallery programs go text -> staged commands -> worker -> scene,
// the vase survives the create / widen / manual edit / undo / reload sequence, a wall-breaking edit
// is refused with the document untouched, and every gallery piece is photographed from the fixed
// review views (docs/modeling/fixtures.md) into target/reports/gallery/ for review by eye.
import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir, readFile } from 'node:fs/promises';
import { PNG } from 'pngjs';
import { driverAdvisory } from './support.mjs';

const root = new URL('../../../', import.meta.url).pathname;
const child = Bun.spawn(['bun', 'x', 'vite', '--host', '127.0.0.1', '--port', '5189'], {
  cwd: root, stdout: 'ignore', stderr: 'inherit',
});
const browser = await chromium.launch({ args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
const out = new URL('../../target/reports/gallery/', import.meta.url).pathname;
const programs = new URL('../../crates/ve-modeling/tests/gallery/', import.meta.url).pathname;
const logs = [];

// name, centre, size (metres) and the feature the detail view looks at
const pieces = {
  vase: { centre: [0, 0.15, 0], size: 0.3, detail: [0, 0.3, 0] },
  twisted_tower: { centre: [0, 0.25, 0], size: 0.5, detail: [0, 0.5, 0] },
  offset_plate: { centre: [0, 0.0035, 0], size: 0.16, detail: [0.05, 0.006, 0.03], detailScale: 0.6 },
};

const eye = (centre, distance, azimuth, elevation) => {
  const a = azimuth * Math.PI / 180, e = elevation * Math.PI / 180;
  return [centre[0] + distance * Math.sin(a) * Math.cos(e), centre[1] + distance * Math.sin(e), centre[2] + distance * Math.cos(a) * Math.cos(e)];
};
const views = (piece) => {
  const d = piece.size * 2.4;
  return {
    front: [eye(piece.centre, d, 0, 0), piece.centre],
    right: [eye(piece.centre, d, 90, 0), piece.centre],
    top: [eye(piece.centre, d, 0, 89), piece.centre],
    'three-quarter': [eye(piece.centre, d, 35, 20), piece.centre],
    detail: [eye(piece.detail, d * (piece.detailScale ?? 0.35), 35, 25), piece.detail],
    underside: [eye(piece.centre, d, 0, -60), piece.centre],
  };
};

try {
  for (let i = 0; i < 100; i++) {
    try { if ((await fetch('http://127.0.0.1:5189')).ok) break; } catch {}
    await Bun.sleep(100);
  }
  await mkdir(out, { recursive: true });
  const page = await browser.newPage({ viewport: { width: 1200, height: 800 } });
  page.on('pageerror', error => logs.push(error.message));
  page.on('console', message => {
    if (message.type() === 'error' && !driverAdvisory.test(message.text())) logs.push(message.text());
  });
  const idle = () => page.waitForFunction(() => window.__spatial?.ready
    && window.__spatial.evaluation.state.worker === 'idle', null, { timeout: 60000 });
  const open = async (docname) => {
    await page.goto(`http://127.0.0.1:5189/?docname=${docname}&evaltest=1`);
    await idle();
  };
  const frames = async (n = 6) => {
    const start = await page.evaluate(() => JSON.parse(window.__spatial.runtime.status()).frames);
    await page.waitForFunction(count => JSON.parse(window.__spatial.runtime.status()).frames > count, start + n);
  };

  // ---- every gallery piece: text in, geometry out, six fixed views
  for (const [name, piece] of Object.entries(pieces)) {
    await open(`g3-gallery-${name}-${Date.now()}`);
    const text = await readFile(`${programs}${name}.txt`, 'utf8');
    const sent = await page.evaluate(t => window.__spatial.doc.sendText(t, `Gallery: ${t.slice(0, 20)}`), text);
    assert(sent.pending, `${name}: the text reads and stages (${JSON.stringify(sent).slice(0, 200)})`);
    await page.waitForFunction(() => window.__spatial.evaluation.state.revision === 1, null, { timeout: 60000 });
    await idle();
    const state = await page.evaluate(() => ({ failure: window.__spatial.evaluation.state.lastFailure, nodes: window.__spatial.doc.state().nodes,
      projection: JSON.parse(window.__spatial.runtime.eval_projection_status()) }));
    assert.equal(state.failure ?? null, null, `${name}: no evaluation failure`);
    assert(state.projection.live_meshes >= 1, `${name}: geometry projected`);
    const printed = await page.evaluate(() => window.__spatial.doc.text());
    assert(printed.text?.includes('output'), `${name}: the document prints back as text`);
    const reread = await page.evaluate(t => window.__spatial.runtime.doc_parse_text_standalone(t), printed.text);
    assert(JSON.parse(reread).commands?.length > 0, `${name}: printed text reads again`);
    await page.evaluate(() => window.__spatial.runtime.viewer_configure(true, 40, false, 2));
    for (const [view, [position, target]] of Object.entries(views(piece))) {
      assert(await page.evaluate(([p, t]) => window.__spatial.runtime.viewer_look_at(...p, ...t), [position, target]), `${name}/${view}: pose accepted`);
      await frames();
      const file = `${out}${name}-${view}.png`;
      await page.locator('canvas#ve').screenshot({ path: file });
      const png = PNG.sync.read(await readFile(file));
      let lit = 0;
      for (let p = 0; p < png.data.length; p += 4) {
        const [r, g, b] = [png.data[p], png.data[p + 1], png.data[p + 2]];
        if (Math.abs(r - 26) + Math.abs(g - 51) + Math.abs(b - 102) > 40) lit++;
      }
      assert(lit / (png.width * png.height) > 0.004, `${name}/${view}: the piece is visible (${(100 * lit / (png.width * png.height)).toFixed(2)} percent of pixels)`);
    }
    console.log(`G3 gallery ${name}: six views captured`);
  }

  // ---- the vase sequence: create, widen, manual edit, undo, reload; a wall-breaking edit is refused
  await open(`g3-sequence-${Date.now()}`);
  const vase = await readFile(`${programs}vase.txt`, 'utf8');
  await page.evaluate(t => window.__spatial.doc.sendText(t, 'make a hollow vase'), vase);
  await page.waitForFunction(() => window.__spatial.evaluation.state.revision === 1, null, { timeout: 60000 });
  await idle();
  const ids = await page.evaluate(() => window.__spatial.doc.state().document.nodes.map(n => `${n.label}:${n.id}`));
  assert.equal(ids.length, 6, 'the vase has six nodes');
  const node = label => page.evaluate(l => window.__spatial.doc.state().document.nodes.find(n => n.label === l), label);
  const widen = async () => {
    const [outer, inner] = [await node('outer'), await node('inner')];
    const move = (n, by) => { const pts = structuredClone(n.params.points); pts[pts.length - 2][0] += by; return { set_param: { node: n.id, param: 'points', value: pts } }; };
    return [move(outer, 0.02), move(inner, 0.024)];
  };
  await page.evaluate(commands => window.__spatial.doc.send(commands, 'widen its opening'), await widen());
  await page.waitForFunction(() => window.__spatial.evaluation.state.revision === 2, null, { timeout: 60000 });
  await idle();
  const afterWiden = await page.evaluate(() => window.__spatial.doc.state().document.nodes.map(n => `${n.label}:${n.id}`));
  assert.deepEqual(afterWiden, ids, 'widening keeps every node and ID');
  const segments = await page.evaluate(() => window.__spatial.doc.state().document.nodes.find(n => n.label === 'body').id);
  await page.evaluate(id => window.__spatial.doc.send([{ set_param: { node: id, param: 'segments', value: 96 } }], 'inspector: segments'), segments);
  await page.waitForFunction(() => window.__spatial.evaluation.state.revision === 3, null, { timeout: 60000 });
  await idle();
  await page.evaluate(() => window.__spatial.doc.undo());
  await page.waitForFunction(() => window.__spatial.evaluation.state.revision === 4, null, { timeout: 60000 });
  await idle();
  assert.equal(await page.evaluate(() => window.__spatial.doc.state().document.nodes.find(n => n.label === 'body').params.segments), 64, 'undo restored segments');
  await page.evaluate(() => window.__spatial.doc.save());
  const saved = await page.evaluate(() => window.__spatial.doc.state().document);
  await page.reload();
  await idle();
  const reloaded = await page.evaluate(() => window.__spatial.doc.state().document);
  assert.deepEqual(reloaded.nodes, saved.nodes, 'reload gives the same nodes, IDs and parameters');
  console.log('G3 vase sequence: create, widen, edit, undo, reload');

  const before = await page.evaluate(() => window.__spatial.doc.state().document.nodes);
  const innerNode = await node('inner');
  // Move the inner rim point to 1 mm inside the outer one: the wall near the top drops to about 1 mm.
  const outerRim = (await node('outer')).params.points.at(-2)[0];
  const wider = structuredClone(innerNode.params.points); wider[wider.length - 2][0] = outerRim - 0.001;
  await page.evaluate(([id, pts]) => window.__spatial.doc.send([{ set_param: { node: id, param: 'points', value: pts } }], 'break the wall'), [innerNode.id, wider]);
  try {
    await page.waitForFunction(() => window.__spatial.evaluation.state.lastFailure?.code === 'constraint.min_wall', null, { timeout: 30000 });
  } catch (error) {
    console.error('wall refusal not seen', JSON.stringify(await page.evaluate(() => window.__spatial.evaluation.state)).slice(0, 800), logs);
    throw error;
  }
  await idle();
  assert.deepEqual(await page.evaluate(() => window.__spatial.doc.state().document.nodes), before, 'the refused edit left the document unchanged');
  console.log('G3 wall constraint refused the edit');
  assert.deepEqual(logs, [], 'no page errors');
  console.log('G3 gallery acceptance passed');
} finally {
  await browser.close();
  child.kill();
}
