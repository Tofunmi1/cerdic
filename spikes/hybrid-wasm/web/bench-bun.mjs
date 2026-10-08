import { bench } from "./bench.mjs";
const file = process.argv[2] ?? new URL("./engine.opt.wasm", import.meta.url).pathname;
console.log(JSON.stringify(await bench(await Bun.file(file).arrayBuffer())));
