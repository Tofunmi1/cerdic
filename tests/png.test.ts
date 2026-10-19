import { expect, test } from 'bun:test';
import { rgbaPng, storedPng } from '../src/engine/png';

/** A smooth gradient with a little structure, like a baked map. */
function gradient(size: number): Uint8Array {
  const out = new Uint8Array(size * size * 4);
  for (let y = 0; y < size; y++) for (let x = 0; x < size; x++) {
    out.set([Math.round((x / size) * 255), Math.round((y / size) * 255), 128 + ((x ^ y) & 7), (x + y) % 5 === 0 ? 0 : 255], (y * size + x) * 4);
  }
  return out;
}

/** Reads a PNG back: the header fields, and the pixels after inflating the image data and undoing the row filters. */
async function decode(png: Uint8Array): Promise<{ width: number; height: number; pixels: Uint8Array }> {
  expect([...png.slice(0, 8)]).toEqual([137, 80, 78, 71, 13, 10, 26, 10]);
  const view = new DataView(png.buffer, png.byteOffset, png.byteLength);
  let at = 8, width = 0, height = 0;
  const data: Uint8Array[] = [];
  while (at < png.length) {
    const length = view.getUint32(at), name = String.fromCharCode(...png.slice(at + 4, at + 8));
    const body = png.slice(at + 8, at + 8 + length);
    if (name === 'IHDR') { width = new DataView(body.buffer).getUint32(0); height = new DataView(body.buffer).getUint32(4); expect([...body.slice(8)]).toEqual([8, 6, 0, 0, 0]); }
    if (name === 'IDAT') data.push(body);
    at += 12 + length;
  }
  const joined = new Uint8Array(data.reduce((n, d) => n + d.length, 0));
  data.reduce((offset, d) => { joined.set(d, offset); return offset + d.length; }, 0);
  const inflated = new Uint8Array(await new Response(new Blob([joined]).stream().pipeThrough(new DecompressionStream('deflate'))).arrayBuffer());
  const stride = width * 4, pixels = new Uint8Array(width * height * 4);
  for (let y = 0; y < height; y++) {
    const filter = inflated[y * (stride + 1)];
    expect(filter).toBe(1);
    for (let x = 0; x < stride; x++) pixels[y * stride + x] = (inflated[y * (stride + 1) + 1 + x] + (x >= 4 ? pixels[y * stride + x - 4] : 0)) & 255;
  }
  return { width, height, pixels };
}

test('the PNG decodes back to exactly the pixels that went in', async () => {
  const image = gradient(64);
  const decoded = await decode(await rgbaPng(image, 64));
  expect([decoded.width, decoded.height]).toEqual([64, 64]);
  expect(decoded.pixels).toEqual(image);
});

test('a smooth map compresses to a small fraction of its raw size', async () => {
  const image = gradient(256);
  const png = await rgbaPng(image, 256);
  const stored = storedPng(image, 256);
  expect(png.length).toBeLessThan(image.length * 0.2);
  expect(png.length).toBeLessThan(stored.length * 0.2);
  // A flat image is tiny.
  const flat = await rgbaPng(new Uint8Array(256 * 256 * 4).fill(128), 256);
  expect(flat.length).toBeLessThan(2000);
});

test('the uncompressed fallback is a valid PNG with the same pixels', async () => {
  const image = gradient(32);
  const decoded = await decode(storedPng(image, 32));
  expect(decoded.pixels).toEqual(image);
});

test('an image larger than one stored block still decodes', async () => {
  // 200 x 200 RGBA is 160,200 filtered bytes: three stored blocks.
  const image = gradient(200);
  expect((await decode(storedPng(image, 200))).pixels).toEqual(image);
});
