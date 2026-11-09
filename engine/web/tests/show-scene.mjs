// Renders a scene text file in the real page and saves pictures. Usage:
//   bun engine/web/tests/show-scene.mjs <scene.txt> <out-dir> "<name>:ex,ey,ez,tx,ty,tz[:wait_ms]" ...
// Each view is a camera position and target in metres; wait_ms lets animations play before the picture (default 1800).
import { mkdirSync, readFileSync } from 'node:fs';
import { chromium } from 'playwright';
import { driverAdvisory } from './support.mjs';

const [file, out, ...views] = process.argv.slice(2);
const root = new URL('../../../', import.meta.url).pathname;
const viewerScene = process.env.SCENE ?? 'stage';
mkdirSync(out, { recursive: true });
// DROP_OUTPUTS="regex" leaves out the `output` lines whose name matches, so a big scene can be looked at one group of parts at a time.
const dropOutputs = process.env.DROP_OUTPUTS ? new RegExp(process.env.DROP_OUTPUTS) : null;
const program = readFileSync(file, 'utf8').split('\n').filter(line => !(dropOutputs && /^output\s/.test(line) && dropOutputs.test(line.replace(/^output\s+/, '')))).join('\n');
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', '5240', '--dir', 'web/dist-bevy-gpu'], { cwd: root, stdout: 'ignore', stderr: 'ignore' });
const browser = await chromium.launch({ channel: process.env.PLAYWRIGHT_CHANNEL || 'chromium', args: ['--enable-unsafe-webgpu', '--enable-features=Metal,WebGPU'] });
const logs = [];
try {
  for (let i = 0; i < 100; i++) { try { if ((await fetch('http://127.0.0.1:5240')).ok) break; } catch {} await Bun.sleep(100); }
  const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
  page.on('pageerror', error => logs.push(error.message));
  page.on('console', message => {
    if (message.type() !== 'error' || driverAdvisory.test(message.text()) || message.location().url.endsWith('/favicon.ico')) return;
    logs.push(`${message.text()} ${message.location().url}`);
  });
  await page.goto(`http://127.0.0.1:5240/?viewer=1&eval=1&scene=${encodeURIComponent(viewerScene)}&docname=scene`);
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  const started = Date.now();
  const staged = await page.evaluate(text => window.__ve.doc.sendText(text, 'scene'), program);
  if (!staged.pending) { console.log('NOT STAGED', JSON.stringify(staged).slice(0, 1500)); process.exitCode = 1; }
  else {
    await page.waitForFunction(
      () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final' || window.__ve.evaluation.state.failureCurrent,
      null, { timeout: 600000 });
    const failure = await page.evaluate(() => window.__ve.evaluation.state.lastFailure);
    console.log('evaluated in', ((Date.now() - started) / 1000).toFixed(1), 's; failure:', JSON.stringify(failure)?.slice(0, 1500));
    const info = await page.evaluate(() => window.__ve.evaluation.parts.map(p => `${p.name}:${(p.indices.length / 3) | 0}${p.animation?.length ? `:${p.animation.length}tracks` : ''}`).join(' '));
    console.log('parts:', info.slice(0, 6000));
    // Bounds of every part, and the ground's height on a coarse grid (highest vertex near each point), so a layout is measured rather than guessed.
    const report = await page.evaluate((digits) => {
      const bounds = window.__ve.evaluation.parts.map(p => {
        const lo = [1e9, 1e9, 1e9], hi = [-1e9, -1e9, -1e9];
        for (let i = 0; i < p.positions.length; i += 3) for (let a = 0; a < 3; a++) { lo[a] = Math.min(lo[a], p.positions[i + a]); hi[a] = Math.max(hi[a], p.positions[i + a]); }
        return `${p.name} [${lo.map(v => v.toFixed(digits))}]..[${hi.map(v => v.toFixed(digits))}]`;
      });
      const ground = window.__ve.evaluation.parts.find(p => p.name === 'ground');
      const rows = [];
      if (ground) {
        const cell = 60, size = 600, n = size / cell + 1;
        const best = new Array(n * n).fill(null);
        for (let i = 0; i < ground.positions.length; i += 3) {
          const gx = Math.round(ground.positions[i] / cell), gz = Math.round(ground.positions[i + 2] / cell);
          if (gx < 0 || gz < 0 || gx >= n || gz >= n) continue;
          const k = gz * n + gx;
          if (best[k] === null || Math.abs(ground.positions[i] - gx * cell) + Math.abs(ground.positions[i + 2] - gz * cell) < best[k][1]) best[k] = [ground.positions[i + 1], Math.abs(ground.positions[i] - gx * cell) + Math.abs(ground.positions[i + 2] - gz * cell)];
        }
        for (let z = 0; z < n; z++) rows.push(`z=${z * cell}: ` + Array.from({ length: n }, (_, x) => (best[z * n + x] ? best[z * n + x][0] : NaN).toFixed(0).padStart(4)).join(''));
      }
      return { bounds, rows };
    }, Number(process.env.BOUNDS_DIGITS ?? 0));
    console.log(report.bounds.join('\n'));
    console.log('ground height (rows z, columns x every 60 m):\n' + report.rows.join('\n'));
    // The `h` key hides the page's panels the way the viewer itself does (hiding elements by CSS also hides canvases the texture baker draws into).
    await page.waitForTimeout(1500);
    console.log('warnings:', JSON.stringify(await page.evaluate(() => window.__ve.evaluation.warnings ?? window.__ve.evaluation.state?.warnings ?? null))?.slice(0, 1200));
    console.log('live textures:', JSON.stringify(await page.evaluate(() => window.__ve.liveTextures?.())));
    await page.addStyleTag({ content: 'body > *:not(canvas) { display: none !important; }' });
    for (const view of views) {
      const [name, camera, wait, at] = view.split(':');
      const [ex, ey, ez, tx, ty, tz] = camera.split(',').map(Number);
      // With a fourth field the animation restarts from zero for this view and `wait` is how many milliseconds of it play before the picture.
      if (at !== undefined) await page.evaluate(() => window.__runtime.viewer_animation(true, 0));
      await page.evaluate(([e, t]) => window.__runtime.viewer_look_at(e[0], e[1], e[2], t[0], t[1], t[2]), [[ex, ey, ez], [tx, ty, tz]]);
      await page.waitForTimeout(Number(wait ?? 1800));
      await page.screenshot({ path: `${out}/${name}.png` });
      console.log('picture', `${out}/${name}.png`);
    }
  }
  if (logs.length) console.log('console errors:', logs.slice(0, 5).join(' | ').slice(0, 800));
} finally { await browser.close(); child.kill(); await child.exited; }
