import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { driverAdvisory } from './support.mjs';

// B7: the shipped kits (desk, shelf, window) and a seeded scatter field reach the Engine Viewer. Each is
// staged in its own page from the kit text in crates/ve-modeling/kits, evaluated to Final quality, and
// captured. Two scatter pages with different seeds must differ; the same seed is deterministic by the
// native tests, so here we only check that the viewer renders the instanced field.
const root = new URL('../../../', import.meta.url).pathname;
const kits = new URL('../../crates/ve-modeling/kits/', import.meta.url);
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', '5202'], {
  cwd: root, stdout: 'ignore', stderr: 'inherit',
});
const browser = await chromium.launch({ args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
const reports = new URL('../../target/reports/g9/', import.meta.url).pathname;
const lights = `
sun_main = light.sun@1(direction=[-0.4, -1, -0.3], color=[1, 0.92, 0.82], intensity=3)
output main_light = sun_main
`;
const kitSource = name => readFile(new URL(`${name}.txt`, kits), 'utf8');
const scatter = seed => `
rock = solid.box@1(size=[0.12, 0.08, 0.1])
rock_lit = solid.material@1(rock, color=[0.45, 0.42, 0.38], roughness=0.8)
field = inst.scatter@1(rock_lit, count=120, width=3, depth=2, min_spacing=0.18, yaw_jitter=180deg, scale_jitter=0.5, seed=${seed})
output field = field
`;
const variants = [
  { name: 'desk', body: async () => `${await kitSource('desk')}\nitem = desk()\noutput item = item\n`, eye: [1.8, 1.4, 2.0], target: [0, 0.4, 0] },
  { name: 'shelf', body: async () => `${await kitSource('shelf')}\nitem = shelf()\noutput item = item\n`, eye: [1.4, 1.6, 2.4], target: [0, 0.9, 0] },
  { name: 'window', body: async () => `${await kitSource('window')}\nitem = window()\noutput item = item\n`, eye: [0.9, 0.5, 2.0], target: [0, 0, 0] },
  { name: 'scatter-a', body: async () => scatter(1), eye: [0, 3.2, 2.6], target: [0, 0, 0] },
  { name: 'scatter-b', body: async () => scatter(2), eye: [0, 3.2, 2.6], target: [0, 0, 0] },
];
const logs = [];
const profile = {};
try {
  for (let i = 0; i < 100; i++) {
    try { if ((await fetch('http://127.0.0.1:5202')).ok) break; } catch {}
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
      await page.goto(`http://127.0.0.1:5202/?viewer=1&eval=1&fov=60&docname=g9-kits-${variant.name}`);
      await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
      await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
      const source = `${await variant.body()}${lights}`;
      const staged = await page.evaluate(([text, name]) => window.__ve.doc.sendText(text, name),
        [source, `G9 kits ${variant.name}`]);
      assert(staged.pending, `${variant.name} staged: ${JSON.stringify(staged)}`);
      await page.waitForFunction(
        () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final',
        null, { timeout: 180000 });
      assert(await page.evaluate(([e, t]) => window.__runtime.viewer_look_at(...e, ...t), [variant.eye, variant.target]),
        `${variant.name}: review camera accepted`);
      const frameStart = await page.evaluate(() => window.__ve.read().frames);
      await page.waitForFunction(start => window.__ve.read().frames >= start + 12, frameStart, { timeout: 30000 });
      await page.screenshot({ path: `${reports}kits-${variant.name}.png` });
      const triangles = await page.evaluate(() => window.__ve.read().render.main.triangles);
      profile[variant.name] = { triangles };
      assert(triangles > 0, `${variant.name} rendered geometry`);
      console.log(`G9 kits ${variant.name}: ${triangles} rendered triangles`);
    } finally {
      await page.close();
    }
  }
  assert(profile['scatter-a'].triangles >= 120 * 12, 'the scatter renders every instance');
  assert.deepEqual(logs, [], 'no browser errors');
  await writeFile(`${reports}kits-profile.json`, JSON.stringify(profile, null, 2));
  console.log('G9 kits: desk, shelf, window and two scatter seeds captured');
} finally {
  await browser.close(); child.kill(); await child.exited;
}
