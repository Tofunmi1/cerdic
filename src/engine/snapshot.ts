import { describeDiff, diffImages, type ImageDiff } from './snapshot-diff';

/** Widest picture kept, in pixels: enough to judge a shape, small enough to diff in a few milliseconds and to cost a model little to read. */
const MAX_WIDTH = 512;
/** JPEG quality of the picture handed to the model (the comparison never uses it: it works on raw pixels). */
const JPEG_QUALITY = 0.8;

type Shot = { width: number; height: number; rgba: Uint8ClampedArray };
const previous = new Map<string, Shot>();

/** The viewport canvas: the largest one on the page. */
function viewCanvas(): HTMLCanvasElement | null {
  let best: HTMLCanvasElement | null = null, area = 0;
  for (const canvas of document.querySelectorAll('canvas')) {
    const size = canvas.clientWidth * canvas.clientHeight;
    if (size > area) { best = canvas; area = size; }
  }
  return best;
}

/**
 * Copies the viewport into a small off-screen canvas. The copy happens inside one animation-frame callback, which runs after the
 * engine drew that frame and before it is shown, while the drawing buffer still holds the picture. One frame, no PNG round trip.
 */
function capture(maxWidth: number): Promise<{ shot: Shot; canvas: HTMLCanvasElement } | string> {
  const source = viewCanvas();
  if (!source || source.width === 0 || source.height === 0) return Promise.resolve('there is no viewport to capture');
  const width = Math.min(maxWidth, source.width), height = Math.max(1, Math.round((source.height * width) / source.width));
  const target = document.createElement('canvas');
  target.width = width;
  target.height = height;
  const context = target.getContext('2d', { willReadFrequently: true });
  if (!context) return Promise.resolve('the page cannot read back the viewport');
  return new Promise(resolve => requestAnimationFrame(() => {
    context.drawImage(source, 0, 0, width, height);
    const rgba = context.getImageData(0, 0, width, height).data;
    // A buffer already cleared by the browser reads back fully transparent.
    let seen = false;
    for (let i = 3; i < rgba.length; i += 4 * 97) if (rgba[i] !== 0) { seen = true; break; }
    resolve(seen ? { shot: { width, height, rgba }, canvas: target } : 'the viewport read back empty; try again');
  }));
}

/**
 * The `snapshot` tool: a small picture of the view, plus what changed since the last snapshot of the same `name`.
 * `image: false` skips the picture and returns only the comparison, which is the cheapest way to ask "did that edit change anything".
 */
export async function snapshotView(params: Record<string, unknown>) {
  const name = typeof params.name === 'string' && params.name.trim() ? params.name.trim().slice(0, 32) : 'last';
  const wide = typeof params.width === 'number' && Number.isFinite(params.width) ? Math.round(params.width) : MAX_WIDTH;
  const taken = await capture(Math.min(MAX_WIDTH, Math.max(64, wide)));
  if (typeof taken === 'string') return { ok: false, reason: taken };
  const { shot, canvas } = taken;
  const before = previous.get(name);
  previous.set(name, shot);
  let compared: { diff: ImageDiff; text: string } | { text: string } | undefined;
  if (before) {
    if (before.width === shot.width && before.height === shot.height) {
      const diff = diffImages(before.rgba, shot.rgba, shot.width, shot.height);
      compared = { diff, text: describeDiff(diff) };
    } else {
      compared = { text: `the view size changed (${before.width} x ${before.height} to ${shot.width} x ${shot.height}), so nothing was compared` };
    }
  }
  const image = params.image === false ? undefined : canvas.toDataURL('image/jpeg', JPEG_QUALITY).split(',')[1];
  return {
    ok: true, name, width: shot.width, height: shot.height,
    compared: compared ?? 'no earlier snapshot with this name; the next one is compared with this',
    ...(image ? { jpegBase64: image } : {}),
  };
}
