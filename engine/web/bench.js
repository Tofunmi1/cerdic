// Benchmark helper for headset runs. While an XR session presents, it samples the engine's status
// (the engine's statistics restart when a session ends, so the engine keeps a copy of the ended
// session's final figures and this page reads that), can end the session after a set time, and
// afterwards builds, shows and posts a report. The report goes to this page's own server at /report
// (tools/serve-lan.mjs saves it); a failed post is not an error.
//
// Each presentation is its own session with its own report: timing starts when presenting starts,
// not when the page loaded, and nothing carries over from an earlier session.

const SAMPLE_MS = 500;
const TIMELINE_EVERY = 4; // one timeline entry every 4 samples (2 seconds)

const round = (value) => Math.round(value * 100) / 100;

export function watchBenchSession({ read, exit, scene, seconds }) {
  /** The running session's own state, or null between sessions. */
  let session = null;

  const begin = () => ({
    startedAt: performance.now(),
    samples: 0,
    timeline: [],
    worstP95: 0,
    latest: null,
    exitTimer: seconds > 0 ? setTimeout(exit, seconds * 1000) : null,
  });

  const finish = async (ended, status) => {
    clearTimeout(ended.exitTimer);
    // The engine's copy of the final figures is exact; the last poll may be up to a sample old.
    const finalPerf = status.perf?.last_session ?? ended.latest?.perf;
    const report = buildReport(ended.latest ?? status, finalPerf, {
      scene,
      timeline: ended.timeline,
      worstP95: ended.worstP95,
      seconds: Math.round((performance.now() - ended.startedAt) / 1000),
    });
    window.__ve.lastReport = report;
    showReport(report);
    try {
      await fetch("/report", { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(report) });
    } catch {
      // No collecting server; the report is still on screen and in window.__ve.lastReport.
    }
  };

  setInterval(() => {
    const status = read();
    if (status.xr.state === "Presenting") {
      session ??= begin();
      session.latest = status;
      session.samples += 1;
      session.worstP95 = Math.max(session.worstP95, status.perf.p95_ms);
      if (session.samples % TIMELINE_EVERY === 1) {
        session.timeline.push({ fps: round(status.perf.fps), p95_ms: round(status.perf.p95_ms), tier: status.quality.tier });
      }
    } else if (session) {
      const ended = session;
      session = null;
      finish(ended, status);
    }
  }, SAMPLE_MS);
}

function buildReport(status, perf, { scene, timeline, worstP95, seconds }) {
  const p = perf ?? {};
  const droppedPercent = p.total_frames ? round((100 * p.total_dropped) / p.total_frames) : null;
  const framebuffer = status?.xr?.last_frame?.framebuffer;
  return {
    scene,
    when: new Date().toISOString(),
    seconds,
    userAgent: navigator.userAgent,
    adapter: status?.adapter,
    backend: status?.backend,
    quality: status?.quality,
    // What the scene is made of (authored) and what the renderer submitted per frame (counted at the
    // draw calls: the main pass covers both eyes, the shadow pass is extra).
    scene_stats: status?.scene,
    render: status?.render,
    xr_framebuffer: framebuffer,
    eye_viewports: status?.xr?.last_frame?.viewports,
    target_hz: p.target_hz,
    fps: round(p.fps ?? 0),
    p50_ms: round(p.p50_ms ?? 0),
    p95_ms: round(p.p95_ms ?? 0),
    max_ms: round(p.max_ms ?? 0),
    worst_window_p95_ms: round(worstP95),
    total_frames: p.total_frames,
    dropped_frames: p.total_dropped,
    dropped_percent: droppedPercent,
    // Refresh periods missed, estimated from frame intervals (not compositor telemetry).
    missed_periods: p.missed_periods,
    governor: p.governor ?? status?.perf?.governor,
    timeline,
    errors: status?.errors ?? [],
  };
}

function showReport(report) {
  const panel = document.createElement("pre");
  panel.id = "report";
  panel.style.cssText = "position:fixed;left:12px;top:12px;margin:0;padding:12px 14px;max-width:90vw;max-height:80vh;overflow:auto;color:#fff;font:13px/1.45 ui-monospace,monospace;background:rgba(20,24,40,.82);border:1px solid rgba(255,255,255,.25);border-radius:14px";
  const scene = report.scene_stats;
  const main = report.render?.main;
  const lines = [
    `${report.scene}  ${scene?.instances ?? "?"} instances, ${scene?.triangles ?? "?"} scene tris`,
    `submitted/frame: ${main?.draws ?? "?"} main draws, ${main?.triangles ?? "?"} main tris, ${report.render?.shadow?.draws ?? "?"} shadow draws`,
    `tier ${report.quality?.tier}  target ${report.target_hz} Hz  ${report.seconds}s`,
    `fps ${report.fps}   p50 ${report.p50_ms} ms   p95 ${report.p95_ms} ms   max ${report.max_ms} ms`,
    `worst window p95 ${report.worst_window_p95_ms} ms   dropped ${report.dropped_percent}% of ${report.total_frames} frames   missed periods ~${report.missed_periods}`,
    `governor changes: ${(report.governor?.changes ?? []).map((c) => c.tier).join(" > ") || "none"}`,
  ];
  panel.textContent = lines.join("\n");
  document.body.append(panel);
}
