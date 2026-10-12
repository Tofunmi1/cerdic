// X1 worker: loads the evaluator-only module and answers one message at a time.
import init, * as api from "./x1_eval.js";
import { bindMemory } from "./wasi_snapshot_preview1.js";

let wasm = null;

self.onmessage = async (event) => {
  const { id, type, args = [] } = event.data;
  try {
    if (type === "init") {
      const t0 = performance.now();
      wasm = await init();
      bindMemory(wasm.memory);
      self.postMessage({ id, ok: true, ms: performance.now() - t0, memory: wasm.memory.buffer.byteLength });
      return;
    }
    const t0 = performance.now();
    const result = api[type](...args);
    const ms = performance.now() - t0;
    if (result instanceof Float32Array) {
      let sum = 0;
      for (let i = 0; i < result.length; i += 97) sum += result[i];
      self.postMessage({ id, ok: true, ms, floats: result.length, sum, memory: wasm.memory.buffer.byteLength, buf: result.buffer }, [result.buffer]);
    } else {
      self.postMessage({ id, ok: true, ms, value: result, memory: wasm.memory.buffer.byteLength });
    }
  } catch (error) {
    self.postMessage({ id, ok: false, error: String(error) });
  }
};
