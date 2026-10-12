// X1 measurements: run with `node measure.mjs` after building. Prints one JSON report.
import fs from "node:fs";
import http from "node:http";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "/Users/mac/Work/victoria-vr-hack/engine/web/node_modules/playwright/index.mjs";

const root = path.join(path.dirname(fileURLToPath(import.meta.url)), "web");
const types = { ".js": "text/javascript", ".wasm": "application/wasm", ".html": "text/html" };
const server = http.createServer((req, res) => {
  const file = path.join(root, req.url.split("?")[0] === "/" ? "index.html" : req.url.split("?")[0]);
  try {
    res.writeHead(200, { "content-type": types[path.extname(file)] ?? "application/octet-stream" });
    res.end(fs.readFileSync(file));
  } catch { res.writeHead(404); res.end(); }
});
await new Promise((r) => server.listen(0, "127.0.0.1", r));
const port = server.address().port;

const browser = await chromium.launch({ args: ["--enable-unsafe-swiftshader", "--ignore-gpu-blocklist"] });
const page = await browser.newPage();
const logs = [];
page.on("console", (m) => logs.push(`${m.type()}: ${m.text()}`));
page.on("pageerror", (e) => logs.push(`pageerror: ${e.message}`));
await page.goto(`http://127.0.0.1:${port}/`);
await page.waitForFunction(() => window.x1);

const report = await page.evaluate(async () => {
  const { spawn, frameProbe } = window.x1;
  const out = {};
  const wait = (ms) => new Promise((r) => setTimeout(r, ms));
  const median = (a) => [...a].sort((x, y) => x - y)[a.length >> 1];

  // Cold start: a fresh worker fetching, compiling and instantiating the module.
  const starts = [];
  for (let i = 0; i < 4; i++) {
    const t0 = performance.now();
    const w = spawn();
    const r = await w.call("init");
    starts.push({ initMs: Math.round(r.ms), totalMs: Math.round(performance.now() - t0), memory: r.memory });
    w.terminate();
  }
  out.coldStart = starts;

  const worker = spawn();
  await worker.call("init");

  // Job latency and transfer: heavier unions return bigger meshes.
  out.jobs = [];
  for (const [spheres, segments] of [[2, 32], [6, 48], [10, 64], [14, 64]]) {
    const times = [];
    let last = null;
    for (let i = 0; i < 3; i++) {
      const t0 = performance.now();
      last = await worker.call("heavy", [spheres, segments]);
      times.push({ roundTripMs: Math.round(performance.now() - t0), jobMs: Math.round(last.ms) });
    }
    out.jobs.push({ spheres, segments, floats: last.floats, megabytes: +(last.floats * 4 / 1e6).toFixed(2), times, ok: last.ok, error: last.error });
  }
  const graph = await worker.call("graph", [200]);
  out.graphCall = { ok: graph.ok, value: graph.value, ms: Math.round(graph.ms) };

  // Memory after many jobs (does the module hand memory back, or only grow).
  const before = (await worker.call("memory_bytes")).value;
  for (let i = 0; i < 20; i++) await worker.call("heavy", [6, 48]);
  const after = (await worker.call("memory_bytes")).value;
  out.memory = { beforeMB: +(before / 1e6).toFixed(1), afterMB: +(after / 1e6).toFixed(1) };

  // Main thread stays responsive during a long job.
  const idle = frameProbe();
  await wait(1000);
  const idleFrames = idle.stop();
  const busy = frameProbe();
  const long = await worker.call("heavy", [14, 64]);
  const busyFrames = busy.stop();
  out.responsiveness = { idle: idleFrames, duringJob: busyFrames, jobMs: Math.round(long.ms), ok: long.ok, error: long.error };

  // A job that never returns: terminate it, restart, and check the answers are unchanged.
  const reference = await worker.call("heavy", [6, 48]);
  out.restarts = [];
  let current = worker;
  for (let i = 0; i < 5; i++) {
    const stuck = current.call("spin");
    await wait(300);
    const probe = frameProbe();
    const t0 = performance.now();
    current.terminate();
    const result = await stuck;
    const next = spawn();
    const init = await next.call("init");
    const again = await next.call("heavy", [6, 48]);
    const frames = probe.stop();
    out.restarts.push({
      stuckResult: result.error,
      restartMs: Math.round(performance.now() - t0),
      initMs: Math.round(init.ms),
      sameAnswer: again.floats === reference.floats && again.sum === reference.sum,
      mainThread: frames,
    });
    current = next;
  }

  // A trap (wasm unreachable): what does the worker say, and can it be used afterwards?
  const trapped = await current.call("trap");
  const afterTrap = await current.call("heavy", [2, 32]);
  out.trap = { trapResult: trapped.error, nextCallOk: afterTrap.ok, nextCallError: afterTrap.error };
  current.terminate();
  return out;
});
report.consoleProblems = logs.filter((l) => /error|warn/.test(l)).slice(0, 8);
console.log(JSON.stringify(report, null, 1));
await browser.close();
server.close();
