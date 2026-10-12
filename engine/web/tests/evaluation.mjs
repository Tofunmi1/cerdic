// G2 acceptance uses the real worker, kernel, renderer and Rust-owned candidate document.
import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir, writeFile } from 'node:fs/promises';
import { driverAdvisory } from './support.mjs';

const child = Bun.spawn(['bun', 'x', 'vite', '--host', '127.0.0.1', '--port', '5188'], {
  cwd: new URL('../../../', import.meta.url).pathname, stdout: 'ignore', stderr: 'inherit',
});
const browser = await chromium.launch({ args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
const reports = new URL('../../target/reports/', import.meta.url).pathname;
const logs = [];
try {
  for (let i = 0; i < 100; i++) {
    try { if ((await fetch('http://127.0.0.1:5188')).ok) break; } catch {}
    await Bun.sleep(100);
  }
  const page = await browser.newPage({ viewport: { width: 1100, height: 750 } });
  page.on('pageerror', error => logs.push(error.message));
  page.on('console', message => {
    if (message.type() === 'error' && !driverAdvisory.test(message.text())) logs.push(message.text());
  });
  const url = 'http://127.0.0.1:5188/?docname=g2-acceptance&evaltest=1';
  await page.goto(url);
  const idle = async () => {
    try { await page.waitForFunction(() => window.__spatial?.ready
      && window.__spatial.evaluation.state.worker === 'idle', null, { timeout: 30000 }); }
    catch (error) {
      console.error('Worker wait failed', JSON.stringify(await page.evaluate(() => ({
        ready:window.__spatial?.ready, error:window.__spatial?.error,
        evaluation:window.__spatial?.evaluation?.state,
        projection:window.__spatial?.runtime && JSON.parse(window.__spatial.runtime.eval_projection_status()),
      }))), logs);
      throw error;
    }
  };
  await idle();
  console.log('G2 initial worker ready');
  await page.getByRole('button', { name: 'Add box' }).click();
  await page.waitForFunction(() => window.__spatial.doc.state().revision === 1);
  await idle();
  assert.equal(await page.evaluate(() => window.__spatial.evaluation.state.revision), 1, 'g2_01 projected revision');
  console.log('G2 first projection committed');
  await page.evaluate(() => {
    for (let i = 0; i < 10; i++) window.__spatial.doc.send([
      { add_node: { op: 'solid.box@1', params: { size: [1+i/100, 1, 1] }, label: `box-${i}` } },
    ], `Independent edit ${i}`);
  });
  await page.waitForFunction(() => window.__spatial.doc.state().nodes === 11);
  await idle();
  assert.equal(await page.evaluate(() => window.__spatial.doc.state().revision), 11, 'g2_04 all rapid edits retained');
  console.log('G2 rapid edits committed');
  const conflict = await page.evaluate(() => {
    const state = window.__spatial.doc.state();
    return window.__spatial.doc.apply({ doc: state.id, base_revision: 1, transaction: 't_conflict',
      commands: [{ set_param: { node: state.document.nodes[0].id, param: 'size', value: [2,2,2] } }] });
  });
  assert.equal(conflict.conflict.current_revision, 11, 'g2_05 stale batch conflicts');
  const before = await page.evaluate(() => JSON.parse(window.__spatial.runtime.eval_projection_status()));
  await page.evaluate(() => window.__spatial.evaluation.request('final'));
  await page.waitForFunction(() => JSON.parse(window.__spatial.runtime.eval_projection_status()).committed?.quality === 'final');
  await idle();
  const stale = await page.evaluate(async () => {
    const host = window.__spatial.evaluation;
    const token = JSON.parse(window.__spatial.runtime.eval_projection_status()).committed;
    await host.message({ kind:'result', token:{...token, quality:'preview', request:token.request-1}, parts:[], bytes:0, stats:{} });
    await host.message({ kind:'result', token:{...token, generation:token.generation-1}, parts:[], bytes:0, stats:{} });
    return host.state.staleResults;
  });
  assert(stale >= 2, 'g2_06/g2_07 stale preview and generation discarded');
  console.log('G2 stale results rejected');
  await page.evaluate(() => {
    window.__g2Pacing = { gaps: [], previous: performance.now(), running: true };
    const frame = time => {
      const state = window.__g2Pacing;
      state.gaps.push(time - state.previous); state.previous = time;
      if (state.running) requestAnimationFrame(frame);
    };
    requestAnimationFrame(frame);
  });
  await page.evaluate(() => window.__spatial.evaluation.request('preview', 'spin'));
  const firstFrame = await page.evaluate(() => JSON.parse(window.__spatial.runtime.status()).frames);
  await page.getByLabel('Command JSON').fill('window remains responsive');
  await page.waitForFunction(n => JSON.parse(window.__spatial.runtime.status()).frames > n + 10, firstFrame);
  await page.waitForFunction(() => window.__spatial.evaluation.state.lastFailure?.code === 'eval.worker_timeout', null, { timeout: 20000 });
  await idle();
  const pacing = await page.evaluate(() => {
    const state = window.__g2Pacing; state.running = false;
    const gaps = state.gaps.slice(1).sort((a,b) => a-b);
    return { frames:gaps.length, medianMs:gaps[Math.floor(gaps.length/2)],
      p95Ms:gaps[Math.floor(gaps.length*0.95)], maxMs:Math.max(...gaps) };
  });
  assert(pacing.frames > 10, 'g2_13 frame pacing recorded during blocked worker');
  assert.equal(await page.evaluate(() => window.__spatial.doc.state().nodes), 11, 'g2_02 timeout retained document');
  console.log('G2 timeout recovered');
  await page.evaluate(() => window.__spatial.evaluation.request('preview', 'crash'));
  await page.waitForFunction(() => window.__spatial.evaluation.state.lastFailure?.code === 'eval.worker_crash');
  await idle();
  assert.equal(await page.evaluate(() => window.__spatial.doc.state().nodes), 11, 'g2_03 crash retained scene');
  console.log('G2 crash recovered');
  assert.match(await page.getByTestId('evaluation-status').innerText(), /eval.worker_crash/, 'g2_14 visible failure status');
  await page.evaluate(() => window.__spatial.doc.send([
    { add_node: { op:'solid.sphere@1', params:{ radius:1, segments:512 } } },
  ], 'Over budget preview'));
  await page.waitForFunction(() => window.__spatial.evaluation.state.lastFailure?.code === 'budget.triangles');
  await idle();
  assert.equal(await page.evaluate(() => window.__spatial.doc.state().nodes), 11, 'g2_10 rejected before committing');
  const warm = await page.evaluate(() => window.__spatial.evaluation.state.stats);
  console.log('G2 budget rejection checked');
  for (let i = 0; i < 100; i++) {
    await page.evaluate(() => window.__spatial.evaluation.request());
    await idle();
  }
  const soak = await page.evaluate(() => ({ eval:window.__spatial.evaluation.state,
    projection:JSON.parse(window.__spatial.runtime.eval_projection_status()) }));
  assert(soak.eval.stats.cache_bytes <= 128*1024*1024, 'g2_09 cache cap');
  assert.equal(soak.eval.stats.cache_bytes, warm.cache_bytes, 'g2_15 warm cache bounded');
  assert(soak.projection.max_frame_bytes <= 8*1024*1024, 'g2_11 per-frame bytes bounded');
  assert.equal(soak.projection.live_meshes, before.live_meshes, 'g2_15 live mesh count bounded');
  await page.evaluate(() => window.__spatial.doc.save());
  await page.evaluate(() => {
    window.__spatial.doc.send([{ add_node: { op:'solid.sphere@1', params:{radius:1,segments:512}, label:'Large transfer' } }], 'Large final result');
    window.__spatial.evaluation.request('final');
  });
  await page.waitForFunction(() => window.__spatial.doc.state().nodes === 12, null, { timeout:120000 });
  await idle();
  const large = await page.evaluate(() => ({ eval:window.__spatial.evaluation.state,
    projection:JSON.parse(window.__spatial.runtime.eval_projection_status()) }));
  assert(large.eval.transferBytes > 100000, 'large result transfer measured');
  assert(large.projection.upload_frames > soak.projection.upload_frames + 1, 'g2_11 large upload spans frames');
  assert(large.projection.max_frame_bytes <= 8*1024*1024, 'g2_11 hard upload byte cap');
  await page.evaluate(() => window.__spatial.doc.undo());
  await idle();
  await page.evaluate(() => window.__spatial.evaluation.request('preview','spin'));
  await page.reload(); await idle();
  assert.equal(await page.evaluate(() => window.__spatial.doc.state().nodes), 11, 'g2_12 reload kept saved scene');
  await mkdir(reports, { recursive: true });
  await writeFile(`${reports}g2-evaluation.json`, JSON.stringify({soak, large, pacing}, null, 2));
  await page.screenshot({ path: `${reports}g2-editor.png` });
  assert.deepEqual(logs, [], 'no page errors');
  console.log('G2 worker acceptance passed');
} finally { await browser.close(); child.kill(); await child.exited; }
