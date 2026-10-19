import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { driverAdvisory } from './support.mjs';

// B3: the authored scene environment reaches the Engine Viewer. Each fixture is staged twice in its
// own page with the same camera and sun: once with no scene environment (the scene baseline ambient
// applies) and once with a strong warm sky/ground band and ground bounce.
const root = new URL('../../../', import.meta.url).pathname;
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', '5198'], {
  cwd: root, stdout: 'ignore', stderr: 'inherit',
});
const browser = await chromium.launch({ args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
const reports = new URL('../../target/reports/g9/', import.meta.url).pathname;
const lights = `
sun_main = light.sun@1(direction=[-0.4, -1, -0.2], color=[1, 0.92, 0.82], intensity=3)
output main_light = sun_main
`;
const environment = `
environment = scene.environment@1(sky_color=[0.72, 0.8, 0.95], ground_color=[0.6, 0.4, 0.24], intensity=0.9, reflection=0.2, bounce=0.9, bounce_color=[0.9, 0.55, 0.28])
output scene_environment = environment
`;
const fixtures = [
  { name: 'study-room', file: 'engine/crates/ve-modeling/tests/gallery/visual/study_room.txt', camera: [0, 2.7, 6.1, 0, 0.9, 0] },
  { name: 'pistol-prop', file: 'engine/crates/ve-modeling/tests/gallery/visual/pistol_prop.txt', camera: [0.12, 0.24, 0.42, 0, 0.075, 0] },
];
const logs = [];
const profile = {};
try {
  for (let i = 0; i < 100; i++) {
    try { if ((await fetch('http://127.0.0.1:5198')).ok) break; } catch {}
    await Bun.sleep(100);
  }
  await mkdir(reports, { recursive: true });
  for (const fixture of fixtures) {
    const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
    page.on('pageerror', error => logs.push(error.message));
    page.on('console', message => {
      if (message.type() === 'error' && !driverAdvisory.test(message.text())) logs.push(message.text());
    });
    try {
      await page.goto(`http://127.0.0.1:5198/?viewer=1&eval=1&fov=75&docname=g9-environment-${fixture.name}`);
      await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
      await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
      const source = await readFile(`${root}${fixture.file}`, 'utf8');
      const readCommitted = () => page.evaluate(() => JSON.parse(window.__runtime.eval_projection_status()).committed);
      const aim = async () => {
        const aimed = await page.evaluate(camera => window.__runtime.viewer_look_at(...camera), fixture.camera);
        assert(aimed, `${fixture.name}: engine viewer accepted the review camera`);
      };
      const shoot = async name => {
        const frameStart = await page.evaluate(() => window.__ve.read().frames);
        await page.waitForFunction(start => window.__ve.read().frames >= start + 12, frameStart, { timeout: 30000 });
        await page.screenshot({ path: `${reports}environment-${name}.png` });
      };
      const stage = async (text, label) => {
        const previous = await readCommitted();
        const staged = await page.evaluate(([source, name]) => window.__ve.doc.sendText(source, name), [text, label]);
        assert(staged.pending, `${label} staged: ${JSON.stringify(staged)}`);
        // Wait for the *new* revision to commit at Final, not the previous one that is still committed.
        await page.waitForFunction(prev => {
          const committed = JSON.parse(window.__runtime.eval_projection_status()).committed;
          return committed?.quality === 'final' && committed.revision !== prev?.revision;
        }, previous, { timeout: 180000 });
        await aim();
        return page.evaluate(() => JSON.parse(window.__runtime.eval_projection_status()));
      };

      // Before: no authored environment; the scene baseline ambient applies.
      const before = await stage(`${source}\n${lights}`, `${fixture.name} before`);
      assert.equal(before.committed?.quality, 'final', `${fixture.name}: before reached Final`);
      assert.equal(before.live_environment, null, `${fixture.name}: no environment is live before it is authored`);
      await shoot(`${fixture.name}-before`);

      // After: authoring the environment appends to the staged fixture and replaces the ambient.
      const after = await stage(environment, `${fixture.name} after`);
      assert.equal(after.committed?.quality, 'final', `${fixture.name}: after reached Final`);
      assert(after.live_environment, `${fixture.name}: the authored environment reached the committed viewport`);
      assert(Math.abs(after.live_environment.intensity - 0.9) < 1e-5, `${fixture.name}: environment intensity reached the renderer`);
      assert(Math.abs(after.live_environment.bounce - 0.9) < 1e-5, `${fixture.name}: ground bounce reached the renderer`);
      const sky = after.live_environment.sky_color;
      assert(sky.every((channel, index) => Math.abs(channel - [0.72, 0.8, 0.95][index]) < 1e-5),
        `${fixture.name}: sky colour reached the renderer`);
      await shoot(`${fixture.name}-after`);
      profile[fixture.name] = { before, after };
      console.log(`G9 environment ${fixture.name}: before revision ${before.committed.revision} (no environment); `
        + `after revision ${after.committed.revision} (intensity ${after.live_environment.intensity}, bounce ${after.live_environment.bounce})`);
    } finally {
      await page.close();
    }
  }
  assert.deepEqual(logs, [], 'no browser errors');
  await writeFile(`${reports}environment-profile.json`, JSON.stringify(profile, null, 2));
} finally {
  await browser.close(); child.kill(); await child.exited;
}
