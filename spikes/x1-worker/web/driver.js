// X1 page side: spawn a worker with a promise-based call, and a probe that shows whether the main
// thread keeps getting animation frames while a job runs.
export function spawn() {
  const worker = new Worker("./worker.js", { type: "module" });
  let seq = 0;
  const pending = new Map();
  const settle = (message) => {
    for (const p of pending.values()) p(message);
    pending.clear();
  };
  worker.onmessage = (e) => {
    const resolve = pending.get(e.data.id);
    if (resolve) {
      pending.delete(e.data.id);
      resolve(e.data);
    }
  };
  worker.onerror = (e) => settle({ ok: false, error: `worker error: ${e.message}` });
  return {
    call(type, args = []) {
      const id = ++seq;
      return new Promise((resolve) => {
        pending.set(id, resolve);
        worker.postMessage({ id, type, args });
      });
    },
    terminate() {
      worker.terminate();
      settle({ ok: false, error: "terminated" });
    },
  };
}

export function frameProbe() {
  let last = performance.now();
  let frames = 0;
  let maxGap = 0;
  let running = true;
  const tick = (now) => {
    if (!running) return;
    maxGap = Math.max(maxGap, now - last);
    last = now;
    frames++;
    requestAnimationFrame(tick);
  };
  requestAnimationFrame(tick);
  return {
    stop() {
      running = false;
      return { frames, maxGapMs: Math.round(maxGap) };
    },
  };
}

window.x1 = { spawn, frameProbe };
