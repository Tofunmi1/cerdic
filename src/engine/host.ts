import { documentBridge } from './bridge';
import type { Runtime } from './bridge';
import { bindInput } from './input';
import { EvaluationHost } from './evaluation';
import { connectMcpBridge } from './mcp-bridge';

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
    runtime.start('ve', params.get('scene') ?? 'stage');
    host.boots++; host.runtime = runtime;
    runtime.viewer_configure(true, 40, false, 3);
    // The first model that appears is framed so it is in view; later edits leave the camera where the person put it.
    let framed = false;
    host.evaluation = new EvaluationHost(runtime, notify, host.resources, {
      onProjected: bounds => {
        if (!framed) framed = runtime.viewer_frame(...bounds.min as [number, number, number], ...bounds.max as [number, number, number]);
      },
    });
    const mcp = params.get('mcp');
    if (mcp) window.addEventListener('pagehide', connectMcpBridge(host, Number(mcp) > 1 ? Number(mcp) : 8765), { once: true });
    host.doc = documentBridge(runtime, notify, () => host.evaluation?.request(), host.evaluation.text);
    // The engine keeps the last document in the browser and opening restores it. A page that names a world to load starts empty instead, so the
    // document that was saved last (another world) is neither shown while the world loads nor mixed into it.
    const docName = params.get('docname') ?? 'spatial-lab';
    if (params.has('world')) runtime.doc_reset(docName, 1);
    else { runtime.doc_open(docName, 1); host.evaluation.request(); }
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
