// Runs a wasm32-wasip1 binary under Node's built-in WASI and forwards its stdout/stderr.
// Usage: node tools/run-wasi.mjs path/to/program.wasm [program args...]
import { readFile } from "node:fs/promises";
import { WASI } from "node:wasi";

const path = process.argv[2];
if (!path) {
  console.error("usage: node tools/run-wasi.mjs <program.wasm> [program args...]");
  process.exit(2);
}

const wasi = new WASI({ version: "preview1", args: [path, ...process.argv.slice(3)], env: {}, returnOnExit: true });
const module = await WebAssembly.compile(await readFile(path));
const instance = await WebAssembly.instantiate(module, wasi.getImportObject());
process.exitCode = wasi.start(instance);
