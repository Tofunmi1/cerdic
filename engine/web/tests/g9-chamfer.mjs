import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir, writeFile } from 'node:fs/promises';
import { driverAdvisory } from './support.mjs';

// B6: the chamfer operator reaches the Engine Viewer on furniture. A procedural stool is staged twice
// in its own page: once as sharp boxes and once with solid.chamfer applied to the union. The chamfered
// capture shows a flat facet on every convex edge and the rendered triangle count rises, while the
// concave leg-to-seat seams the Minkowski operation cannot reach stay sharp.
const root = new URL('../../../', import.meta.url).pathname;
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', '5200'], {
  cwd: root, stdout: 'ignore', stderr: 'inherit',
});
const browser = await chromium.launch({ args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
const reports = new URL('../../target/reports/g9/', import.meta.url).pathname;

const prop = `
seat = solid.box@1(size=[0.44, 0.06, 0.36])
seat_up = xform.move@1(seat, by=[0, 0.45, 0])
leg = solid.box@1(size=[0.045, 0.45, 0.045])
leg_a = xform.move@1(leg, by=[0.18, 0.225, 0.14])
legs_x = xform.mirror@1(leg_a, axis="x")
legs = xform.mirror@1(legs_x, axis="z")
stool = solid.union@1(seat_up, legs)
`;
const lights = `
sun_main = light.sun@1(direction=[-0.4, -1, -0.2], color=[1, 0.92, 0.82], intensity=3)
output main_light = sun_main
`;
const variants = [
  { name: 'sharp', body: `${prop}\noutput prop = stool\n` },
  { name: 'cut', body: `${prop}\nprop_cut = solid.chamfer@1(stool, width=0.01)\noutput prop = prop_cut\n` },
];
const logs = [];
const profile = {};
try {
  for (let i = 0; i < 100; i++) {
    try { if ((await fetch('http://127.0.0.1:5200')).ok) break; } catch {}
    await Bun.sleep(100);
  }
  await mkdir(reports, { recursive: true });
  for (const variant of variants) {
    const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
    page.on('pageerror', error => logs.push(error.message));
    page.on('console', message => {
      if (message.type() === 'error' && !driverAdvisory.test(message.text())) logs.push(message.text());
    });
    try {
      await page.goto(`http://127.0.0.1:5200/?viewer=1&eval=1&fov=75&docname=g9-chamfer-${variant.name}`);
      await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
      await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
      const staged = await page.evaluate(([source, name]) => window.__ve.doc.sendText(source, name),
        [`${variant.body}${lights}`, `G9 chamfer ${variant.name}`]);
      assert(staged.pending, `${variant.name} staged: ${JSON.stringify(staged)}`);
      await page.waitForFunction(
        () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final',
        null, { timeout: 180000 });
      assert(await page.evaluate(() => window.__runtime.viewer_look_at(0.6, 0.5, 0.62, 0, 0.24, 0)),
        `${variant.name}: review camera accepted`);
      const frameStart = await page.evaluate(() => window.__ve.read().frames);
      await page.waitForFunction(start => window.__ve.read().frames >= start + 12, frameStart, { timeout: 30000 });
      await page.screenshot({ path: `${reports}chamfer-${variant.name}.png` });
      const triangles = await page.evaluate(() => window.__ve.read().render.main.triangles);
      profile[variant.name] = { triangles };
      console.log(`G9 chamfer ${variant.name}: ${triangles} rendered triangles`);
    } finally {
      await page.close();
    }
  }
  assert(profile.cut.triangles > profile.sharp.triangles,
    `the chamfer adds facets: ${profile.cut.triangles} vs ${profile.sharp.triangles}`);
  assert.deepEqual(logs, [], 'no browser errors');
  await writeFile(`${reports}chamfer-profile.json`, JSON.stringify(profile, null, 2));
  console.log('G9 chamfer: sharp and chamfered stool captured; the chamfer raises the triangle count');
} finally {
  await browser.close(); child.kill(); await child.exited;
}
