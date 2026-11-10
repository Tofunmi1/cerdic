import { describeDiff, diffImages, type ImageDiff } from './snapshot-diff';
import { validateToolArguments } from '../../engine/web/tool-arguments.js';

/** Widest picture kept, in pixels: enough to judge a shape, small enough to diff in a few milliseconds and to cost a model little to read. */
const DEFAULT_WIDTH = 512;
/** The widest a caller may ask for with `width` (a close-up of detail needs more than the default; the default stays small). */
const MAX_WIDTH = 1280;
/** JPEG quality of the picture handed to the model (the comparison never uses it: it works on raw pixels). */
const JPEG_QUALITY = 0.8;

type Shot = { width: number; height: number; rgba: Uint8ClampedArray };
const previous = new Map<string, Shot>();

/** The viewport canvas: the largest one on the page. */
function viewCanvas(): HTMLCanvasElement | null {
  let best: HTMLCanvasElement | null = null, area = 0;
  for (const canvas of Array.from(document.querySelectorAll('canvas'))) {
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
  return new Promise((resolve, reject) => requestAnimationFrame(() => {
    try {
      context.drawImage(source, 0, 0, width, height);
      const rgba = context.getImageData(0, 0, width, height).data;
      // A buffer already cleared by the browser reads back fully transparent.
      let seen = false;
      for (let i = 3; i < rgba.length; i += 4 * 97) if (rgba[i] !== 0) { seen = true; break; }
      resolve(seen ? { shot: { width, height, rgba }, canvas: target } : 'the viewport read back empty; try again');
    } catch (error) { reject(error); }
  }));
}

type Box = { min: number[]; max: number[] };
type Camera = { position: number[]; target: number[]; label?: string };
/** What the contact sheet needs from the page: the camera, and the extents of the scene and of single parts. */
export type ViewHooks = {
  lookAt: (position: number[], target: number[]) => boolean;
  pose: () => { position: number[]; rotation: number[] } | null;
  bounds: () => Box | null;
  partBounds: (name: string) => Box | null;
  /** Motion sheets: pause the animation clock at `seconds`. */
  seek?: (seconds: number) => void;
  /** Motion sheets: run or pause the animation clock where it is. */
  play?: (on: boolean, seconds?: number) => void;
  /** Current shared clock, so a motion sheet can restore time and run state. */
  clock?: () => { seconds: number; playing: boolean };
  /** Motion sheets: set a parameter of the animation state machines (a number or a true/false). */
  setParam?: (name: string, value: number | boolean) => void;
};

/** Named cameras, as the direction from the subject towards the camera (not normalised). */
const VIEW_DIRECTIONS: Record<string, [number, number, number]> = {
  front: [0, 0.2, 1], back: [0, 0.2, -1], side: [1, 0.2, 0], left: [-1, 0.2, 0], top: [0, 1, 0.02], iso: [1, 0.8, 1],
};
const MAX_VIEWS = 6;
const settle = () => new Promise<void>(resolve => requestAnimationFrame(() => requestAnimationFrame(() => requestAnimationFrame(() => setTimeout(resolve, 120)))));

/** Rotates the camera's forward axis (minus Z) by a unit quaternion `[x, y, z, w]`, to put the camera back where it was. */
function forwardOf([x, y, z, w]: number[]): number[] {
  const v = [0, 0, -1];
  const c = [y * v[2] - z * v[1], z * v[0] - x * v[2], x * v[1] - y * v[0]];
  const cc = [y * c[2] - z * c[1], z * c[0] - x * c[2], x * c[1] - y * c[0]];
  return [v[0] + 2 * (w * c[0] + cc[0]), v[1] + 2 * (w * c[1] + cc[1]), v[2] + 2 * (w * c[2] + cc[2])];
}

/**
 * Several named views of the scene (or of one `focus` part) in one picture: far fewer tokens than separate images, and every angle
 * is taken from the same distance so the sizes compare. The camera goes back where it was afterwards.
 */
async function contactSheet(params: Record<string, unknown>, hooks: ViewHooks) {
  const views = Array.isArray(params.views) ? params.views.map(String) : [];
  const unknown = views.filter(view => !(view in VIEW_DIRECTIONS));
  if (unknown.length > 0) return { ok: false, reason: `unknown view ${unknown.join(', ')}; use ${Object.keys(VIEW_DIRECTIONS).join(', ')}` };
  if (views.length > MAX_VIEWS) return { ok: false, reason: `at most ${MAX_VIEWS} views in one sheet` };
  const focus = typeof params.focus === 'string' && params.focus.trim() ? params.focus.trim() : '';
  let cameras = params.cameras as Camera[] | undefined;
  if (!cameras) {
    const box = focus ? hooks.partBounds(focus) : hooks.bounds();
    if (!box) return { ok: false, reason: focus ? `no part named ${focus} is on screen` : 'there is no model to look at yet' };
    const centre = box.min.map((v, i) => (v + box.max[i]) / 2);
    const distance = Math.max(Math.hypot(...box.max.map((v, i) => v - box.min[i])) * 1.1, 0.3);
    cameras = views.map(view => {
      const direction = VIEW_DIRECTIONS[view];
      const length = Math.hypot(...direction);
      return { position: centre.map((v, i) => v + direction[i] / length * distance), target: centre, label: view };
    });
  }
  const labels = cameras.map((camera, i) => camera.label ?? `camera ${i + 1}`);
  const wide = typeof params.width === 'number' && Number.isFinite(params.width) ? Math.round(params.width) : DEFAULT_WIDTH;
  const columns = cameras.length === 1 ? 1 : 2, rows = Math.ceil(cameras.length / columns);
  const cellWidth = Math.floor(Math.min(MAX_WIDTH, Math.max(128, wide)) / columns);
  const saved = hooks.pose();
  const cells: HTMLCanvasElement[] = [];
  try {
    for (const camera of cameras) {
      if (!hooks.lookAt(camera.position, camera.target)) return { ok: false, reason: 'the page could not set the camera' };
      await settle();
      const taken = await capture(cellWidth);
      if (typeof taken === 'string') return { ok: false, reason: taken };
      cells.push(taken.canvas);
    }
  } finally {
    if (saved) restore(hooks, saved);
  }
  const cellHeight = cells[0].height;
  const sheet = document.createElement('canvas');
  sheet.width = cellWidth * columns;
  sheet.height = cellHeight * rows;
  const context = sheet.getContext('2d');
  if (!context) return { ok: false, reason: 'the page cannot compose a contact sheet' };
  context.fillStyle = '#000';
  context.fillRect(0, 0, sheet.width, sheet.height);
  cells.forEach((cell, i) => {
    const x = (i % columns) * cellWidth, y = Math.floor(i / columns) * cellHeight;
    context.drawImage(cell, x, y);
    context.font = '12px sans-serif';
    context.fillStyle = 'rgba(0,0,0,0.6)';
    context.fillRect(x + 4, y + 4, labels[i].length * 7 + 10, 18);
    context.fillStyle = '#fff';
    context.fillText(labels[i], x + 9, y + 17);
  });
  const image = params.image === false ? undefined : sheet.toDataURL('image/jpeg', JPEG_QUALITY).split(',')[1];
  return { ok: true, ...(params.cameras ? { cameras: cameras.map((camera, i) => ({ ...camera, label: labels[i] })) } : { views, ...(focus ? { focus } : {}) }), width: sheet.width, height: sheet.height, ...(image ? { jpegBase64: image } : {}) };
}

const MAX_MOTION_FRAMES = 8;
const MOTION_SETTLE_MS = 1200;

/**
 * An animated thing at several moments in one picture, from one camera: `times` pauses the animation clock at each time (clips played
 * from the clock), `states` sets a machine's parameters for each frame and lets it run for a second so its blend settles (a character at
 * speed 0, then 1.5). The same framing and distance for every frame, each cell labelled; the camera and the clock go back afterwards.
 */
async function motionSheet(params: Record<string, unknown>, hooks: ViewHooks) {
  if (!hooks.seek || !hooks.play || !hooks.setParam) return { ok: false, reason: 'this page cannot move the animation clock' };
  const states = Array.isArray(params.states) ? params.states as Record<string, unknown>[] : [];
  const times = Array.isArray(params.times) ? params.times.map(Number) : [];
  const frames = states.length > 0 ? states.length : times.length;
  if (frames === 0 || frames > MAX_MOTION_FRAMES) return { ok: false, reason: `give 1 to ${MAX_MOTION_FRAMES} times or states` };
  if (states.length === 0 && times.some(t => !Number.isFinite(t) || t < 0 || t > 600)) return { ok: false, reason: 'times are seconds from 0 to 600' };
  const view = typeof params.view === 'string' ? params.view : 'side';
  if (!(view in VIEW_DIRECTIONS)) return { ok: false, reason: `unknown view ${view}; use ${Object.keys(VIEW_DIRECTIONS).join(', ')}` };
  const focus = typeof params.focus === 'string' && params.focus.trim() ? params.focus.trim() : '';
  const box = focus ? hooks.partBounds(focus) : hooks.bounds();
  if (!box) return { ok: false, reason: focus ? `no part named ${focus} is on screen` : 'there is no model to look at yet' };
  const centre = box.min.map((v, i) => (v + box.max[i]) / 2);
  const distance = Math.max(Math.hypot(...box.max.map((v, i) => v - box.min[i])) * 1.1, 0.3);
  const [dx, dy, dz] = VIEW_DIRECTIONS[view];
  const length = Math.hypot(dx, dy, dz);
  const saved = hooks.pose();
  const savedClock = hooks.clock?.();
  try {
    hooks.lookAt([centre[0] + (dx / length) * distance, centre[1] + (dy / length) * distance, centre[2] + (dz / length) * distance], centre);
    const columns = Math.min(4, frames), rows = Math.ceil(frames / columns);
    const cellWidth = Math.floor(Math.min(1024, 256 * columns) / columns);
    const cells: HTMLCanvasElement[] = [];
    const labels: string[] = [];
    for (let i = 0; i < frames; i++) {
      if (states.length > 0) {
        for (const [name, value] of Object.entries(states[i] ?? {})) {
          if (typeof value === 'boolean' || (typeof value === 'number' && Number.isFinite(value))) hooks.setParam(name, value);
        }
        hooks.play(true);
        await new Promise<void>(resolve => setTimeout(resolve, MOTION_SETTLE_MS));
        labels.push(Object.entries(states[i] ?? {}).map(([k, v]) => `${k}=${v}`).join(' ') || 'no change');
      } else {
        hooks.play(false);
        hooks.seek(times[i]);
        await settle();
        labels.push(`t=${times[i]}s`);
      }
      const taken = await capture(cellWidth);
      if (typeof taken === 'string') return { ok: false, reason: taken };
      cells.push(taken.canvas);
    }
    const cellHeight = cells[0].height;
    const sheet = document.createElement('canvas');
    sheet.width = cellWidth * columns;
    sheet.height = cellHeight * rows;
    const context = sheet.getContext('2d');
    if (!context) return { ok: false, reason: 'the page cannot compose a motion sheet' };
    context.fillStyle = '#000';
    context.fillRect(0, 0, sheet.width, sheet.height);
    cells.forEach((cell, i) => {
      const x = (i % columns) * cellWidth, y = Math.floor(i / columns) * cellHeight;
      context.drawImage(cell, x, y);
      context.font = '12px sans-serif';
      context.fillStyle = 'rgba(0,0,0,0.6)';
      context.fillRect(x + 4, y + 4, labels[i].length * 7 + 10, 18);
      context.fillStyle = '#fff';
      context.fillText(labels[i], x + 9, y + 17);
    });
    const image = params.image === false ? undefined : sheet.toDataURL('image/jpeg', JPEG_QUALITY).split(',')[1];
    return { ok: true, view, frames: labels, ...(focus ? { focus } : {}), width: sheet.width, height: sheet.height, ...(image ? { jpegBase64: image } : {}) };
  } finally {
    if (saved) restore(hooks, saved);
    if (savedClock) {
      hooks.play(savedClock.playing, savedClock.seconds);
    }
  }
}

function restore(hooks: ViewHooks, pose: { position: number[]; rotation: number[] }) {
  const forward = forwardOf(pose.rotation);
  hooks.lookAt(pose.position, pose.position.map((v, i) => v + forward[i]));
}

/**
 * The `snapshot` tool: a small picture of the view, plus what changed since the last snapshot of the same `name`.
 * `image: false` skips the picture and returns only the comparison, which is the cheapest way to ask "did that edit change anything".
 */
export async function snapshotView(params: Record<string, unknown>, hooks?: ViewHooks) {
  const invalid = validateToolArguments('snapshot', params);
  if (invalid) return { ok: false, reason: invalid };
  if (params.camera !== undefined) {
    if (!hooks) return { ok: false, reason: 'this page cannot move the camera' };
    const camera = params.camera as Camera;
    const saved = hooks.pose();
    try {
      if (!hooks.lookAt(camera.position, camera.target)) return { ok: false, reason: 'the page could not set the camera' };
      await settle();
      return { ...await snapshotCurrent(params), camera };
    } finally {
      if (saved) restore(hooks, saved);
    }
  }
  if (params.cameras !== undefined) return hooks ? contactSheet(params, hooks) : { ok: false, reason: 'this page cannot move the camera' };
  if ((Array.isArray(params.times) && params.times.length > 0) || (Array.isArray(params.states) && params.states.length > 0)) return hooks ? motionSheet(params, hooks) : { ok: false, reason: 'this page cannot move the camera' };
  if (Array.isArray(params.views) && params.views.length > 0) return hooks ? contactSheet(params, hooks) : { ok: false, reason: 'this page cannot move the camera' };
  return snapshotCurrent(params);
}

async function snapshotCurrent(params: Record<string, unknown>) {
  const name = typeof params.name === 'string' && params.name.trim() ? params.name.trim().slice(0, 32) : 'last';
  const wide = typeof params.width === 'number' && Number.isFinite(params.width) ? Math.round(params.width) : DEFAULT_WIDTH;
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
