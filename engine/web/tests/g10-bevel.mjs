import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir, writeFile } from 'node:fs/promises';
import { driverAdvisory } from './support.mjs';

// G10: Blender's bevel (poly.bevel_network) reaches the Engine Viewer. A sharp box and the same box with
// every edge rounded are staged in their own pages, evaluated to Final quality and captured. The rounded
// one must render more triangles and look different; the images land in target/reports/g10/.
const root = new URL('../../../', import.meta.url).pathname;
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', '5204'], {
  cwd: root, stdout: 'ignore', stderr: 'inherit',
});
// PLAYWRIGHT_CHANNEL=chrome runs the installed Google Chrome when Playwright's own Chromium is absent.
const browser = await chromium.launch({
  channel: process.env.PLAYWRIGHT_CHANNEL || undefined,
  args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'],
});
const reports = new URL('../../target/reports/g10/', import.meta.url).pathname;
const lights = `
sun_main = light.sun@1(direction=[-0.4, -1, -0.3], color=[1, 0.92, 0.82], intensity=3)
output main_light = sun_main
`;
const boxMesh = `
box = solid.box@1(size=[1, 1, 1])
triangles = poly.from_solid@1(box)
mesh = poly.merge_coplanar@1(triangles)
`;
const finish = source => `
solid = poly.to_solid@1(${source})
lit = solid.material@1(solid, color=[0.72, 0.5, 0.3], roughness=0.35, metallic=0.1)
output item = lit
`;
const variants = [
  { name: 'sharp', body: `${boxMesh}${finish('mesh')}` },
  {
    name: 'rounded',
    body: `${boxMesh}sharp_edges = poly.select@1(mesh, element="edge", by="sharp", angle=0.5)
shape = poly.bevel_network@1(mesh, sharp_edges, width=0.12, segments=6)
${finish('shape')}`,
  },
  {
    // The four connected top edges only: the layout the older poly.bevel_edges refuses.
    name: 'top-loop',
    body: `${boxMesh}top = poly.select@1(mesh, element="edge", by="box", box_min=[-1, 0.4, -1], box_max=[1, 0.6, 1])
shape = poly.bevel_network@1(mesh, top, width=0.15, segments=5)
${finish('shape')}`,
  },
];
const logs = [];
const profile = {};
try {
  for (let i = 0; i < 100; i++) {
    try { if ((await fetch('http://127.0.0.1:5204')).ok) break; } catch {}
    await Bun.sleep(100);
  }
  await mkdir(reports, { recursive: true });
  for (const variant of variants) {
    const page = await browser.newPage({ viewport: { width: 1000, height: 800 } });
    page.on('pageerror', error => logs.push(error.message));
    page.on('console', message => {
      if (message.type() === 'error' && !driverAdvisory.test(message.text())) logs.push(message.text());
    });
    try {
      await page.goto(`http://127.0.0.1:5204/?viewer=1&eval=1&fov=45&docname=g10-bevel-${variant.name}`);
      await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
      await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
      const staged = await page.evaluate(([text, name]) => window.__ve.doc.sendText(text, name),
        [`${variant.body}${lights}`, `G10 bevel ${variant.name}`]);
      assert(staged.pending, `${variant.name} staged: ${JSON.stringify(staged)}`);
      await page.waitForFunction(
        () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final',
        null, { timeout: 180000 });
      assert(await page.evaluate(() => window.__runtime.viewer_look_at(1.6, 1.3, 1.9, 0, 0, 0)),
        `${variant.name}: review camera accepted`);
      const frameStart = await page.evaluate(() => window.__ve.read().frames);
      await page.waitForFunction(start => window.__ve.read().frames >= start + 12, frameStart, { timeout: 30000 });
      await page.screenshot({ path: `${reports}bevel-${variant.name}.png` });
      const triangles = await page.evaluate(() => window.__ve.read().render.main.triangles);
      profile[variant.name] = { triangles };
      assert(triangles > 0, `${variant.name} rendered geometry`);
      console.log(`G10 bevel ${variant.name}: ${triangles} rendered triangles`);
    } finally {
      await page.close();
    }
  }
  assert(profile.rounded.triangles > profile.sharp.triangles * 5, 'rounding every edge adds geometry');
  assert(profile['top-loop'].triangles > profile.sharp.triangles, 'the top loop adds geometry');
  assert.deepEqual(logs, [], 'no browser errors');
  await writeFile(`${reports}bevel-profile.json`, JSON.stringify(profile, null, 2));
  console.log('G10 bevel: sharp, rounded and top-loop boxes captured');
} finally {
  await browser.close(); child.kill(); await child.exited;
}
