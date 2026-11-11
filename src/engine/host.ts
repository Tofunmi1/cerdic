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
  mcpStatus: 'off' as 'off' | 'connecting' | 'connected',
  subscribe(listener: () => void) { listeners.add(listener); return () => { listeners.delete(listener); }; },
  resources: { inputBindings: 0, workers: 0, polling: 0 },
};
declare global { interface Window { __spatial: typeof host; __loadSpatialRuntime(): Promise<Runtime>; } }
window.__spatial = host;
let boot: Promise<void> | undefined;
let disconnectMcp: (() => void) | undefined;
export function setMcpEnabled(enabled: boolean, port = 8765) {
  disconnectMcp?.(); disconnectMcp = undefined;
  const url = new URL(location.href);
  if (enabled) url.searchParams.set('mcp', port === 8765 ? '1' : String(port));
  else url.searchParams.delete('mcp');
  history.replaceState(null, '', url);
  if (enabled) disconnectMcp = connectMcpBridge(host, port, status => { host.mcpStatus = status; notify(); });
}

// This module owns the engine singleton and its subscriptions. Fast-refreshing
// components around a replacement singleton can pair old project state with a
// new empty runtime. Recreate the page lifecycle together in development.
if (import.meta.hot) import.meta.hot.accept(() => location.reload());

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
    if (mcp) setMcpEnabled(true, Number(mcp) > 1 ? Number(mcp) : 8765);
    host.doc = documentBridge(runtime, notify, () => host.evaluation?.request(), host.evaluation.text);
    // Studio's project controller owns restoration. Never briefly open the legacy
    // default document before it loads the requested project (that can be a tractor
    // or any unrelated saved scene). Explicit legacy document links still work.
    const docName = params.get('docname') ?? 'spatial-lab';
    const loadingProject = ['project', 'projects', 'world', 'example'].some(key => params.has(key));
    if (loadingProject || (!params.has('editor') && !params.has('docname'))) runtime.doc_reset('cerdic-startup', 1);
    else runtime.doc_open(docName, 1);
    host.evaluation.request();
    const disposeInput = bindInput(canvas, runtime);
    host.resources.inputBindings = 8;
    const cleanup = () => {
      disconnectMcp?.(); disconnectMcp = undefined;
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
