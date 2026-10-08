// Times each engine stage (warm-up, then mean of several iterations).
import { createWasi } from "./wasi-shim.mjs";

const STAGES = [
  [0, "C++  Manifold boolean (cube − sphere)"],
  [2, "C++  geomproc weld (ours)"],
  [3, "C++  meshoptimizer simplify to 25%"],
  [4, "Rust Rapier: build trimesh + 240 steps"],
  [5, "C++→Rust pipeline: Manifold → weld → simplify → Rapier"],
  [1, "Rust boolmesh boolean (same inputs)"],
  [6, "Rust-only pipeline: boolmesh → Rapier"],
];

export async function bench(bytes, segmentsList = [64, 128]) {
  const wasi = createWasi();
  const tc = performance.now();
  const module = await WebAssembly.compile(bytes);
  const compileMs = performance.now() - tc;
  const ti = performance.now();
  const instance = await WebAssembly.instantiate(module, wasi.imports);
  wasi.bind(instance);
  const e = instance.exports;
  e._initialize();
  const out = { compileMs, instantiateMs: performance.now() - ti, runs: [] };
  for (const segments of segmentsList) {
    e.setup(segments);
    const rows = [];
    for (const [id, label] of STAGES) {
      const firstStart = performance.now();
      const value = e.run_stage(id, 1);
      const firstMs = performance.now() - firstStart;
      e.run_stage(id, 2);
      let iters = 3, ms;
      for (;;) {
        const t = performance.now();
        e.run_stage(id, iters);
        ms = performance.now() - t;
        if (ms > 300 || iters >= 200) break;
        iters *= 3;
      }
      rows.push({ stage: label, firstMs: +firstMs.toFixed(2), meanMs: +(ms / iters).toFixed(3), iters, value });
      if (id === 0 || id === 1) {
        const len = e.report();
        rows.at(-1).mesh = JSON.parse(new TextDecoder().decode(new Uint8Array(e.memory.buffer, e.report_ptr(), len)));
      }
    }
    out.runs.push({ segments, rows });
  }
  return out;
}
