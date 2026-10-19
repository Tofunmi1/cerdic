// G4c-02/06: real language -> worker -> shared meshes -> picking -> panels.
import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir,writeFile } from 'node:fs/promises';
import { serve } from './support.mjs';
const {server,port}=await serve();
const browser=await chromium.launch({args:['--enable-unsafe-swiftshader','--ignore-gpu-blocklist']});
const errors=[];
try {
  const page=await browser.newPage({viewport:{width:1100,height:750}});
  page.on('pageerror',e=>errors.push(e.message));
  await page.goto(`http://127.0.0.1:${port}/?viewer=1&eval=1&docname=g4c-${Date.now()}`);
  await page.waitForFunction(()=>window.__ve?.ready&&window.__ve.evaluation?.state.worker==='idle',null,{timeout:120000});
  await page.evaluate(async()=>{window.__runtime=await import('./ve_runtime.js');});
  const source='b = solid.box(size=[0.008,0.012,0.008])\ns = inst.array(b, count_x=100, count_z=100, spacing=[0.012,0,0.012])\noutput books = s';
  const outcome=await page.evaluate(text=>window.__ve.doc.sendText(text,'Ten thousand instances'),source);
  assert(outcome.pending,JSON.stringify(outcome));
  await page.waitForFunction(()=>window.__ve.doc.state().revision===1&&window.__ve.evaluation.state.worker==='idle',null,{timeout:120000});
  await page.waitForFunction(()=>window.__ve.read().render.main.triangles>=120000,null,{timeout:30000});
  const snapshot=await page.evaluate(async()=>({projection:JSON.parse(window.__runtime.eval_projection_status()),status:window.__ve.read(),eval:window.__ve.evaluation.state,
    runtime_memory_bytes:(await window.__runtime.default()).memory.buffer.byteLength}));
  assert.equal(snapshot.projection.live_meshes,1,'one prototype mesh upload');
  assert.equal(snapshot.projection.upload_frames,1,'one prototype upload frame');
  assert.equal(snapshot.status.authored_count,10000,'ten thousand placements');
  assert(snapshot.status.render.main.draws<=6,'one authored batch plus the five stage meshes');
  assert.equal(Object.values(snapshot.projection.instance_sets)[0],10000);
  assert(snapshot.eval.transferBytes<1000000,'prototype transferred once with compact matrices');
  const id=await page.evaluate(()=>window.__ve.doc.state().document.nodes.find(n=>n.label==='s').id);
  await page.click(`.orow[data-id="${id}"]`);
  assert.match(await page.textContent('#dock-inspector'),/10,000 instances/);
  await page.click(`.orow[data-id="${id}"]`);
  // Keep this pick regression independent of the viewer's product-shot FOV default.
  await page.evaluate(()=>window.__runtime.viewer_configure(true,90,false,2));
  await page.evaluate(()=>window.__runtime.viewer_look_at(0.6,1.5,0.61,0.6,0,0.6));
  await page.waitForTimeout(1200);
  let selected=null;
  for (const [x,y] of [[550,375],[551,375],[550,374]]) {
    await page.mouse.move(x,y); await page.waitForTimeout(700);
    await page.mouse.down(); await page.waitForTimeout(400); await page.mouse.up(); await page.waitForTimeout(700);
    selected=await page.evaluate(()=>JSON.parse(window.__runtime.selection_get()));
    if (selected.instance!==null) break;
  }
  await page.screenshot({path:'target/reports/g4c-instances-pick.png'});
  assert.equal(selected.node,id,'picked authored set');
  assert(Number.isInteger(selected.instance)&&selected.instance>=0&&selected.instance<10000,'picked original instance index');
  assert(selected.instance>=4900&&selected.instance<=5101,'centre ray returns a central placement, not a constant index');
  await page.waitForFunction(()=>document.querySelector('.orow.selected'));
  assert.match(await page.textContent('#dock-inspector'),/selected instance \d+/);
  await mkdir('target/reports',{recursive:true});
  await writeFile('target/reports/g4c-instances.json',JSON.stringify({snapshot,selected},null,2));
  await page.screenshot({path:'target/reports/g4c-instances.png'});
  assert.deepEqual(errors,[]);
  console.log('G4c instances acceptance passed');
} finally {await browser.close();server.close();}
