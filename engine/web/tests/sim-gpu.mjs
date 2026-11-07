// Runs SimL programs on a real WebGPU device (the shaders the code generator wrote) and compares them with the CPU reference.
// Usage: cargo run --release -p ve-world --example sim_gpu_dump > dump.json && node engine/web/tests/sim-gpu.mjs dump.json
// Exits 1 when a GPU result is outside the tolerance of the program (fall and shift are checked value by value; the chaotic flock by
// short-run values and by whole-flock statistics, since the GPU's division and fused multiply-add differ from the CPU's in the last bits).
import { spawn } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { chromium } from 'playwright';

const dump = JSON.parse(readFileSync(process.argv[2], 'utf8'));
// WebGPU needs a secure context: a page served from 127.0.0.1 is one.
const root = new URL('../../../', import.meta.url).pathname;
const server = spawn('node', ['engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', '5243', '--dir', 'web/dist-bevy-gpu'], { cwd: root, stdio: 'ignore' });
for (let i = 0; i < 100; i++) { try { if ((await fetch('http://127.0.0.1:5243')).ok) break; } catch { /* not up yet */ } await new Promise(r => setTimeout(r, 100)); }
const browser = await chromium.launch({ channel: process.env.PLAYWRIGHT_CHANNEL || 'chromium', args: ['--enable-unsafe-webgpu', '--enable-features=Metal,WebGPU'] });
let failed = false;
try {
  const page = await browser.newPage();
  await page.goto('http://127.0.0.1:5243/');
  const results = await page.evaluate(async (programs) => {
    const adapter = await navigator.gpu?.requestAdapter();
    if (!adapter) return { error: 'no WebGPU adapter' };
    const device = await adapter.requestDevice({ requiredLimits: { maxStorageBuffersPerShaderStage: Math.min(10, adapter.limits.maxStorageBuffersPerShaderStage) } });
    const out = [];
    for (const prog of programs) {
      const { plan, dt, seed } = prog;
      const U = GPUBufferUsage;
      const make = (size, usage) => device.createBuffer({ size: Math.max(16, (size + 3) & ~3), usage });
      // a state sized by a mesh input has one element per vertex of that mesh
      const meshWords = prog.meshWords ? new Uint32Array(prog.meshWords) : null;
      for (const s of plan.states) if (s.mesh !== null) s.count = meshWords[0];
      const live = plan.states.map(s => make(s.count * s.width * 4, U.STORAGE | U.COPY_SRC | U.COPY_DST));
      const snap = plan.states.map(s => make(s.count * s.width * 4, U.STORAGE | U.COPY_DST));
      const boneBuf = make(Math.max(1, plan.inputs.length) * 64, U.STORAGE | U.COPY_DST);
      const meshBuf = meshWords ? make(meshWords.length * 4, U.STORAGE | U.COPY_DST) : null;
      if (meshWords) device.queue.writeBuffer(meshBuf, 0, meshWords);
      // every bone starts at its rest pose (identity), so `init` sees the rest shape
      device.queue.writeBuffer(boneBuf, 0, new Float32Array(Array.from({ length: Math.max(1, plan.inputs.length) }, () => [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]).flat()));
      const params = make(plan.params.length * 4, U.STORAGE | U.COPY_DST);
      device.queue.writeBuffer(params, 0, new Float32Array(plan.params.length ? plan.params : [0]));
      const clockBuf = make(32, U.UNIFORM | U.COPY_DST);
      const pipelines = plan.kernels.map(k => {
        const entries = [
          { binding: 0, visibility: GPUShaderStage.COMPUTE, buffer: { type: 'uniform' } },
          { binding: 1, visibility: GPUShaderStage.COMPUTE, buffer: { type: 'read-only-storage' } },
        ];
        for (const [a, rw, hasSnap] of k.arrays) {
          entries.push({ binding: 2 + 2 * a, visibility: GPUShaderStage.COMPUTE, buffer: { type: rw ? 'storage' : 'read-only-storage' } });
          if (hasSnap) entries.push({ binding: 3 + 2 * a, visibility: GPUShaderStage.COMPUTE, buffer: { type: 'read-only-storage' } });
        }
        for (const [binding] of k.inputs) entries.push({ binding, visibility: GPUShaderStage.COMPUTE, buffer: { type: 'read-only-storage' } });
        const layout = device.createBindGroupLayout({ entries });
        const module = device.createShaderModule({ code: k.wgsl });
        const pipeline = device.createComputePipeline({ layout: device.createPipelineLayout({ bindGroupLayouts: [layout] }), compute: { module, entryPoint: 'main' } });
        const group = device.createBindGroup({
          layout,
          entries: [
            { binding: 0, resource: { buffer: clockBuf } },
            { binding: 1, resource: { buffer: params } },
            ...k.arrays.flatMap(([a, rw, hasSnap]) => [{ binding: 2 + 2 * a, resource: { buffer: live[a] } }, ...(hasSnap ? [{ binding: 3 + 2 * a, resource: { buffer: snap[a] } }] : [])]),
            ...k.inputs.map(([binding, , bone]) => ({ binding, resource: { buffer: bone ? boneBuf : meshBuf } })),
          ],
        });
        return { pipeline, group };
      });
      let t = 0, step = 0;
      const run = async (index, stepDt) => {
        const k = plan.kernels[index];
        const count = k.over === null ? 1 : plan.states[k.over].count;
        if (stepDt > 0 && prog.poses[step]) device.queue.writeBuffer(boneBuf, 0, new Float32Array(prog.poses[step]));
        const clock = new ArrayBuffer(32);
        const f = new Float32Array(clock), u = new Uint32Array(clock);
        f[0] = stepDt; f[1] = Math.fround(t); f[2] = step; u[3] = seed; u[4] = count;
        device.queue.writeBuffer(clockBuf, 0, clock);
        const enc = device.createCommandEncoder();
        for (const [a, rw, hasSnap] of k.arrays) if (hasSnap) enc.copyBufferToBuffer(live[a], 0, snap[a], 0, plan.states[a].count * plan.states[a].width * 4);
        const pass = enc.beginComputePass();
        pass.setPipeline(pipelines[index].pipeline);
        pass.setBindGroup(0, pipelines[index].group);
        pass.dispatchWorkgroups(Math.ceil(count / 64));
        pass.end();
        device.queue.submit([enc.finish()]);
      };
      const walk = async (list) => {
        for (const item of list) {
          if (item[0] === 'run') await run(item[1], dt);
          else for (let i = 0; i < item[1]; i++) await walk(item[2]);
        }
      };
      if (plan.init !== null) await run(plan.init, 0);
      const read = async () => {
        const named = {};
        for (let a = 0; a < plan.states.length; a++) {
          const size = plan.states[a].count * plan.states[a].width * 4;
          const staging = make(size, U.MAP_READ | U.COPY_DST);
          const enc = device.createCommandEncoder();
          enc.copyBufferToBuffer(live[a], 0, staging, 0, size);
          device.queue.submit([enc.finish()]);
          await staging.mapAsync(GPUMapMode.READ);
          named[plan.states[a].name] = Array.from(new Float32Array(staging.getMappedRange().slice(0, size)));
          staging.unmap();
        }
        return named;
      };
      const marks = [];
      const began = performance.now();
      for (const cp of prog.checkpoints) {
        while (step < cp.steps) { await walk(plan.schedule); t += dt; step += 1; }
        marks.push({ steps: cp.steps, gpu: await read() });
      }
      out.push({ name: prog.name, marks, ms: performance.now() - began, info: adapter.info?.description || adapter.info?.vendor || 'adapter' });
    }
    return { out };
  }, dump.programs);
  if (results.error) { console.log('FAIL:', results.error); await browser.close(); server.kill(); process.exit(1); }
  for (const r of results.out) {
    const prog = dump.programs.find(p => p.name === r.name);
    for (const mark of r.marks) {
      const cpu = prog.checkpoints.find(c => c.steps === mark.steps).states;
      let worst = 0, mean = 0, n = 0;
      for (const name of Object.keys(cpu)) for (let i = 0; i < cpu[name].length; i++) { const d = Math.abs(cpu[name][i] - mark.gpu[name][i]); worst = Math.max(worst, d); mean += d; n++; }
      mean /= Math.max(n, 1);
      // aggregate statistics: mean position of the first array, for the chaotic flock after a long run
      const first = Object.keys(cpu)[0];
      const centre = a => [0, 1, 2].map(k => a.filter((_, i) => i % 3 === k).reduce((s, x) => s + x, 0) / (a.length / 3));
      const cc = centre(cpu[first]), gc = centre(mark.gpu[first]);
      const centreGap = Math.max(...cc.map((x, k) => Math.abs(x - gc[k])));
      // [worst single value, centre gap, mean error]. After a long run one bouncing particle can land a step apart (a branch taken
      // on the other side of the floor), so long runs are held to the mean and the centre, short runs to every value.
      const long = mark.steps > 100;
      const limits = { fall: long ? [Infinity, 0.05, 1e-4] : [0.05, 0.05, 1e-4], shift: [0, 0, 0], flock: mark.steps <= 5 ? [0.2, 0.05, 0.01] : [0.5, 2.5, 0.1], bridge: [0.1, 0.05, 0.01], cloth: [0.1, 0.05, 0.01] };
      const [worstLimit, centreLimit, meanLimit] = limits[r.name];
      const ok = worst <= worstLimit && centreGap <= centreLimit && mean <= meanLimit;
      failed ||= !ok;
      console.log(`${ok ? 'ok  ' : 'FAIL'} ${r.name} @ ${mark.steps} steps: worst ${worst.toExponential(2)} mean ${mean.toExponential(2)} centre gap ${centreGap.toExponential(2)}`);
    }
    console.log(`     ${r.name}: ${r.ms.toFixed(0)} ms on the GPU for all checkpoints (${r.info})`);
  }
} finally { await browser.close(); server.kill(); }
process.exit(failed ? 1 : 0);
