// Benchmark helper for headset runs. While an XR session presents, it samples the engine's frame
// statistics (they restart when the session ends, so the page keeps its own copy), can end the
// session after a set time, and afterwards builds, shows and posts a report. The report goes to this
// page's own server at /report (tools/serve-lan.mjs saves it); a failed post is not an error.

const SAMPLE_MS = 500;
const TIMELINE_EVERY = 4; // one timeline entry every 4 samples (2 seconds)

export function watchBenchSession({ read, exit, scene, seconds }) {
  let latest = null;
  let wasPresenting = false;
  let samples = 0;
  let exitTimer = null;
  const timeline = [];
  let worstP95 = 0;
  const startedAt = performance.now();

  const finish = async () => {
    const report = buildReport(latest, { scene, timeline, worstP95, seconds: Math.round((performance.now() - startedAt) / 1000) });
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
    const presenting = status.xr.state === "Presenting";
    if (presenting) {
      latest = status;
      samples += 1;
      worstP95 = Math.max(worstP95, status.perf.p95_ms);
      if (samples % TIMELINE_EVERY === 1) {
        timeline.push({ fps: round(status.perf.fps), p95_ms: round(status.perf.p95_ms), tier: status.quality.tier });
      }
      if (!wasPresenting && seconds > 0) exitTimer = setTimeout(exit, seconds * 1000);
    } else if (wasPresenting) {
      clearTimeout(exitTimer);
      finish();
    }
    wasPresenting = presenting;
  }, SAMPLE_MS);
}

const round = (value) => Math.round(value * 100) / 100;

function buildReport(status, { scene, timeline, worstP95, seconds }) {
  const perf = status?.perf ?? {};
  const droppedPercent = perf.total_frames ? round((100 * perf.total_dropped) / perf.total_frames) : null;
  return {
    scene,
    when: new Date().toISOString(),
    seconds,
    userAgent: navigator.userAgent,
    adapter: status?.adapter,
    backend: status?.backend,
    quality: status?.quality,
    scene_stats: status?.scene,
    target_hz: perf.target_hz,
    fps: round(perf.fps ?? 0),
    p50_ms: round(perf.p50_ms ?? 0),
    p95_ms: round(perf.p95_ms ?? 0),
    max_ms: round(perf.max_ms ?? 0),
    worst_window_p95_ms: round(worstP95),
    total_frames: perf.total_frames,
    dropped_percent: droppedPercent,
    governor: perf.governor,
    timeline,
    errors: status?.errors ?? [],
  };
}

function showReport(report) {
  const panel = document.createElement("pre");
  panel.id = "report";
  panel.style.cssText = "position:fixed;left:12px;top:12px;margin:0;padding:12px 14px;max-width:90vw;max-height:80vh;overflow:auto;color:#fff;font:13px/1.45 ui-monospace,monospace;background:rgba(20,24,40,.82);border:1px solid rgba(255,255,255,.25);border-radius:14px";
  const lines = [
    `${report.scene}  ${report.scene_stats?.draws ?? "?"} draws, ${report.scene_stats?.triangles ?? "?"} tris`,
    `tier ${report.quality?.tier}  target ${report.target_hz} Hz  ${report.seconds}s`,
    `fps ${report.fps}   p50 ${report.p50_ms} ms   p95 ${report.p95_ms} ms   max ${report.max_ms} ms`,
    `worst window p95 ${report.worst_window_p95_ms} ms   dropped ${report.dropped_percent}% of ${report.total_frames} frames`,
    `governor changes: ${(report.governor?.changes ?? []).map((c) => c.tier).join(" > ") || "none"}`,
  ];
  panel.textContent = lines.join("\n");
  document.body.append(panel);
}
