// Run a WASI field measurement and report the module's retained linear-memory high-water size.
import { readFile } from 'node:fs/promises';
import { WASI } from 'node:wasi';

const [path, ...args] = process.argv.slice(2);
if (!path) throw new Error('usage: node tools/run-wasi-measure.mjs <program.wasm> [case]');
const wasi = new WASI({ version: 'preview1', args: [path, ...args], env: {}, returnOnExit: true });
const module = await WebAssembly.compile(await readFile(path));
const instance = await WebAssembly.instantiate(module, wasi.getImportObject());
const started = performance.now();
const exitCode = wasi.start(instance);
const elapsedMs = performance.now() - started;
const memoryBytes = instance.exports.memory?.buffer.byteLength ?? 0;
console.error(JSON.stringify({ wasi_elapsed_ms: elapsedMs, linear_memory_bytes: memoryBytes, exit_code: exitCode }));
process.exitCode = exitCode;
