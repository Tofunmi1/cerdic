// Regression: a legacy saved scene must never appear while Studio restores a project.
// Uses one headless browser and the real engine; requires the Vite server.
import { chromium } from 'playwright';
import { expect as baseExpect } from '@playwright/test';
const expect = baseExpect.configure({ timeout: 60000 });
const browser = await chromium.launch({ headless: true, args: [process.platform === 'darwin' ? '--use-angle=metal' : '--use-angle=swiftshader', '--enable-unsafe-webgpu'] });
const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
page.setDefaultTimeout(30000);
try {
  await page.goto('http://127.0.0.1:5173/studio?projects=1');
  await page.getByLabel('Project name', { exact: true }).fill('Startup regression');
  await page.getByRole('button', { name: 'Create project', exact: true }).click();
  await expect(page.getByRole('dialog')).toBeHidden();
  await page.evaluate(async () => { await window.__spatial.doc.sendText('project_marker = solid.box@1(size=[1,2,1])\noutput project_marker', 'startup-regression'); });
  await page.waitForFunction(() => window.__spatial.doc.state().nodes > 0 && !window.__spatial.doc.state().pending_document);
  await expect(page.locator('.project-save-state')).toHaveText('Saved locally');
  const expectedDocument = await page.evaluate(() => window.__spatial.doc.state().id);
  // Reproduce the old default document that used to be shown before project restoration.
  await page.evaluate(async () => {
    const host = window.__spatial;
    host.runtime.doc_reset('spatial-lab', 1); host.evaluation.request();
    while (host.evaluation.state.worker !== 'idle') await new Promise(resolve => setTimeout(resolve, 50));
    await host.doc.sendText('legacy_tractor_marker = solid.box@1(size=[8,8,8])\noutput legacy_tractor_marker', 'legacy-seed');
    while (host.doc.state().pending_document || host.evaluation.state.worker !== 'idle') await new Promise(resolve => setTimeout(resolve, 50));
    host.doc.save();
  });
  await page.addInitScript(({ expectedDocument }) => {
    window.__startupViolations = []; window.__startupVisibleFrames = 0;
    const projectReload = new URLSearchParams(location.search).has('project');
    const observe = () => {
      const host = window.__spatial, canvas = document.querySelector('.stage canvas');
      if (host?.doc) {
        const state = host.doc.state();
        if (state.document.nodes.some(node => node.label === 'legacy_tractor_marker')) window.__startupViolations.push('Legacy document restored');
        if (canvas && getComputedStyle(canvas).visibility === 'visible') {
          window.__startupVisibleFrames++;
          const projection = JSON.parse(host.runtime.eval_projection_status());
          if (projectReload && (state.id !== expectedDocument || projection.committed?.document !== expectedDocument)) window.__startupViolations.push('Canvas exposed before requested project projection');
          if (!projectReload && projection.live_meshes > 0) window.__startupViolations.push('Unrequested scene visible behind project picker');
        }
      }
      requestAnimationFrame(observe);
    };
    requestAnimationFrame(observe);
  }, { expectedDocument });
  for (let reload = 0; reload < 2; reload++) {
    await page.reload();
    await expect(page.locator('.stage canvas')).toBeVisible();
    await page.waitForFunction(() => window.__startupVisibleFrames > 5);
    expect(await page.evaluate(() => window.__startupViolations)).toEqual([]);
    expect(await page.evaluate(() => window.__spatial.doc.text().text)).toContain('project_marker');
  }
  for (const url of ['/studio?projects=1', '/studio']) {
    await page.goto(`http://127.0.0.1:5173${url}`);
    await expect(page.getByRole('dialog', { name: 'A home for every world.' })).toBeVisible();
    await page.waitForFunction(() => window.__startupVisibleFrames > 5);
    expect(await page.evaluate(() => window.__startupViolations)).toEqual([]);
    expect(await page.evaluate(() => window.__spatial.doc.state().nodes)).toBe(0);
  }
  console.log('PASS: two project reloads, project picker load, and plain Studio load never restore or expose a legacy saved scene.');
} finally { await browser.close(); }
