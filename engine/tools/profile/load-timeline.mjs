// node timeline.mjs <world> : wall-clock of each stage of the browser path, polled every 250 ms.
import { chromium } from "/Users/mac/Work/victoria-vr-hack/engine/web/node_modules/playwright/index.mjs";
const world = process.argv[2];
const b = await chromium.launch({ channel: "chromium", args: ["--enable-unsafe-webgpu", "--enable-features=Metal,WebGPU", "--ignore-gpu-blocklist"] });
const p = await b.newPage({ viewport: { width: 1200, height: 750 } });
const t0 = Date.now();
const at = () => ((Date.now() - t0) / 1000).toFixed(1).padStart(6) + "s";
const reqs = [];
p.on("response", async (r) => { const u = r.url(); if (/\.wasm|\.js$|\.pack|\.jpg|\.png/.test(u)) reqs.push([Date.now() - t0, u.split("/").pop(), Number(r.headers()["content-length"] ?? 0)]); });
await p.goto(`http://127.0.0.1:8801/?viewer=1&eval=1&scene=open&world=${world}`);
console.log(at(), "document loaded");
await p.waitForFunction(() => window.__ve?.ready, null, { timeout: 180000 });
console.log(at(), "page ready (__ve.ready)");
await p.evaluate(async () => { window.__rt = await import("./ve_runtime.js"); });
const seen = new Set();
let last = "";
for (let i = 0; i < 4800; i++) {
  const s = await p.evaluate(() => {
    const st = JSON.parse(window.__rt.eval_projection_status());
    const ev = window.__ve.evaluation.state;
    return { worker: ev.worker, rev: ev.revision, refining: ev.refining, quality: st.committed?.quality ?? null, gi: st.gi_progress, tex: st.texture_maps, meshes: st.live_meshes, staging: st.staging_meshes, queued: st.queued_bytes, upload_ms: st.last_upload_ms };
  });
  const key = JSON.stringify([s.worker, s.rev, s.refining, s.quality, s.gi === null ? null : Math.round(s.gi * 10), s.tex > 0, s.meshes > 0]);
  if (key !== last) { console.log(at(), JSON.stringify(s)); last = key; }
  if (s.quality === "final" && (s.gi === null || s.gi >= 1) && !s.refining && s.worker === "idle") { console.log(at(), "DONE"); break; }
  await p.waitForTimeout(250);
}
const big = reqs.filter((r) => r[2] > 500000).map((r) => `${(r[0] / 1000).toFixed(1)}s ${r[1]} ${(r[2] / 1e6).toFixed(1)}MB`);
console.log("large fetches:", big.join(" | "));
await b.close();
