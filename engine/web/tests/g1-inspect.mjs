import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { driverAdvisory } from './support.mjs';

// G1: the value inspector. In the real page (the wasm evaluator in its worker), the `inspect` tool answers "why did this node give this" for a small program:
// a wall that is the wrong height because a number linked into it says so, and the chain upstream is followed back to the node that set the number.
const root = new URL('../../../', import.meta.url).pathname;
const program = `h = value.number@1(value=3.5)
half = value.math@1(op="div", a=h, b=2)
wall_box = solid.box@1(size=[4, 1, 0.3], size.y=half)
lifted = xform.move@1(wall_box, by=[0, 1, 0])
wall = solid.material@1(lifted, color=[0.7, 0.6, 0.5], roughness=0.8)
output wall = wall
`;
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', '5228', '--dir', process.env.VE_DIST_DIR || 'web/dist-bevy-gpu'], { cwd: root, stdout: 'ignore', stderr: 'inherit' });
const browser = await chromium.launch({ channel: process.env.PLAYWRIGHT_CHANNEL || 'chromium', args: ['--enable-unsafe-webgpu', '--enable-features=Metal,WebGPU'] });
const logs = [];
try {
  for (let i = 0; i < 100; i++) { try { if ((await fetch('http://127.0.0.1:5228')).ok) break; } catch {} await Bun.sleep(100); }
  const page = await browser.newPage({ viewport: { width: 900, height: 700 } });
  page.on('pageerror', error => logs.push(error.message));
  page.on('console', message => {
    if (message.type() !== 'error' || driverAdvisory.test(message.text()) || message.location().url.endsWith('/favicon.ico')) return;
    logs.push(`${message.text()} ${message.location().url}`);
  });
  await page.goto('http://127.0.0.1:5228/?viewer=1&eval=1&docname=g1-inspect');
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  const staged = await page.evaluate(text => window.__ve.doc.sendText(text, 'inspect'), program);
  assert(staged.pending, `staged: ${JSON.stringify(staged).slice(0, 500)}`);
  await page.waitForFunction(
    () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final' || window.__ve.evaluation.state.failureCurrent,
    null, { timeout: 120000 });
  assert.equal(await page.evaluate(() => window.__ve.evaluation.state.lastFailure), null, 'evaluates');

  const overview = await page.evaluate(() => window.__ve.tool('inspect', {}));
  console.log('G1 overview:', JSON.stringify(overview.nodes.map(n => `${n.node}:${n.kind}`)));
  assert.deepEqual(overview.nodes.map(n => n.node), ['h', 'half', 'wall_box', 'lifted', 'wall'], 'every node in run order');
  assert.equal(overview.nodes[0].kind, 'number');

  const why = await page.evaluate(() => window.__ve.tool('inspect', { node: 'wall_box' }));
  console.log('G1 why wall_box:', why.why);
  assert.equal(why.operator, 'solid.box@1');
  assert(Math.abs(why.facts.size[1] - 1.75) < 1e-6, 'the wall is 1.75 m tall: half of the number 3.5');
  assert.deepEqual(why.upstream.map(n => n.node), ['half', 'h'], 'followed back to the number that set it');
  assert(why.upstream[1].summary.includes('3.5'), 'the number it came from is shown');
  assert(why.inputs.some(i => i.from === 'half'), 'the number linked into size.y is an input');

  const lifted = await page.evaluate(() => window.__ve.tool('inspect', { node: 'lifted' }));
  assert(lifted.changed.includes('centre y 0 to 2.75 m') || lifted.changed.some(c => c.startsWith('centre y')), `moving is described: ${JSON.stringify(lifted.changed)}`);
  const material = await page.evaluate(() => window.__ve.tool('inspect', { node: 'wall' }));
  assert(material.changed.includes('gave it a material'), JSON.stringify(material.changed));
  const wrong = await page.evaluate(() => window.__ve.tool('inspect', { node: 'wal' }));
  assert.equal(wrong.error.code, 'inspect.unknown_node');
  assert(wrong.error.suggestion.includes('wall'));
  console.log('G1 browser errors:', JSON.stringify(logs));
  assert.deepEqual(logs, [], 'no browser errors');
} finally { await browser.close(); child.kill(); await child.exited; }
