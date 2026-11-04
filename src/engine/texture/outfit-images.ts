// Picture maps for the imported clothes and hair (ADR 0055, amended): a texture program written `image(albedo="name", normal="name")` takes its maps from the
// CC0 pictures shipped next to the page in `outfits/`, instead of drawing them with nodes. The page fetches and decodes them before baking (`preloadOutfitImages`), so
// baking itself stays synchronous.
export type ImageProgram = { albedo?: string; normal?: string };

type Decoded = { width: number; height: number; bitmap: ImageBitmap };
const decoded = new Map<string, Decoded>();
const pending = new Map<string, Promise<void>>();

const NAME = /^[A-Za-z0-9_]{1,64}$/;

/** The parsed `image(...)` program, or undefined when the text is an ordinary node program. */
export function parseImageProgram(source: string | undefined): ImageProgram | undefined {
  const match = /^\s*image\(([^)]*)\)\s*$/.exec(source ?? '');
  // `image(name="...")` is the node of a larger texture program (see image-node.ts), not this whole-program form.
  if (!match || /\bname\s*=/.test(match[1])) return undefined;
  const program: ImageProgram = {};
  for (const part of match[1].split(',')) {
    const item = /^\s*(albedo|normal)\s*=\s*"([^"]*)"\s*$/.exec(part);
    if (!item) {
      if (part.trim() === '') continue;
      throw new Error(`image() takes albedo="name" and normal="name"; "${part.trim()}" is neither`);
    }
    if (!NAME.test(item[2])) throw new Error(`image name "${item[2]}" must be letters, digits and underscores`);
    program[item[1] as 'albedo' | 'normal'] = item[2];
  }
  if (!program.albedo && !program.normal) throw new Error('image() needs albedo="name" or normal="name"');
  return program;
}

/** Fetches and decodes every picture the programs name; resolves when all are ready. A missing picture rejects with the name and the status. */
export async function preloadOutfitImages(sources: Iterable<string | undefined>, base = './outfits/'): Promise<void> {
  const names = new Set<string>();
  for (const source of sources) {
    const program = parseImageProgram(source);
    if (program?.albedo) names.add(program.albedo);
    if (program?.normal) names.add(program.normal);
    // pictures used as nodes of a texture program: image(name="Bricks113A_Color", ...)
    for (const m of (source ?? '').matchAll(/\bimage\(\s*[^)]*?\bname\s*=\s*\\?"([A-Za-z0-9_]{1,64})\\?"/g)) names.add(m[1]);
  }
  await Promise.all([...names].map((name) => {
    if (decoded.has(name)) return Promise.resolve();
    let job = pending.get(name);
    if (!job) {
      job = (async () => {
        // a picture is a PNG (clothes, hair) or a JPEG (the photographic CC0 materials)
        let response = await fetch(new URL(`${name}.png`, new URL(base, document.baseURI)));
        if (!response.ok) response = await fetch(new URL(`${name}.jpg`, new URL(base, document.baseURI)));
        if (!response.ok) throw new Error(`picture "${name}": HTTP ${response.status}`);
        const bitmap = await createImageBitmap(await response.blob(), { premultiplyAlpha: 'none', colorSpaceConversion: 'none' });
        decoded.set(name, { width: bitmap.width, height: bitmap.height, bitmap });
      })().finally(() => pending.delete(name));
      pending.set(name, job);
    }
    return job;
  }));
}

type RawPicture = { width: number; height: number; pixels: Uint8Array };
const raw = new Map<string, RawPicture>();

/** Keeps a picture made by the engine itself (a simulation) as RGBA bytes, row 0 at v = 0, under `name` for the baker. */
export function registerRawPicture(name: string, width: number, height: number, pixels: Uint8Array): void {
  if (pixels.length !== width * height * 4) throw new Error(`picture "${name}": ${pixels.length} bytes is not ${width} by ${height} RGBA`);
  raw.set(name, { width, height, pixels });
}

/** The picture made by the engine under `name`, if there is one. */
export function rawPicture(name: string): RawPicture | undefined {
  return raw.get(name);
}

/** Whether a picture (decoded or made by the engine) is known under `name`. */
export function hasPicture(name: string): boolean {
  return raw.has(name) || decoded.has(name);
}

/** The decoded picture `name`, for the baker to upload as a texture. */
export function decodedPicture(name: string): ImageBitmap {
  const image = decoded.get(name);
  if (!image) throw new Error(`picture "${name}" was not loaded`);
  return image.bitmap;
}

/** The picture `name` drawn into a square of `size` pixels as RGBA bytes, first row at the bottom (the order the bake and the renderer use for UVs). */
export function outfitImagePixels(name: string, size: number): Uint8Array {
  const image = decoded.get(name);
  if (!image) throw new Error(`picture "${name}" was not loaded`);
  const canvas = new OffscreenCanvas(size, size);
  const context = canvas.getContext('2d', { willReadFrequently: true });
  if (!context) throw new Error('a 2d canvas is not available to draw the picture');
  context.imageSmoothingQuality = 'high';
  context.drawImage(image.bitmap, 0, 0, size, size);
  const top = context.getImageData(0, 0, size, size).data;
  const out = new Uint8Array(size * size * 4);
  for (let row = 0; row < size; row++) out.set(top.subarray(row * size * 4, (row + 1) * size * 4), (size - 1 - row) * size * 4);
  return out;
}
