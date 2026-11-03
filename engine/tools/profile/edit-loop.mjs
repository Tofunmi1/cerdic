import { chromium } from "/Users/mac/Work/victoria-vr-hack/engine/web/node_modules/playwright/index.mjs";
const world = process.argv[2];
const b = await chromium.launch({ channel: "chromium", args: ["--enable-unsafe-webgpu", "--enable-features=Metal,WebGPU", "--ignore-gpu-blocklist"] });
const p = await b.newPage({ viewport: { width: 1200, height: 750 } });
await p.goto(`http://127.0.0.1:8801/?viewer=1&eval=1&scene=open&world=${world}`);
await p.waitForFunction(() => window.__ve?.ready, null, { timeout: 120000 });
await p.evaluate(async () => { window.__rt = await import("./ve_runtime.js"); });
const status = () => p.evaluate(() => { const s = JSON.parse(window.__rt.eval_projection_status()); return { rev: s.committed?.revision ?? 0, q: s.committed?.quality, gi: s.gi_progress }; });
await p.waitForFunction(() => JSON.parse(window.__rt.eval_projection_status()).committed?.quality === "final", null, { timeout: 300000 });
await p.waitForFunction(() => JSON.parse(window.__rt.eval_projection_status()).gi_progress == null, null, { timeout: 300000 });
const cases = [
  ["sky only: sun_elevation", "sky", "sun_elevation", 20],
  ["one building part: detail 0.5 -> 0.6 (roof)", "roof", "detail", 0.6],
  ["a texture colour (walls style 1)", "w1_t", "program", null],
];
for (const [name, label, param, value] of cases) {
  const before = await status();
  const t0 = Date.now();
  const res = await p.evaluate(([label, param, value]) => {
    const doc = window.__ve.doc.state().document;
    const node = (doc.nodes ?? []).find((n) => n.label === label);
    if (!node) return { error: "no node " + label };
    let v = value;
    if (param === "program") v = String(node.params?.program ?? node.params?.program?.value ?? "").replace("0.86, 0.82, 0.72", "0.9, 0.8, 0.7");
    return window.__ve.doc.send([{ set_param: { node: node.id, param, value: v } }], "edit");
  }, [label, param, value]);
  if (res.error || res.diagnostics) { console.log(name, JSON.stringify(res).slice(0, 300)); continue; }
  await p.waitForFunction((r0) => { const s = JSON.parse(window.__rt.eval_projection_status()); return (s.committed?.revision ?? 0) > r0 && s.committed?.quality === "final"; }, before.rev, { timeout: 300000 });
  const shown = Date.now() - t0;
  await p.waitForFunction(() => JSON.parse(window.__rt.eval_projection_status()).gi_progress == null, null, { timeout: 300000 });
  const all = Date.now() - t0;
  const st = await p.evaluate(() => { const s = window.__ve.evaluation.state.stats; return s ? { cache_hits: s.cache_hits, evaluated: s.evaluated_nodes, eval_ms: Math.round(s.elapsed_ms) } : null; });
  console.log(`${name}: visible after ${(shown / 1000).toFixed(1)} s, light probes done after ${(all / 1000).toFixed(1)} s`, JSON.stringify(st));
}
await b.close();
