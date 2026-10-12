// Browser-native loading: Vite must not rewrite the wasm-bindgen glue or its snippet imports.
window.__loadSpatialRuntime = async () => {
  const runtime = await import('/engine/ve_runtime.js');
  const shim = await import('/engine/wasi_snapshot_preview1.js');
  const wasm = await runtime.default();
  shim.bindMemory(wasm.memory);
  return runtime;
};
