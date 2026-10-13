// G4d: frames flow into the accepted evaluation result and can be selected in the inspector.
import assert from 'node:assert/strict';
import {chromium} from 'playwright';
import {serve} from './support.mjs';

const {server,port}=await serve();
const browser=await chromium.launch({args:['--enable-unsafe-swiftshader','--ignore-gpu-blocklist']});
try {
  const page=await browser.newPage({viewport:{width:1100,height:750}});
  const errors=[];page.on('pageerror',e=>errors.push(e.message));
  await page.goto(`http://127.0.0.1:${port}/?viewer=1&eval=1&docname=sockets-${Date.now()}`);
  await page.waitForFunction(()=>window.__ve?.ready&&window.__ve.evaluation?.state.worker==='idle',null,{timeout:90000});
  const program=`base = solid.box(size=[1,1,1])
socket = frame.add(base, name="mount", position=[0,0.5,0], rotation=[0,0,90])
parent = xform.move(socket, by=[2,1,0])
leg = solid.box(size=[0.2,1,0.2])
foot = frame.add(leg, name="foot", position=[0,0.5,0])
attached = xform.attach(foot, to=parent, socket="foot", to_socket="mount")
output parent
output attached`;
  const outcome=await page.evaluate(t=>window.__ve.doc.sendText(t,'Build socket assembly'),program);
  assert(outcome.pending,JSON.stringify(outcome));
  await page.waitForFunction(()=>window.__ve.doc.state().revision===1&&window.__ve.evaluation.state.worker==='idle',null,{timeout:90000});
  const child=await page.evaluate(()=>window.__ve.doc.state().document.nodes.find(n=>n.label==='attached').id);
  await page.click(`.orow[data-id="${child}"]`);
  const socket=page.locator('#dock-inspector button.socket[data-socket="foot"]');
  await socket.waitFor({timeout:10000});
  assert.match(await socket.textContent(),/attached · foot · 2, 1\.5, 0 m/);
  await socket.click();
  assert.deepEqual(await page.evaluate(()=>window.__ve.docPanel.selection.socket),{part:child,name:'foot'});
  assert((await socket.getAttribute('class')).includes('selected'),'socket selection is visible');

  await page.evaluate(t=>window.__ve.doc.sendText(t,'Define a grouped socket'),`group wheel
body = solid.box(size=[0.4,0.2,0.4])
hub = frame.add(body, name="hub", position=[0,0.1,0])
result hub
end
wheels = wheel()
output wheels`);
  await page.waitForFunction(()=>window.__ve.doc.state().revision===2&&window.__ve.evaluation.state.worker==='idle',null,{timeout:90000});
  const group=await page.evaluate(()=>window.__ve.doc.state().document.groups[0].id);
  await page.click(`.orow[data-id="${group}"]`);
  await page.locator('#dock-inspector button.socket[data-socket="hub"]').waitFor({timeout:10000});
  assert.match(await page.locator('#dock-inspector button.socket[data-socket="hub"]').textContent(),/wheels · hub/);
  assert.deepEqual(errors,[]);
  console.log('G4d sockets inspector acceptance passed');
} finally {await browser.close();server.close();}
