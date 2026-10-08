import { runEngine } from "./wasi-shim.mjs";
const file = process.argv[2] ?? new URL("./engine.opt.wasm", import.meta.url).pathname;
const bytes = await Bun.file(file).arrayBuffer();
console.log(JSON.stringify({ bytes: bytes.byteLength, ...(await runEngine(bytes)) }, null, 2));
