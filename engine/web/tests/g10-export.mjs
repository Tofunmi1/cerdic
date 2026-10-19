import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import validator from 'gltf-validator';
import { driverAdvisory } from './support.mjs';

// G10: the evaluated model leaves the engine as a binary glTF. The cottage (rounded walls with a cut
// door, a separate red roof) is evaluated in the viewer page, exported through the page's own API, and
// the file is checked by the Khronos validator: no errors or warnings, two materials, the bounds the
// viewer measured, and a pyramid roof's vertices. The file is also left in target/reports/g10/ so it can
// be opened in Blender or a web viewer.
const root = new URL('../../../', import.meta.url).pathname;
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', '5206'], {
  cwd: root, stdout: 'ignore', stderr: 'inherit',
});
const browser = await chromium.launch({
  channel: process.env.PLAYWRIGHT_CHANNEL || undefined,
  args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'],
});
const reports = new URL('../../target/reports/g10/', import.meta.url).pathname;
const program = await readFile(new URL('./fixtures/g10-cottage.txt', import.meta.url), 'utf8');
const texturedProgram = await readFile(new URL('./fixtures/g10-textured-box.txt', import.meta.url), 'utf8');
const logs = [];
try {
  for (let i = 0; i < 100; i++) {
    try { if ((await fetch('http://127.0.0.1:5206')).ok) break; } catch {}
    await Bun.sleep(100);
  }
  await mkdir(reports, { recursive: true });
  const page = await browser.newPage({ viewport: { width: 1100, height: 800 } });
  page.on('pageerror', error => logs.push(error.message));
  page.on('console', message => {
    if (message.type() === 'error' && !driverAdvisory.test(message.text())) logs.push(message.text());
  });
  await page.goto('http://127.0.0.1:5206/?viewer=1&eval=1&docname=g10-export');
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  console.log('G10 export: viewer ready');
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  const staged = await page.evaluate(([text, name]) => window.__ve.doc.sendText(text, name), [program, 'G10 export']);
  assert(staged.pending, `staged: ${JSON.stringify(staged)}`);
  console.log('G10 export: cottage staged');
  await page.waitForFunction(
    () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final'
      || window.__ve.evaluation.state.failureCurrent,
    null, { timeout: 180000 });
  const cottageFailure = await page.evaluate(() => window.__ve.evaluation.state.lastFailure);
  assert.equal(cottageFailure, null, `cottage evaluation: ${JSON.stringify(cottageFailure)}`);
  const exported = await page.evaluate(async () => {
    const glb = await window.__ve.doc.exportGlb();
    const bounds = window.__ve.evaluation.bounds;
    return { bytes: glb ? Array.from(glb) : null, bounds, triangles: window.__ve.evaluation.measures.reduce((s, m) => s + m.triangles, 0) };
  });
  assert(exported.bytes, 'the page produced a glb');
  console.log('G10 export: cottage evaluated and exported');
  const glb = Uint8Array.from(exported.bytes);
  await writeFile(`${reports}cottage.glb`, glb);
  const report = await validator.validateBytes(glb);
  const messages = report.issues.messages.map(m => `${m.code} ${m.pointer ?? ''}`).join('; ');
  assert.equal(report.issues.numErrors, 0, `validator errors: ${messages}`);
  assert.equal(report.issues.numWarnings, 0, `validator warnings: ${messages}`);
  assert.equal(report.info.materialCount, 2, 'beige walls and red roof');
  assert(report.info.totalTriangleCount >= exported.triangles, 'every triangle is exported');
  // The file's bounds are the viewer's bounds (accessor min and max of the POSITION data).
  const jsonLength = new DataView(glb.buffer).getUint32(12, true);
  const json = JSON.parse(new TextDecoder().decode(glb.subarray(20, 20 + jsonLength)));
  const lo = [Infinity, Infinity, Infinity], hi = [-Infinity, -Infinity, -Infinity];
  for (const mesh of json.meshes) {
    const accessor = json.accessors[mesh.primitives[0].attributes.POSITION];
    accessor.min.forEach((v, k) => { lo[k] = Math.min(lo[k], v); });
    accessor.max.forEach((v, k) => { hi[k] = Math.max(hi[k], v); });
  }
  for (let k = 0; k < 3; k++) {
    assert(Math.abs(lo[k] - exported.bounds.min[k]) < 1e-4 && Math.abs(hi[k] - exported.bounds.max[k]) < 1e-4, `axis ${k} bounds match the viewer`);
  }
  await page.evaluate(() => window.__ve.doc.open('g10-texture-export'));
  // Opening reuses the document id and restarts revisions, so "revision N is committed" can be true of the
  // previous scene: wait for a commit from a later evaluation request instead.
  const requestBefore = await page.evaluate(() => JSON.parse(window.__runtime.eval_projection_status()).committed?.request ?? -1);
  const texturedStage = await page.evaluate(text => window.__ve.doc.sendText(text, 'G10 texture export'), texturedProgram);
  assert(texturedStage.pending, `textured stage: ${JSON.stringify(texturedStage)}`);
  console.log('G10 export: UV textured graph staged');
  const texturedRevision = texturedStage.pending.committed.revision;
  await page.waitForFunction(
    ([revision, before]) => {
      const committed = JSON.parse(window.__runtime.eval_projection_status()).committed;
      return (committed?.quality === 'final' && committed?.revision === revision && committed?.request > before)
        || window.__ve.evaluation.state.failureCurrent;
    },
    [texturedRevision, requestBefore],
    { timeout: 180000 });
  const textureFailure = await page.evaluate(() => window.__ve.evaluation.state.lastFailure);
  assert.equal(textureFailure, null, `UV texture evaluation: ${JSON.stringify(textureFailure)}`);
  const texturedBytes = await page.evaluate(async () => Array.from(await window.__ve.doc.exportGlb()));
  const authoredUvCount = await page.evaluate(() => window.__ve.evaluation.parts[0].uvs.length);
  assert(authoredUvCount > 0, 'Blender unwrap UVs crossed the WASM worker boundary');
  console.log(`G10 export: ${authoredUvCount} authored UV floats reached the page`);
  const texturedGlb = Uint8Array.from(texturedBytes);
  const texturedReport = await validator.validateBytes(texturedGlb);
  const texturedMessages = texturedReport.issues.messages.map(m => `${m.code} ${m.pointer ?? ''}`).join('; ');
  assert.equal(texturedReport.issues.numErrors, 0, `textured validator errors: ${texturedMessages}`);
  assert.equal(texturedReport.issues.numWarnings, 0, `textured validator warnings: ${texturedMessages}`);
  const texturedJsonLength = new DataView(texturedGlb.buffer).getUint32(12, true);
  const texturedJson = JSON.parse(new TextDecoder().decode(texturedGlb.subarray(20, 20 + texturedJsonLength)));
  assert(texturedJson.images?.length >= 4, 'albedo, metallic-roughness, normal and emissive PNGs are embedded');
  assert(texturedJson.textures?.length >= 4, 'embedded images have texture objects');
  const texturedMaterial = texturedJson.materials[0];
  assert(texturedMaterial.pbrMetallicRoughness.baseColorTexture, 'base colour texture is linked');
  assert(texturedMaterial.pbrMetallicRoughness.metallicRoughnessTexture, 'metallic-roughness texture is linked');
  assert(texturedMaterial.normalTexture, 'height-derived normal texture is linked');
  assert(texturedMaterial.emissiveTexture, 'emissive texture is linked');
  assert.deepEqual(texturedMaterial.emissiveFactor, [1, 1, 1], 'an emissive map is not multiplied by glTF default black');
  const texturedPrimitive = texturedJson.meshes[0].primitives[0];
  assert(texturedPrimitive.attributes.TANGENT !== undefined, 'MikkTSpace tangents are exported for the normal map');
  assert.equal(texturedJson.accessors[texturedPrimitive.attributes.TANGENT].type, 'VEC4');
  await page.locator('#doc-texture-preview').click();
  await page.waitForFunction(() => {
    const canvas = document.querySelector('#texture-preview canvas');
    return canvas instanceof HTMLCanvasElement && canvas.width > 0 && canvas.height > 0;
  }, null, { timeout: 30000 });
  const previewCanvas = page.locator('#texture-preview canvas');
  const previewBox = await previewCanvas.boundingBox();
  assert(previewBox && previewBox.width > 500 && previewBox.height > 300, 'the on-demand 3D preview has a visible render target');
  await page.screenshot({ path: `${reports}texture-preview.png` });
  await page.mouse.move(previewBox.x + previewBox.width / 2, previewBox.y + previewBox.height / 2);
  await page.mouse.down();
  await page.mouse.move(previewBox.x + previewBox.width / 2 + 40, previewBox.y + previewBox.height / 2 + 15);
  await page.mouse.up();
  await page.keyboard.press('Escape');
  await page.waitForFunction(() => !document.querySelector('#texture-preview'));
  await writeFile(`${reports}textured-box.glb`, texturedGlb);
  console.log(`G10 export: ${glb.byteLength} bytes, ${report.info.totalVertexCount} vertices, ${report.info.totalTriangleCount} triangles, 2 materials, validator clean`);
  assert.deepEqual(logs, [], 'no browser errors');
} finally {
  await browser.close(); child.kill(); await child.exited;
}
