// Representation experiment; no imported spike code. Counts and timings use the real runtime.
import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { writeFile, mkdir } from 'node:fs/promises';
import { serve } from './support.mjs';
const { server, port } = await serve();
const browser = await chromium.launch({ args: ['--enable-unsafe-swiftshader','--ignore-gpu-blocklist'] });
try {
  const page = await browser.newPage({ viewport:{width:480,height:270} });
  await page.goto(`http://127.0.0.1:${port}/?scene=instance-probe`);
  await page.waitForFunction(() => window.__ve?.ready && window.__ve.read().frames >= 40, null, {timeout:120000});
  const report = await page.evaluate(async () => {
    const api=await import('./ve_runtime.js');const module=await api.default();
    return {...window.__ve.read(),memory_bytes:module.memory.buffer.byteLength};
  });
  assert.equal(report.scene.instances,10000); assert.equal(report.scene.mesh_assets,1);
  assert.equal(report.render.main.draws,1);
  await mkdir('target/reports',{recursive:true});
  await writeFile('target/reports/g4c-instance-probe.json',JSON.stringify(report,null,2));
  console.log(JSON.stringify({scene:report.scene,perf:report.perf,render:report.render}));
} finally { await browser.close(); server.close(); }
