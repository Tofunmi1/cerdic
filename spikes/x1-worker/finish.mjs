// Points the wasm-bindgen glue at the local wasi shim and copies the shim next to it (what
// `cargo xtask web` does for the full runtime), then prints the module sizes.
import fs from "node:fs";
import zlib from "node:zlib";

const glue = "web/x1_eval.js";
// Every import of the module name must be repointed (a bare specifier cannot load in a browser).
fs.writeFileSync(glue, fs.readFileSync(glue, "utf8").replaceAll('from "wasi_snapshot_preview1"', 'from "./wasi_snapshot_preview1.js"'));
fs.copyFileSync("../../engine/web/wasi_snapshot_preview1.js", "web/wasi_snapshot_preview1.js");

const bytes = fs.readFileSync("web/x1_eval_bg.wasm");
const leb = (buf, p) => { let r = 0, s = 0, x; do { x = buf[p++]; r |= (x & 0x7f) << s; s += 7; } while (x & 0x80); return [r, p]; };
let o = 8; const keep = [bytes.subarray(0, 8)];
while (o < bytes.length) { const id = bytes[o]; const [len, p] = leb(bytes, o + 1); const end = p + len; if (id !== 0) keep.push(bytes.subarray(o, end)); o = end; }
const stripped = Buffer.concat(keep);
const q = { params: { [zlib.constants.BROTLI_PARAM_QUALITY]: 9, [zlib.constants.BROTLI_PARAM_LGWIN]: 24 } };
console.log(JSON.stringify({ raw: bytes.length, stripped: stripped.length, strippedBrotli9: zlib.brotliCompressSync(stripped, q).length }));
