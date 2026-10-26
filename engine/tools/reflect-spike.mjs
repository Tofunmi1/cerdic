// Measures reflection throughput in the browser's wasm (spike, 2026-10-25): node tools/reflect-spike.mjs
// For N entities (each a Transform): reading every component to JSON through bevy_reflect, writing every component from JSON, and the same
// writes done directly (the floor). Times are milliseconds per pass, the median of 15 passes after 3 warm-up passes.
import { chromium } from "../web/node_modules/playwright/index.mjs";
const browser = await chromium.launch({ channel: "chromium", args: ["--enable-unsafe-webgpu", "--enable-features=Metal,WebGPU", "--ignore-gpu-blocklist"] });
const page = await browser.newPage({ viewport: { width: 800, height: 600 } });
await page.goto("http://localhost:8801/?viewer=1&eval=1&scene=stage&quality=low");
await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 120000 });
const rows = await page.evaluate(async () => {
  const rt = await import("./ve_runtime.js");
  const median = (xs) => xs.sort((a, b) => a - b)[Math.floor(xs.length / 2)];
  const time = (f) => { const t = performance.now(); const r = f(); return [performance.now() - t, r]; };
  const out = [];
  for (const n of [100, 1000, 5000]) {
    rt.reflect_spike_setup(n);
    const result = { entities: n };
    for (const [name, call] of [["read_json", rt.reflect_spike_get], ["write_json", rt.reflect_spike_set], ["write_direct", rt.reflect_spike_direct]]) {
      for (let i = 0; i < 3; i++) call();
      const samples = [];
      let last = 0;
      for (let i = 0; i < 15; i++) { const [ms, r] = time(() => call()); samples.push(ms); last = r; }
      result[name] = { ms: Number(median(samples).toFixed(3)), result: last };
    }
    out.push(result);
  }
  return out;
});
for (const r of rows) {
  const per = (ms) => (ms / r.entities * 1000).toFixed(2);
  console.log(`${String(r.entities).padStart(5)} entities: read ${r.read_json.ms} ms (${per(r.read_json.ms)} us each, ${r.read_json.result} bytes), write ${r.write_json.ms} ms (${per(r.write_json.ms)} us each, ${r.write_json.result} written), direct ${r.write_direct.ms} ms`);
}
await browser.close();
