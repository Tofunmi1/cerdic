import { brotliCompressSync, constants } from 'node:zlib';
import { readFileSync } from 'node:fs';

// Strip only custom sections in memory; measurements never alter the runtime artifact.
function stripped(bytes: Uint8Array): Uint8Array {
  const sections: Uint8Array[] = [bytes.subarray(0, 8)];
  let offset = 8;
  while (offset < bytes.length) {
    const start = offset;
    const id = bytes[offset++];
    let size = 0, shift = 0, byte: number;
    do {
      if (offset >= bytes.length || shift > 28) throw new Error('Invalid WASM section');
      byte = bytes[offset++]; size += (byte & 127) * 2 ** shift; shift += 7;
    } while (byte & 128);
    offset += size;
    if (offset > bytes.length) throw new Error('Truncated WASM section');
    if (id !== 0) sections.push(bytes.subarray(start, offset));
  }
  return Buffer.concat(sections);
}

const file = process.argv[2] ?? 'public/engine/ve_runtime_bg.wasm';
const raw = readFileSync(file);
const clean = stripped(raw);
if (!WebAssembly.validate(clean)) throw new Error('Stripped module failed validation');
const options = { params: { [constants.BROTLI_PARAM_QUALITY]: 9 } };
console.log(JSON.stringify({ file, quality: 9, raw: raw.length, brotli: brotliCompressSync(raw, options).length,
  stripped: clean.length, strippedBrotli: brotliCompressSync(clean, options).length }));
