// The engine as a singleton outside React: one wasm instance, started once, on a canvas that React
// does not own. React only moves the canvas element into whichever panel is showing.
const state = { starts: 0, ready: null, canvas: null, api: null, errors: [] };
window.__x6 = state;

export function getCanvas() {
  if (!state.canvas) {
    const canvas = document.createElement("canvas");
    canvas.id = "ve";
    canvas.style.cssText = "display:block;width:100%;height:100%;touch-action:none";
    state.canvas = canvas;
  }
  return state.canvas;
}

// The canvas must be in the document when this is called: `start` looks it up by id.
export function boot() {
  state.ready ??= (async () => {
    const base = "/engine/";
    const mod = await import(/* @vite-ignore */ `${base}ve_runtime.js`);
    const shim = await import(/* @vite-ignore */ `${base}wasi_snapshot_preview1.js`);
    const wasm = await mod.default();
    shim.bindMemory(wasm.memory);
    mod.start("ve", "xr");
    state.starts++;
    state.api = mod;
  })().catch((e) => { state.errors.push(String(e)); throw e; });
  return state.ready;
}
