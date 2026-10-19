/** Encodes a square RGBA image as a PNG using a stored zlib stream. */
export function rgbaPng(rgba: Uint8Array, size: number): Uint8Array {
  const raw = new Uint8Array(size * (size * 4 + 1));
  for (let y = 0; y < size; y++) raw.set(rgba.subarray(y * size * 4, (y + 1) * size * 4), y * (size * 4 + 1) + 1);
  const blocks: number[] = [0x78, 0x01];
  for (let offset = 0; offset < raw.length;) {
    const length = Math.min(65535, raw.length - offset), final = offset + length === raw.length;
    blocks.push(final ? 1 : 0, length & 255, length >>> 8, (~length) & 255, ((~length) >>> 8) & 255);
    for (let i = 0; i < length; i++) blocks.push(raw[offset + i]);
    offset += length;
  }
  let a = 1, b = 0;
  for (const byte of raw) { a = (a + byte) % 65521; b = (b + a) % 65521; }
  blocks.push((b >>> 8) & 255, b & 255, (a >>> 8) & 255, a & 255);
  const chunks = [pngChunk('IHDR', uint32(size, size, 8, 6, 0, 0, 0)), pngChunk('IDAT', Uint8Array.from(blocks)), pngChunk('IEND', new Uint8Array())];
  const out = new Uint8Array(8 + chunks.reduce((n, c) => n + c.length, 0));
  out.set([137, 80, 78, 71, 13, 10, 26, 10]);
  let at = 8;
  for (const chunk of chunks) { out.set(chunk, at); at += chunk.length; }
  return out;
}

function uint32(width: number, height: number, depth: number, color: number, compression: number, filter: number, interlace: number) {
  const out = new Uint8Array(13), view = new DataView(out.buffer);
  view.setUint32(0, width); view.setUint32(4, height); out.set([depth, color, compression, filter, interlace], 8); return out;
}
function pngChunk(name: string, data: Uint8Array): Uint8Array {
  const type = new TextEncoder().encode(name), body = new Uint8Array(type.length + data.length); body.set(type); body.set(data, 4);
  const out = new Uint8Array(body.length + 8), view = new DataView(out.buffer); view.setUint32(0, data.length); out.set(body, 4); view.setUint32(out.length - 4, crc32(body)); return out;
}
function crc32(bytes: Uint8Array) { let crc = 0xffffffff; for (const byte of bytes) { crc ^= byte; for (let i = 0; i < 8; i++) crc = (crc >>> 1) ^ ((crc & 1) ? 0xedb88320 : 0); } return (crc ^ 0xffffffff) >>> 0; }
