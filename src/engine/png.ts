/**
 * Square RGBA images as PNG files. Rows use the Sub filter (each byte minus the pixel to its left), which turns the smooth
 * gradients of a baked map into long runs of small numbers, and the data is deflated with the browser's `CompressionStream`
 * (the zlib format a PNG expects). Where that is missing the same bytes go out as a stored, uncompressed stream, which is
 * still a valid PNG, only larger.
 */
export async function rgbaPng(rgba: Uint8Array, size: number): Promise<Uint8Array> {
  const filtered = subFiltered(rgba, size);
  let compressed: Uint8Array;
  try {
    compressed = await deflate(filtered);
  } catch {
    compressed = storedZlib(filtered);
  }
  return assemble(size, compressed);
}

/** The same image without compression: the fallback, kept public so it can be tested. */
export function storedPng(rgba: Uint8Array, size: number): Uint8Array {
  return assemble(size, storedZlib(subFiltered(rgba, size)));
}

/** Each row prefixed with filter type 1 (Sub). */
function subFiltered(rgba: Uint8Array, size: number): Uint8Array {
  const stride = size * 4;
  const out = new Uint8Array(size * (stride + 1));
  for (let y = 0; y < size; y++) {
    const row = y * stride, target = y * (stride + 1);
    out[target] = 1;
    for (let x = 0; x < stride; x++) out[target + 1 + x] = (rgba[row + x] - (x >= 4 ? rgba[row + x - 4] : 0)) & 255;
  }
  return out;
}

async function deflate(bytes: Uint8Array): Promise<Uint8Array> {
  const stream = new Blob([bytes as BlobPart]).stream().pipeThrough(new CompressionStream('deflate'));
  return new Uint8Array(await new Response(stream).arrayBuffer());
}

/** A zlib stream of stored blocks: header, the data in blocks of at most 65535 bytes, and the Adler-32 checksum. */
function storedZlib(raw: Uint8Array): Uint8Array {
  const blocks = Math.max(1, Math.ceil(raw.length / 65535));
  const out = new Uint8Array(2 + raw.length + blocks * 5 + 4);
  out.set([0x78, 0x01]);
  let at = 2;
  for (let offset = 0, block = 0; block < blocks; block++) {
    const length = Math.min(65535, raw.length - offset);
    out.set([block === blocks - 1 ? 1 : 0, length & 255, length >>> 8, ~length & 255, (~length >>> 8) & 255], at);
    out.set(raw.subarray(offset, offset + length), at + 5);
    at += 5 + length;
    offset += length;
  }
  let a = 1, b = 0;
  for (const byte of raw) { a = (a + byte) % 65521; b = (b + a) % 65521; }
  out.set([(b >>> 8) & 255, b & 255, (a >>> 8) & 255, a & 255], at);
  return out;
}

function assemble(size: number, zlib: Uint8Array): Uint8Array {
  const chunks = [chunk('IHDR', header(size)), chunk('IDAT', zlib), chunk('IEND', new Uint8Array())];
  const out = new Uint8Array(8 + chunks.reduce((n, c) => n + c.length, 0));
  out.set([137, 80, 78, 71, 13, 10, 26, 10]);
  let at = 8;
  for (const part of chunks) { out.set(part, at); at += part.length; }
  return out;
}

/** Width, height, 8 bits per channel, colour type 6 (RGBA), default compression and filter, no interlace. */
function header(size: number): Uint8Array {
  const out = new Uint8Array(13), view = new DataView(out.buffer);
  view.setUint32(0, size);
  view.setUint32(4, size);
  out.set([8, 6, 0, 0, 0], 8);
  return out;
}

function chunk(name: string, data: Uint8Array): Uint8Array {
  const type = new TextEncoder().encode(name);
  const body = new Uint8Array(4 + data.length);
  body.set(type);
  body.set(data, 4);
  const out = new Uint8Array(body.length + 8), view = new DataView(out.buffer);
  view.setUint32(0, data.length);
  out.set(body, 4);
  view.setUint32(out.length - 4, crc32(body));
  return out;
}

function crc32(bytes: Uint8Array): number {
  let crc = 0xffffffff;
  for (const byte of bytes) {
    crc ^= byte;
    for (let i = 0; i < 8; i++) crc = (crc >>> 1) ^ ((crc & 1) ? 0xedb88320 : 0);
  }
  return (crc ^ 0xffffffff) >>> 0;
}
