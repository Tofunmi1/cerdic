import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { readFile } from 'node:fs/promises';

// D1/D2: the wasm evaluator drapes a panel over a ball with exactly the bits the native evaluator does (hash from
// crates/ve-modeling/tests/cloth_drape_hash.rs). Run: bun web/tests/d1-drape-wasm.mjs (serves web/dist-bevy-gpu).
const EXPECTED = 2686809893;
const root = new URL('../../../', import.meta.url).pathname;
const program = await readFile(new URL('./fixtures/drape.txt', import.meta.url), 'utf8');
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', '5214', '--dir', process.env.VE_DIST_DIR || 'web/dist-bevy-gpu'], {
  cwd: root, stdout: 'ignore', stderr: 'inherit',
});
const browser = await chromium.launch({ channel: process.env.PLAYWRIGHT_CHANNEL || 'chromium', args: ['--enable-unsafe-webgpu', '--enable-features=Metal,WebGPU'] });
try {
  for (let i = 0; i < 100; i++) {
    try { if ((await fetch('http://127.0.0.1:5214')).ok) break; } catch {}
    await Bun.sleep(100);
  }
  const page = await browser.newPage({ viewport: { width: 900, height: 700 } });
  await page.goto('http://127.0.0.1:5214/?viewer=1&eval=1&docname=d1-drape');
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  const staged = await page.evaluate(text => window.__ve.doc.sendText(text, 'drape'), program);
  assert(staged.pending, `staged: ${JSON.stringify(staged).slice(0, 400)}`);
  await page.waitForFunction(
    () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final' || window.__ve.evaluation.state.failureCurrent,
    null, { timeout: 180000 });
  assert.equal(await page.evaluate(() => window.__ve.evaluation.state.lastFailure), null, 'evaluates');
  const result = await page.evaluate(() => {
    const part = window.__ve.evaluation.parts.find(p => p.name === 'cloth');
    const bits = new Uint32Array(new Float32Array(part.positions).buffer);
    let hash = 0x811c9dc5;
    for (const word of bits) for (let s = 0; s < 32; s += 8) hash = Math.imul(hash ^ ((word >>> s) & 255), 0x01000193) >>> 0;
    return { hash, points: part.positions.length / 3 };
  });
  console.log(`D1/D2 drape in wasm: ${result.points} points, hash ${result.hash} (native ${EXPECTED})`);
  assert.equal(result.points, 625);
  assert.equal(result.hash, EXPECTED, 'wasm and native drape bit for bit');
} finally {
  await browser.close(); child.kill(); await child.exited;
}
