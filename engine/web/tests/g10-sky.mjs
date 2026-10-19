import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { chromium } from 'playwright';
import { mkdir } from 'node:fs/promises';
import { PNG } from 'pngjs';
import { driverAdvisory } from './support.mjs';

// The authored sky is rendered in the normal viewer pass used by mono, mock stereo and WebXR views.
const root = new URL('../../../', import.meta.url).pathname;
const reports = new URL('../../target/reports/g10/', import.meta.url).pathname;
const iwerBundle = fs.readFileSync(path.join(root, 'engine/web/node_modules/iwer/build/iwer.min.js'), 'utf8');
const port = 5232;
const server = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', String(port)], { cwd: root, stdout: 'ignore', stderr: 'inherit' });
const browser = await chromium.launch({ channel: process.env.PLAYWRIGHT_CHANNEL || undefined, args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
const logs = [];
const noon = `sky = scene.sky@1(sun_elevation=70, sun_azimuth=0, turbidity=2, clouds=false, seed=3)
output sky = sky`;
const sunset = `floor = solid.box@1(size=[8,0.2,6])
base = xform.move@1(floor, by=[0,-0.1,0])
ground = solid.material@1(base, color=[0.52,0.45,0.36], roughness=0.85)
output ground = ground
block = solid.box@1(size=[1.2,1.2,1.2])
left = xform.move@1(block, by=[-1.5,0.6,0])
right = xform.move@1(block, by=[1.5,0.6,0])
left_surface = solid.material@1(left, color=[0.7,0.7,0.7], roughness=0.8)
right_surface = solid.material@1(right, color=[0.7,0.7,0.7], roughness=0.8)
output left = left_surface
output right = right_surface
occluder = solid.box@1(size=[0.25,2,2])
shade = xform.move@1(occluder, by=[0,1,0])
output shade = shade
sky = scene.sky@1(sun_elevation=9, sun_azimuth=90, sun_intensity=8, turbidity=9, clouds=false)
output sky = sky`;
try {
  for (let i = 0; i < 100; i++) { try { if ((await fetch(`http://127.0.0.1:${port}`)).ok) break; } catch {} await Bun.sleep(100); }
  await mkdir(reports, { recursive: true });
  let page = await browser.newPage({ viewport: { width: 1000, height: 760 } });
  const watch = target => {
    target.on('pageerror', error => logs.push(error.message));
    target.on('console', message => { if (message.type() === 'error' && !driverAdvisory.test(message.text())) logs.push(message.text()); });
  };
  watch(page);
  await page.goto(`http://127.0.0.1:${port}/?viewer=1&eval=1&scene=stage&docname=g10-sky`);
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  const stage = async (text, label) => {
    const result = await page.evaluate(([source, name]) => window.__ve.doc.sendText(source, name), [text, label]);
    assert(result.pending, `staged ${label}: ${JSON.stringify(result)}`);
    await page.waitForFunction(() => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final' || window.__ve.evaluation.state.failureCurrent, null, { timeout: 180000 });
    assert.equal(await page.evaluate(() => window.__ve.evaluation.state.lastFailure), null, `${label} evaluates`);
  };
  await stage(noon, 'noon sky');
  assert(await page.evaluate(() => window.__runtime.viewer_look_at(0, 0, 4, 0, 0, 0)), 'sky review camera accepted');
  let canvas = page.locator('canvas').first();
  const noonPng = PNG.sync.read(await canvas.screenshot({ path: `${reports}sky-noon.png` }));
  const pixel = (png, x, y) => { const i = (Math.floor(y) * png.width + Math.floor(x)) * 4; return [...png.data.slice(i, i + 3)]; };
  const top = pixel(noonPng, noonPng.width * 0.5, noonPng.height * 0.22);
  const horizon = pixel(noonPng, noonPng.width * 0.5, noonPng.height * 0.5);
  console.log(`G10 sky noon: overhead RGB ${top}; horizon RGB ${horizon}`);
  const blueRatio = rgb => rgb[2] / Math.max(rgb[1], 1);
  assert(blueRatio(top) > blueRatio(horizon) + 0.04, 'noon sky is bluer overhead than at the horizon');

  await page.close();
  page = await browser.newPage({ viewport: { width: 1000, height: 760 } });
  watch(page);
  canvas = page.locator('canvas').first();
  await page.goto(`http://127.0.0.1:${port}/?viewer=1&eval=1&scene=stage&docname=g10-sky-sunset`);
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  await stage(sunset, 'sunset scene');
  assert(await page.evaluate(() => window.__runtime.viewer_look_at(4.8, 4.1, 7.5, 0, 0.7, 0)), 'sunset review camera accepted');
  await page.waitForTimeout(400);
  const sunsetPng = PNG.sync.read(await canvas.screenshot({ path: `${reports}sky-sunset.png` }));
  const warm = [], cool = [];
  for (let y = sunsetPng.height * 0.25; y < sunsetPng.height * 0.8; y++) for (let x = sunsetPng.width * 0.2; x < sunsetPng.width * 0.8; x++) {
    const [r,g,b] = pixel(sunsetPng,x,y), lightness = (r+g+b)/3;
    if (lightness > 105 && r > b * 1.08) warm.push([r,g,b]);
    if (lightness > 35 && lightness < 125 && b > r * 0.98) cool.push([r,g,b]);
  }
  const mean = list => list.reduce((out,p) => out.map((v,i)=>v+p[i]/list.length),[0,0,0]);
  const lit = mean(warm), shadow = mean(cool);
  console.log(`G10 sky sunset: ${warm.length} warm lit pixels RGB ${lit.map(Math.round)}; ${cool.length} darker cool pixels RGB ${shadow.map(Math.round)}`);
  const state = await page.evaluate(() => JSON.parse(window.__runtime.eval_projection_status()));
  assert(state.live_environment?.sky && state.live_lights > 0, 'sky drives an authored directional sun');
  assert(warm.length > 100 && cool.length > 100, 'sunset capture contains lit and shadow-side samples');
  assert(lit[0] / Math.max(lit[2], 1) > shadow[0] / Math.max(shadow[2], 1) + 0.04, 'the sunset lit side is warmer than the shadow side');

  // IWER provides the Quest 3 WebXR frame loop without requiring a physical headset.
  const xrPage = await browser.newPage({ viewport: { width: 1000, height: 760 } });
  page = xrPage;
  watch(xrPage);
  await xrPage.addInitScript({ content: iwerBundle });
  await xrPage.addInitScript(() => {
    const device = new window.IWER.XRDevice(window.IWER.metaQuest3);
    device.installRuntime({ forceInstall: true });
    device.stereoEnabled = true;
    window.__xr = device;
  });
  await xrPage.goto(`http://127.0.0.1:${port}/?scene=xr&eval=1&xr=auto&docname=g10-sky-xr`);
  await xrPage.waitForFunction(() => window.__ve?.ready && window.__ve.xrSupported, null, { timeout: 90000 });
  await xrPage.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  await stage(noon, 'XR noon sky');
  await xrPage.waitForFunction(() => window.__ve.read().xr.state === 'Presenting' && window.__ve.read().xr.frames >= 8, null, { timeout: 60000 });
  const xrFrame = await xrPage.evaluate(() => window.__ve.read().xr);
  assert(xrFrame.view, `mock XR view is missing: ${JSON.stringify(xrFrame)}`);
  assert.equal(xrFrame.view.viewports.length, 2, 'the mock headset has two rendered eyes');
  await xrPage.waitForTimeout(100);
  const xrPng = PNG.sync.read(await xrPage.screenshot({ path: `${reports}sky-xr.png` }));
  for (const [eye, viewport] of xrFrame.view.viewports.entries()) {
    const [x, y, width, height] = viewport;
    const leftX = Math.floor(x + width * 0.5);
    const top = pixel(xrPng, leftX, y + height * 0.22);
    const horizon = pixel(xrPng, leftX, y + height * 0.5);
    assert(blueRatio(top) > blueRatio(horizon) + 0.02, `mock XR eye ${eye} renders the authored sky gradient`);
  }
  assert.deepEqual(logs, [], 'no browser errors');
} finally { await browser.close(); server.kill(); await server.exited; }
