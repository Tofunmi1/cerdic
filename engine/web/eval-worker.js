// Dedicated evaluator entry; kernel geometry stays off the window. Render packing is bounded there.
import init, * as api from './ve_evaluator.js';
import { bindMemory } from './wasi_snapshot_preview1.js';

let serial = 0;
let wasm;
const started = performance.now();
const ready = init().then(instance => {
  wasm = instance; bindMemory(instance.memory);
  postMessage({ kind: 'ready', startup_ms: performance.now() - started });
});
const yieldTask = () => new Promise(resolve => setTimeout(resolve, 0));
const testMode = new URL(import.meta.url).searchParams.get('test') === '1';

self.onmessage = async ({ data }) => {
  if (data.kind === 'cancel') { serial++; api.eval_cancel(); postMessage({ kind: 'cancelled', token: data.token }); return; }
  const own = ++serial;
  await ready;
  if (own !== serial) return;
  if (testMode && data.test === 'spin') {
    const until = performance.now() + 15000;
    while (performance.now() < until) {} // the supervisor must terminate this worker
  }
  const began = performance.now();
  try {
    if (testMode && data.test === 'crash') throw new Error('forced worker crash');
    let state = JSON.parse(api.eval_begin(JSON.stringify(data.document), data.token.quality));
    if (state.error) { postMessage({ kind: 'failure', token: data.token, error: state.error }); return; }
    let quantum = performance.now();
    while (own === serial) {
      state = JSON.parse(api.eval_step());
      if (state.error) { postMessage({ kind: 'failure', token: data.token, error: state.error }); return; }
      if (state.done) break;
      if (performance.now() - quantum > 8) { await yieldTask(); quantum = performance.now(); }
    }
    if (own !== serial) return;
    const parts = state.done.parts.map((part, index) => {
      const positions = api.eval_positions(index), indices = api.eval_indices(index);
      const transforms = api.eval_transforms(index), instance_indices = api.eval_instance_indices(index);
      return { ...part, positions, indices, transforms, instance_indices };
    });
    const bytes = parts.reduce((sum, part) => sum + part.positions.byteLength + part.indices.byteLength + part.transforms.byteLength + part.instance_indices.byteLength
      + new TextEncoder().encode(JSON.stringify(part.frames ?? {})).byteLength, 0);
    const transfer = parts.flatMap(part => [part.positions.buffer, part.indices.buffer, part.transforms.buffer, part.instance_indices.buffer]);
    postMessage({ kind: 'result', token: data.token, parts, bytes, sent_ms: performance.timeOrigin + performance.now(),
      stats: { ...state.done, parts: undefined, elapsed_ms: performance.now() - began,
        memory_bytes: wasm.memory.buffer.byteLength } }, transfer);
    api.eval_cancel();
  } catch (error) {
    postMessage({ kind: 'crash', token: data.token, error: { code: 'eval.worker_crash', message: String(error) } });
  }
};
