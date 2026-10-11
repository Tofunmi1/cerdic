// The headset benchmark workflow end to end, against the emulated Quest: a bench page enters VR,
// ends the session itself after a few seconds, shows a report and posts it to the LAN test server,
// which saves it. Also checks the server's own safety (no path escape, bad reports refused) and that
// it can make and use its self-signed certificate.
import fs from "node:fs";
import http from "node:http";
import https from "node:https";
import os from "node:os";
import path from "node:path";
import { chromium } from "playwright";
import { driverAdvisory, here } from "./support.mjs";
import { startServer } from "../../tools/serve-lan.mjs";

const iwerBundle = fs.readFileSync(path.join(here, "..", "node_modules", "iwer", "build", "iwer.min.js"), "utf8");
const failures = [];
const check = (ok, message) => { if (!ok) failures.push(message); };
const scratch = fs.mkdtempSync(path.join(os.tmpdir(), "ve-lan-"));
const reportsDir = path.join(scratch, "reports");

const request = (client, options, body) => new Promise((resolve, reject) => {
  const req = client.request({ rejectUnauthorized: false, ...options }, (res) => {
    const chunks = [];
    res.on("data", (c) => chunks.push(c));
    res.on("end", () => resolve({ status: res.statusCode, body: Buffer.concat(chunks).toString("utf8") }));
  });
  req.on("error", reject);
  req.end(body);
});

const plain = await startServer({ host: "127.0.0.1", port: 0, secure: false, reportsDir });
try {
  const base = { host: "127.0.0.1", port: plain.port };
  check((await request(http, { ...base, path: "/" })).status === 200, "the index page is not served");
  check((await request(http, { ...base, path: "/ve_runtime.js" })).status === 200, "the runtime script is not served");
  check((await request(http, { ...base, path: "/..%2f..%2fpackage.json" })).status === 404, "a path outside web/dist was served");
  check((await request(http, { ...base, path: "/../../../../etc/hosts" })).status === 404, "a path outside web/dist was served");
  check((await request(http, { ...base, path: "/report", method: "POST" }, "not json")).status === 400, "a bad report was accepted");
  check((await request(http, { ...base, path: "/index.html", method: "DELETE" })).status === 405, "DELETE was not refused");
  // Malformed percent-encoding must be refused with 400, and must not take the server down.
  check((await request(http, { ...base, path: "/%E0%A4%A" })).status === 400, "bad percent-encoding was not answered with 400");
  check((await request(http, { ...base, path: "/%" })).status === 400, "a lone % was not answered with 400");
  check((await request(http, { ...base, path: "/index.html%00.js" })).status === 404, "a NUL byte in the path was served");
  check((await request(http, { ...base, path: "/report", method: "POST" }, "5")).status === 400, "a report that is not an object was accepted");
  check((await request(http, { ...base, path: "/" })).status === 200, "the server stopped answering after malformed requests");
  check(fs.existsSync(reportsDir) === false || fs.readdirSync(reportsDir).length === 0, "a bad request left a report file behind");

  const browser = await chromium.launch({ args: ["--enable-unsafe-swiftshader", "--ignore-gpu-blocklist"] });
  try {
    const page = await browser.newPage({ viewport: { width: 480, height: 270 } });
    const logs = [];
    page.on("console", (m) => {
      if (["error", "warning"].includes(m.type()) && !driverAdvisory.test(m.text())) logs.push(`${m.type()}: ${m.text()}`);
    });
    page.on("pageerror", (e) => logs.push(`pageerror: ${e.message}`));
    await page.addInitScript({ content: iwerBundle });
    await page.addInitScript(() => {
      const device = new window.IWER.XRDevice(window.IWER.metaQuest3);
      device.installRuntime({ forceInstall: true });
      device.stereoEnabled = true;
    });
    await page.goto(`http://127.0.0.1:${plain.port}/?scene=bench1&xr=auto&bench=1&seconds=4`);
    await page.waitForSelector("#report", { timeout: 120000 });
    const shown = await page.textContent("#report");
    check(/bench1/.test(shown) && /p95/.test(shown), `the on-page report looks wrong:\n${shown}`);

    // The server saved it.
    const deadline = Date.now() + 10000;
    while ((!fs.existsSync(reportsDir) || fs.readdirSync(reportsDir).length === 0) && Date.now() < deadline) {
      await new Promise((r) => setTimeout(r, 100));
    }
    const files = fs.existsSync(reportsDir) ? fs.readdirSync(reportsDir) : [];
    check(files.length === 1 && files[0].endsWith("-bench1.json"), `reports saved: ${JSON.stringify(files)}`);
    if (files.length === 1) {
      const report = JSON.parse(fs.readFileSync(path.join(reportsDir, files[0]), "utf8"));
      check(report.scene === "bench1" && report.scene_stats?.instances === 45, `unexpected scene info ${JSON.stringify(report.scene_stats)}`);
      // A stereo frame draws every instance once per eye; the report must say what was submitted.
      check(report.render?.main?.draws === 90, `main pass draws ${report.render?.main?.draws}, expected 90 (45 instances x 2 eyes)`);
      check(report.render?.main?.triangles === 2 * report.scene_stats.triangles, `main pass triangles ${report.render?.main?.triangles}, expected two views of ${report.scene_stats.triangles}`);
      check(Array.isArray(report.eye_viewports) && report.eye_viewports.length === 2, "the report has no eye viewports");
      check(Number.isFinite(report.missed_periods) && report.dropped_frames >= 0, "the report has no drop figures");
      check(report.target_hz > 0 && report.total_frames > 0, `no frames were measured: ${report.total_frames}`);
      check(report.seconds >= 3, `the session lasted ${report.seconds}s, expected about 4`);
      check(report.timeline.length >= 1, "the timeline is empty");
      check(report.p95_ms >= report.p50_ms && report.max_ms >= report.p95_ms, "incoherent percentiles in the report");
      check(typeof report.userAgent === "string" && report.errors.length === 0, `report errors: ${JSON.stringify(report.errors)}`);
      console.log(JSON.stringify({ seconds: report.seconds, frames: report.total_frames, fps: report.fps, p95_ms: report.p95_ms, timeline: report.timeline.length }));
    }
    // A second session is its own measurement: its report starts when it starts, and its totals do
    // not include the first session.
    const first = await page.evaluate(() => window.__ve.lastReport);
    await page.waitForTimeout(3000); // time that must not leak into the next session's duration
    await page.evaluate(() => window.__ve.enterXr());
    await page.waitForFunction((when) => window.__ve.lastReport && window.__ve.lastReport.when !== when, first.when, { timeout: 120000 });
    const second = await page.evaluate(() => window.__ve.lastReport);
    check(second.seconds >= 3 && second.seconds <= 7, `the second session lasted ${second.seconds}s, expected about 4 (delayed entry must not count)`);
    check(second.total_frames < first.total_frames * 2 && second.total_frames > 0, `second session frames ${second.total_frames} vs first ${first.total_frames}: totals carried over`);
    const saved = fs.readdirSync(reportsDir).length;
    check(saved === 2, `${saved} reports saved after two sessions, expected 2`);
    check(logs.length === 0, `browser logged problems:\n  ${logs.join("\n  ")}`);
  } finally {
    await browser.close();
  }
} finally {
  await plain.close();
}

// HTTPS with a generated self-signed certificate.
const secure = await startServer({ host: "127.0.0.1", port: 0, secure: true, reportsDir, certDir: path.join(scratch, "cert") });
try {
  const response = await request(https, { host: "127.0.0.1", port: secure.port, path: "/" });
  check(response.status === 200 && /<canvas/.test(response.body), "the https server did not serve the page");
  check(fs.existsSync(path.join(scratch, "cert", "cert.pem")), "no certificate was generated");
} finally {
  await secure.close();
  fs.rmSync(scratch, { recursive: true, force: true });
}

if (failures.length > 0) {
  console.error(`\nFAILED (${failures.length}):\n- ${failures.join("\n- ")}`);
  process.exit(1);
}
console.log("\nbench session checks passed");
