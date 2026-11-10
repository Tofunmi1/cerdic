import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { chromium } from 'playwright';

const sceneDir = new URL('./', import.meta.url).pathname;
const engineDir = new URL('../../../../../', import.meta.url).pathname;
const outputDir = `${engineDir}target/reports/character-head/shots/`;
const port = Number(process.env.CHARACTER_SHOT_PORT ?? 5271);
mkdirSync(outputDir, { recursive: true });

const server = Bun.spawn(['node', 'tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', String(port), '--dir', 'web/dist-bevy-gpu'], {
  cwd: engineDir,
  stdout: 'ignore',
  stderr: 'ignore',
});
const browser = await chromium.launch({
  channel: process.env.PLAYWRIGHT_CHANNEL || 'chromium',
  args: ['--enable-unsafe-webgpu', '--enable-features=Metal,WebGPU'],
});

try {
  let ready = false;
  for (let i = 0; i < 100; i++) {
    try {
      ready = (await fetch(`http://127.0.0.1:${port}`)).ok;
      if (ready) break;
    } catch {}
    await Bun.sleep(100);
  }
  if (!ready) throw new Error(`viewer did not start on port ${port}`);

  const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
  const errors = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.goto(`http://127.0.0.1:${port}/?viewer=1&eval=1&docname=head_study`);
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  const text = readFileSync(`${sceneDir}world.txt`, 'utf8');
  const staged = await page.evaluate((source) => window.__ve.doc.sendText(source, 'head_study'), text);
  if (!staged.pending) throw new Error(`head study was not staged: ${JSON.stringify(staged).slice(0, 500)}`);
  await page.waitForFunction(() => {
    const status = JSON.parse(window.__runtime.eval_projection_status());
    return status.committed?.quality === 'final' || window.__ve.evaluation.state.failureCurrent;
  }, null, { timeout: 600000 });
  const failure = await page.evaluate(() => window.__ve.evaluation.state.lastFailure);
  if (failure) throw new Error(`head study evaluation failed: ${JSON.stringify(failure).slice(0, 500)}`);
  await page.addStyleTag({ content: 'body > *:not(canvas) { display: none !important; }' });

  const shots = JSON.parse(readFileSync(`${sceneDir}shots.json`, 'utf8')).views;
  for (const shot of shots) {
    await page.evaluate(([position, target, fov]) => {
      window.__runtime.viewer_configure(true, fov, false, 1);
      window.__runtime.viewer_look_at(...position, ...target);
    }, [shot.position, shot.target, shot.fov]);
    await page.waitForTimeout(600);
    writeFileSync(`${outputDir}${shot.name}.png`, await page.screenshot());
  }
  writeFileSync(`${outputDir}console-errors.json`, JSON.stringify(errors, null, 2));
  if (errors.length) throw new Error(`viewer console errors: ${errors.slice(0, 3).join('; ')}`);
  console.log(`captured ${shots.length} fixed views to ${outputDir}`);
} finally {
  await browser.close();
  server.kill();
  await server.exited;
}
