// Browser-native loading of the engine: Vite must not rewrite the wasm-bindgen glue or its snippet imports, so the page loads it from /engine at run time.
// Where the engine's files live, for code in the app bundle that loads them (the texture library).
window.__veEngineBase = `${location.origin}/engine/`;
window.__loadSpatialRuntime = async () => {
  const runtime = await import('/engine/ve_runtime.js');
  const shim = await import('/engine/wasi_snapshot_preview1.js');
  const wasm = await runtime.default();
  shim.bindMemory(wasm.memory);
  return runtime;
};
