// XR session lifecycle against IWER: a request that fails after the browser granted a session gives
// the session back, two simultaneous requests make exactly one session, and entering and leaving
// repeatedly always ends with exactly one running presentation and no abandoned session.
import fs from "node:fs";
import path from "node:path";
import { chromium } from "playwright";
import { driverAdvisory, here, serve } from "./support.mjs";

const iwerBundle = fs.readFileSync(path.join(here, "..", "node_modules", "iwer", "build", "iwer.min.js"), "utf8");
const failures = [];
const check = (ok, message) => { if (!ok) failures.push(message); };

const { server, port } = await serve();
const browser = await chromium.launch({ args: ["--enable-unsafe-swiftshader", "--ignore-gpu-blocklist"] });
try {
  const page = await browser.newPage({ viewport: { width: 480, height: 270 } });
  const logs = [];
  page.on("console", (m) => {
    // The deliberately failing request is reported by the page; anything else is a problem.
    if (["error", "warning"].includes(m.type()) && !driverAdvisory.test(m.text()) && !/space rejected/.test(m.text())) logs.push(`${m.type()}: ${m.text()}`);
  });
  page.on("pageerror", (e) => logs.push(`pageerror: ${e.message}`));
  await page.addInitScript({ content: iwerBundle });
  await page.addInitScript(() => {
    const device = new window.IWER.XRDevice(window.IWER.metaQuest3);
    device.installRuntime({ forceInstall: true });
    device.stereoEnabled = true;
    // Record every session the page obtains and how often it was ended; optionally break the
    // reference-space request to simulate a failure after the session was granted.
    const real = navigator.xr.requestSession.bind(navigator.xr);
    window.__sessions = [];
    window.__failSpace = false;
    navigator.xr.requestSession = async (...args) => {
      const session = await real(...args);
      const record = { ended: 0, over: false };
      window.__sessions.push(record);
      const end = session.end.bind(session);
      session.end = () => { record.ended++; return end(); };
      session.addEventListener("end", () => { record.over = true; });
      if (window.__failSpace) session.requestReferenceSpace = () => Promise.reject(new Error("space rejected"));
      return session;
    };
  });

  await page.goto(`http://127.0.0.1:${port}/?scene=xr`);
  await page.waitForFunction(() => window.__ve?.ready || window.__ve?.error, null, { timeout: 90000 });
  check(!(await page.evaluate(() => window.__ve.error)), `start failed: ${await page.evaluate(() => window.__ve.error)}`);
  const xr = () => page.evaluate(() => window.__ve.read().xr);
  const sessions = () => page.evaluate(() => window.__sessions);

  // 1. A failure after the session was granted ends that session and reports Failed.
  await page.evaluate(() => { window.__failSpace = true; });
  const refused = await page.evaluate(() => window.__ve.enterXr().then(() => "entered", (e) => String(e)));
  check(refused !== "entered", "a rejected reference space did not fail the request");
  let list = await sessions();
  check(list.length === 1 && list[0].ended === 1 && list[0].over, `the granted session was not ended: ${JSON.stringify(list)}`);
  let state = await xr();
  check(state.state === "Failed" && /space rejected/.test(state.error ?? ""), `state after the failure: ${JSON.stringify([state.state, state.error])}`);

  // 2. After a failure the next attempt works (the state machine is not wedged).
  await page.evaluate(() => { window.__failSpace = false; });
  await page.evaluate(() => window.__ve.enterXr());
  await page.waitForFunction(() => window.__ve.read().xr.state === "Presenting" && window.__ve.read().xr.frames >= 3, null, { timeout: 60000 });
  list = await sessions();
  check(list.length === 2 && !list[1].over, `expected a live second session: ${JSON.stringify(list)}`);

  // 3. A second request while presenting is refused and makes no session.
  const busy = await page.evaluate(() => window.__ve.enterXr().then(() => "entered", (e) => String(e)));
  check(/already/.test(busy), `a request while presenting was not refused: ${busy}`);
  check((await sessions()).length === 2, "a refused request still created a session");

  // 4. Leave, then two simultaneous requests: exactly one session results.
  await page.evaluate(() => window.__ve.exitXr());
  await page.waitForFunction(() => window.__ve.read().xr.state === "Ended", null, { timeout: 30000 });
  const both = await page.evaluate(() => Promise.allSettled([window.__ve.enterXr(), window.__ve.enterXr()]).then((r) => r.map((x) => x.status)));
  check(both.filter((s) => s === "fulfilled").length === 1 && both.filter((s) => s === "rejected").length === 1, `simultaneous requests: ${JSON.stringify(both)}`);
  await page.waitForFunction(() => window.__ve.read().xr.state === "Presenting", null, { timeout: 60000 });
  check((await sessions()).length === 3, `simultaneous requests made ${(await sessions()).length - 2} sessions, expected 1`);

  // 5. Repeated enter and exit: every session ends exactly once, and the loop keeps running.
  for (let i = 0; i < 3; i++) {
    await page.evaluate(() => window.__ve.exitXr());
    await page.waitForFunction(() => window.__ve.read().xr.state === "Ended", null, { timeout: 30000 });
    await page.evaluate(() => window.__ve.enterXr());
    await page.waitForFunction(() => window.__ve.read().xr.state === "Presenting" && window.__ve.read().xr.frames >= 3, null, { timeout: 60000 });
  }
  list = await sessions();
  const live = list.filter((s) => !s.over).length;
  check(live === 1, `${live} sessions are still open, expected exactly 1: ${JSON.stringify(list)}`);
  check(list.slice(0, -1).every((s) => s.over), "an earlier session was left open");
  await page.evaluate(() => window.__ve.exitXr());
  await page.waitForFunction(() => window.__ve.read().xr.state === "Ended", null, { timeout: 30000 });
  check((await sessions()).every((s) => s.over), "a session survived the final exit");

  check(logs.length === 0, `browser logged problems:\n  ${logs.join("\n  ")}`);
} finally {
  await browser.close();
  server.close();
}

if (failures.length > 0) {
  console.error(`xr-lifecycle test FAILED (${failures.length}):\n- ${failures.join("\n- ")}`);
  process.exit(1);
}
console.log("xr-lifecycle test passed");
