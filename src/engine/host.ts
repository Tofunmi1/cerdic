import { documentBridge } from './bridge';
import type { Runtime } from './bridge';
import { bindInput } from './input';
import { EvaluationHost } from './evaluation';

// React moves this element. It must never create or replace the engine's canvas.
const canvas = document.createElement('canvas');
canvas.id = 've'; canvas.tabIndex = 0;
canvas.setAttribute('aria-label', '3D viewport');
const listeners = new Set<() => void>();
const notify = () => listeners.forEach(listener => listener());
export const host = {
  canvas, ready: false, boots: 0, error: '', xr: false,
  runtime: null as Runtime | null,
  doc: null as ReturnType<typeof documentBridge> | null,
  evaluation: null as EvaluationHost | null,
  subscribe(listener: () => void) { listeners.add(listener); return () => { listeners.delete(listener); }; },
  resources: { inputBindings: 0, workers: 0, polling: 0 },
};
declare global { interface Window { __spatial: typeof host; __loadSpatialRuntime(): Promise<Runtime>; } }
window.__spatial = host;
let boot: Promise<void> | undefined;

export function mountEngine(container: HTMLElement) {
  container.append(canvas);
  boot ??= start();
  return boot;
}

async function start() {
  try {
    const runtime = await window.__loadSpatialRuntime();
    const params = new URLSearchParams(location.search);
    runtime.start('ve', params.get('scene') ?? 'gallery');
    host.boots++; host.runtime = runtime;
    runtime.viewer_configure(true, 40, false, 3);
    host.evaluation = new EvaluationHost(runtime, notify, host.resources);
    host.doc = documentBridge(runtime, notify, () => host.evaluation?.request());
    runtime.doc_open(params.get('docname') ?? 'spatial-lab', 1);
    host.evaluation.request();
    const disposeInput = bindInput(canvas, runtime);
    host.resources.inputBindings = 8;
    const cleanup = () => {
      disposeInput(); host.resources.inputBindings = 0;
      host.evaluation?.dispose();
      clearInterval(poll); host.resources.polling = 0;
    };
    window.addEventListener('pagehide', cleanup, { once: true });
    host.resources.polling = 1;
    const poll = setInterval(() => {
      const status = JSON.parse(runtime.status()) as { frames: number };
      if (status.frames > 0) {
        host.ready = true; clearInterval(poll); host.resources.polling = 0; notify();
      }
    }, 50);
    host.xr = await runtime.xr_supported();
    notify();
  } catch (error) { host.error = String(error); notify(); }
}
