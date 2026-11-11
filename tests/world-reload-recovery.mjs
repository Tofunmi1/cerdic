// Real-engine regression: recover committed text when IndexedDB saving fails or reload beats its debounce.
import { chromium } from 'playwright';
import { expect as baseExpect } from '@playwright/test';
const expect = baseExpect.configure({ timeout: 60000 });
const browser = await chromium.launch({ headless: true, args: [process.platform === 'darwin' ? '--use-angle=metal' : '--use-angle=swiftshader', '--enable-unsafe-webgpu'] });
const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
try {
  await page.goto('http://127.0.0.1:5173/studio?projects=1');
  await page.getByLabel('Project name', { exact: true }).fill('Reload recovery');
  await page.getByRole('button', { name: 'Create project', exact: true }).click();
  await expect(page.getByRole('dialog')).toBeHidden();
  const projectId = new URL(page.url()).searchParams.get('project');
  await page.getByRole('textbox', { name: 'Message', exact: true }).fill('Keep this project and its world on reload.');
  await page.getByRole('button', { name: 'Send', exact: true }).click();
  await page.evaluate(projectId => {
    const original = IDBObjectStore.prototype.put;
    IDBObjectStore.prototype.put = function(value, ...args) {
      if (this.name === 'projects' && value.id === projectId && value.text.includes('recovered_box')) throw new DOMException('Injected storage failure', 'QuotaExceededError');
      return original.call(this, value, ...args);
    };
  }, projectId);
  await page.evaluate(async () => { await window.__spatial.doc.sendText('recovered_box = solid.box@1(size=[2,3,1])\noutput recovered_box', 'recovery-regression'); });
  await expect(page.locator('.project-save-state')).toHaveText('Save problem');
  const sourceBeforeReload = await page.evaluate(() => new Promise(resolve => {
    const request = indexedDB.open('cerdic-library');
    request.onsuccess = () => { const db = request.result; const row = db.transaction('projects').objectStore('projects').get(new URLSearchParams(location.search).get('project')); row.onsuccess = () => { resolve(row.result.text); db.close(); }; };
  }));
  expect(sourceBeforeReload).toBe(''); // Prove restoration comes from the checkpoint, not a successful save.
  await page.reload();
  await expect(page.locator('.stage canvas')).toBeVisible();
  await expect.poll(() => page.evaluate(() => window.__spatial.doc.text().text)).toContain('recovered_box');
  await expect(page.locator('.msg.user')).toContainText('Keep this project and its world on reload.');
  await expect(page.locator('.project-save-state')).toHaveText('Saved locally');
  // Reload immediately after another committed edit, before its 600ms save timer.
  const revision = await page.evaluate(() => window.__spatial.doc.state().revision);
  await page.evaluate(async () => { await window.__spatial.doc.sendText('second_box = solid.box@1(size=[1,1,1])\noutput second_box', 'immediate-reload'); });
  await page.waitForFunction(revision => window.__spatial.doc.state().revision > revision && !window.__spatial.doc.state().pending_document, revision);
  await page.reload();
  await expect(page.locator('.stage canvas')).toBeVisible();
  const restored = await page.evaluate(() => window.__spatial.doc.text().text);
  expect(restored).toContain('recovered_box'); expect(restored).toContain('second_box');
  expect(new URL(page.url()).searchParams.get('project')).toBe(projectId);
  console.log('PASS: saved chat and world both return after a failed IndexedDB save and an immediate reload.');
} finally { await browser.close(); }
