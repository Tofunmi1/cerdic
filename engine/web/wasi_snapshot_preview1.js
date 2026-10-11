// The C++ kernel is compiled against wasi libc and links into a wasm32-unknown-unknown module, which
// leaves a handful of wasi imports. The kernel does no file or console I/O on its normal paths, so
// those answer "bad file descriptor" (EBADF = 8); only the clock does real work. The build step
// points the wasm-bindgen glue at this file, and the page calls `bindMemory` once after `init()`.
const EBADF = 8;
let memory = null;

/** Gives the shim access to the wasm linear memory (needed to write the clock value). */
export function bindMemory(wasmMemory) {
  memory = wasmMemory;
}

export const fd_close = () => EBADF;
export const fd_seek = () => EBADF;
export const fd_write = () => EBADF;
export const fd_read = () => EBADF;
export const fd_fdstat_get = () => EBADF;

export function clock_time_get(_clockId, _precision, out) {
  if (!memory) return EBADF;
  const nanoseconds = BigInt(Math.round(performance.now() * 1e6));
  new DataView(memory.buffer).setBigUint64(out, nanoseconds, true);
  return 0;
}
