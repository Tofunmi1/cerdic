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
// libc start-up asks for pre-opened directories; there are none, so the enumeration ends at once.
export const fd_prestat_get = () => EBADF;
export const fd_prestat_dir_name = () => EBADF;

// The vendored Blender code reads environment variables (`getenv`); the page has none.
export function environ_sizes_get(count, size) {
  if (!memory) return EBADF;
  const view = new DataView(memory.buffer);
  view.setUint32(count, 0, true);
  view.setUint32(size, 0, true);
  return 0;
}
export const environ_get = () => 0;

/** The vendored Blender code can reach `abort()` on a broken invariant; surface it as an exception. */
export function proc_exit(code) {
  throw new Error(`wasm module exited (code ${code})`);
}

export function clock_time_get(_clockId, _precision, out) {
  if (!memory) return EBADF;
  const nanoseconds = BigInt(Math.round(performance.now() * 1e6));
  new DataView(memory.buffer).setBigUint64(out, nanoseconds, true);
  return 0;
}
