// Drives the spike: for each effect set and each backend, loads the page fresh, waits for frames, and records what happened.
// Usage: bun run.mjs <webgl|webgpu> [fx-set ...]   (an fx-set is the text after fx=, "" for the baseline)
import { chromium } from 'playwright';
import { readFile, mkdir } from 'node:fs/promises';
import { extname, join } from 'node:path';
import { PNG } from 'pngjs';

const root = new URL(`./web-${process.argv[2] ?? 'webgl'}/`, import.meta.url).pathname;
const out = new URL('./results/', import.meta.url).pathname;
await mkdir(out, { recursive: true });
const backend = process.argv[2] ?? 'webgl';
const sets = process.argv.slice(3);
const defaults = [
  '', 'hdr', 'tonemap=agx', 'tonemap=aces', 'tonemap=neutral', 'bloom', 'taa', 'smaa', 'fxaa', 'cas', 'ssao', 'ssr', 'dof', 'mb',
  'exposure', 'atmo', 'atmo,atmoenv', 'point', 'spot', 'point,spot', 'stereo', 'stereo,taa,bloom',
];
const runs = sets.length ? sets : defaults;

const types = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm' };
const server = Bun.serve({
  port: 0,
  async fetch(request) {
    const path = new URL(request.url).pathname;
    try {
      const body = await readFile(join(root, path === '/' ? 'index.html' : path));
      return new Response(body, { headers: { 'content-type': types[extname(path)] ?? 'application/octet-stream' } });
    } catch { return new Response('not found', { status: 404 }); }
  },
});

const real = process.env.REAL_GPU === '1';
const args = real ? ['--enable-unsafe-webgpu', '--enable-features=Metal,WebGPU', '--ignore-gpu-blocklist'] : backend === 'webgpu'
  ? ['--enable-unsafe-webgpu', '--enable-features=Vulkan,WebGPU', '--use-angle=swiftshader', '--use-webgpu-adapter=swiftshader', '--ignore-gpu-blocklist', '--enable-unsafe-swiftshader']
  : ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'];
const browser = await chromium.launch(real ? { args, channel: 'chromium', headless: true } : { args });
const results = [];
for (const fx of runs) {
  const page = await browser.newPage({ viewport: { width: 800, height: 520 } });
  const messages = [];
  page.on('pageerror', e => messages.push(`pageerror: ${e.message.slice(0, 300)}`));
  page.on('console', m => {
    if (process.env.DEBUG) console.log(`  [${m.type()}] ${m.text().slice(0, 400)}`);
    if (['error', 'warning'].includes(m.type()) && !m.text().includes('Failed to load resource')) messages.push(`${m.type()}: ${m.text().slice(0, 300)}`);
  });
  page.on('crash', () => messages.push('PAGE CRASHED'));
  page.on('close', () => { if (process.env.DEBUG) console.log('  [page closed]'); });
  const began = Date.now();
  await page.goto(`http://localhost:${server.port}/index.html?fx=${fx}`);
  let frames = 0, error = null;
  try {
    await page.waitForFunction(() => window.__spike.frames >= 30 || window.__spike.error, null, { timeout: Number(process.env.WAIT ?? 120000) });
    ({ frames, error } = await page.evaluate(() => window.__spike));
  } catch { frames = await page.evaluate(() => window.__spike?.frames ?? -1).catch(() => -1); error = error ?? 'timeout'; }
  await page.waitForTimeout(1500);
  const png = PNG.sync.read(await page.screenshot({ path: `${out}${backend}-${fx.replace(/[^a-z0-9]+/gi, '_') || 'baseline'}.png` }));
  let sum = 0, lit = 0;
  for (let i = 0; i < png.data.length; i += 4) { const v = png.data[i] + png.data[i + 1] + png.data[i + 2]; sum += v; if (v > 30) lit++; }
  const mean = sum / (png.data.length / 4) / 3;
  const unique = [...new Set(messages)].slice(0, 4);
  const verdict = error || frames < 30 ? 'FAIL' : (lit / (png.data.length / 4) < 0.05 ? 'BLANK' : (unique.some(m => /error|panick|validation/i.test(m)) ? 'ERRORS' : 'ok'));
  results.push({ backend, fx: fx || '(baseline)', verdict, frames, mean: Number(mean.toFixed(1)), seconds: Number(((Date.now() - began) / 1000).toFixed(1)), messages: unique });
  console.log(JSON.stringify(results.at(-1)));
  await page.close();
}
await browser.close();
server.stop();
