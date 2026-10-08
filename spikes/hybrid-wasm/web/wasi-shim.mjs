// Minimal browser WASI shim: the engine needs no files, network or env.
const ENOSYS = 52;
export function createWasi(log = console.log) {
  let memory;
  const view = () => new DataView(memory.buffer);
  const decoder = new TextDecoder();
  const called = new Set();
  const impl = {
    fd_write(fd, iovs, iovsLen, nwritten) {
      let total = 0, text = "";
      for (let i = 0; i < iovsLen; i++) {
        const ptr = view().getUint32(iovs + i * 8, true);
        const len = view().getUint32(iovs + i * 8 + 4, true);
        text += decoder.decode(new Uint8Array(memory.buffer, ptr, len));
        total += len;
      }
      log(`[wasm fd${fd}] ${text}`);
      view().setUint32(nwritten, total, true);
      return 0;
    },
    random_get(ptr, len) { crypto.getRandomValues(new Uint8Array(memory.buffer, ptr, len)); return 0; },
    clock_time_get(_id, _precision, out) {
      view().setBigUint64(out, BigInt(Math.round(performance.now() * 1e6)), true); return 0;
    },
    environ_sizes_get(count, size) { view().setUint32(count, 0, true); view().setUint32(size, 0, true); return 0; },
    environ_get() { return 0; },
    args_sizes_get(count, size) { view().setUint32(count, 0, true); view().setUint32(size, 0, true); return 0; },
    args_get() { return 0; },
    proc_exit(code) { throw new Error(`wasm proc_exit(${code})`); },
  };
  const imports = new Proxy({}, {
    get: (_, name) => (...args) => {
      called.add(name);
      return name in impl ? impl[name](...args) : ENOSYS;
    },
  });
  return {
    imports: { wasi_snapshot_preview1: imports },
    bind(instance) { memory = instance.exports.memory; },
    called,
  };
}

export async function runEngine(bytes, log) {
  const wasi = createWasi(log);
  const module = await WebAssembly.compile(bytes);
  const needed = WebAssembly.Module.imports(module).map((i) => `${i.module}.${i.name}`);
  const t0 = performance.now();
  const instance = await WebAssembly.instantiate(module, wasi.imports);
  wasi.bind(instance);
  instance.exports._initialize?.(); // reactor: runs C++ static constructors
  const t1 = performance.now();
  instance.exports.run_spike();
  const t2 = performance.now();
  instance.exports.run_spike(); // second call: C++ globals must survive between exports
  const t3 = performance.now();
  const { memory, result_ptr, result_len } = instance.exports;
  const result = JSON.parse(new TextDecoder().decode(new Uint8Array(memory.buffer, result_ptr(), result_len())));
  return { result, imports: needed, wasiCalled: [...wasi.called], instantiateMs: t1 - t0, firstRunMs: t2 - t1, secondRunMs: t3 - t2 };
}
