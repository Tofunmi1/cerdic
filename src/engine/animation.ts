// Keyframed transform tracks (`anim.track`) for export. The viewer plays tracks in Rust (`ve-runtime`'s
// animation module) and the maths here is the same: a track is sampled at a time, each track acts in the
// order listed in world space (position adds a translation, rotation and scale act about the track's pivot),
// and the result is a transform. glTF wants translation, rotation and scale per node, so the clip is sampled
// into those at the key times plus a few points inside every curved segment.
import { backOut, bezierSample, bounceOut, elasticOut } from './easing';
import type { AnimationTrack } from './worker-types';

type Vec3 = [number, number, number];
/** A 4 x 4 matrix, column-major (glTF's and glam's layout). */
type Mat4 = number[];

const IDENTITY: Mat4 = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];
/** Samples inside each segment that is not a straight translation, so spherical interpolation stays close. */
const SUBSAMPLES = 8;
/** Most samples one clip exports. */
const MAX_SAMPLES = 1024;

/** The value of a track `seconds` into the clip (the same rules as `Track::sample` in ve-modeling). */
export function sampleTrack(track: AnimationTrack, seconds: number): Vec3 {
  const keys = track.keys;
  const first = keys[0], last = keys[keys.length - 1];
  if (!first || !last) return [0, 0, 0];
  const span = last.time - first.time;
  const t = track.looped && span > 0
    ? first.time + (((seconds - first.time) % span) + span) % span
    : Math.min(Math.max(seconds, first.time), last.time);
  if (t >= last.time) return [...last.value];
  let next = keys.findIndex(key => key.time > t);
  if (next < 0) next = keys.length - 1;
  const a = keys[Math.max(next - 1, 0)], b = keys[next];
  const fraction = (t - a.time) / (b.time - a.time);
  if (track.interpolation === 'bezier') {
    const segment = Math.max(next - 1, 0);
    return [0, 1, 2].map(axis => bezierSample(keys, segment, t, axis)) as Vec3;
  }
  const eased = track.interpolation === 'linear' ? fraction
    : track.interpolation === 'smooth' ? fraction * fraction * (3 - 2 * fraction)
    : track.interpolation === 'ease_in' ? fraction * fraction
    : track.interpolation === 'ease_out' ? 1 - (1 - fraction) * (1 - fraction)
    : track.interpolation === 'back' ? backOut(fraction)
    : track.interpolation === 'bounce' ? bounceOut(fraction)
    : track.interpolation === 'elastic' ? elasticOut(fraction) : 0;
  return [0, 1, 2].map(axis => a.value[axis] + (b.value[axis] - a.value[axis]) * eased) as Vec3;
}

function multiply(a: Mat4, b: Mat4): Mat4 {
  const out = new Array<number>(16).fill(0);
  for (let column = 0; column < 4; column++) {
    for (let row = 0; row < 4; row++) {
      for (let k = 0; k < 4; k++) out[column * 4 + row] += a[k * 4 + row] * b[column * 4 + k];
    }
  }
  return out;
}

const translation = (v: Vec3): Mat4 => [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, v[0], v[1], v[2], 1];
const scaling = (v: Vec3): Mat4 => [v[0], 0, 0, 0, 0, v[1], 0, 0, 0, 0, v[2], 0, 0, 0, 0, 1];

/** Rotation by Euler angles in degrees, applied in the order Y, X, Z (glam's `EulerRot::YXZ`). */
function rotation(degrees: Vec3): Mat4 {
  const [x, y, z] = degrees.map(d => (d * Math.PI) / 180);
  const [sx, cx, sy, cy, sz, cz] = [Math.sin(x), Math.cos(x), Math.sin(y), Math.cos(y), Math.sin(z), Math.cos(z)];
  const rx: Mat4 = [1, 0, 0, 0, 0, cx, sx, 0, 0, -sx, cx, 0, 0, 0, 0, 1];
  const ry: Mat4 = [cy, 0, -sy, 0, 0, 1, 0, 0, sy, 0, cy, 0, 0, 0, 0, 1];
  const rz: Mat4 = [cz, sz, 0, 0, -sz, cz, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];
  return multiply(multiply(ry, rx), rz);
}

/** A fraction in 0 up to 1 that depends only on the copy's index: the same integer mix as `jitter_fraction` in ve-modeling. */
export function jitterFraction(index: number): number {
  let x = (index + 0x9e3779b9) >>> 0;
  x = Math.imul(x ^ (x >>> 16), 0x21f0aaad) >>> 0;
  x = Math.imul(x ^ (x >>> 15), 0x735a2d97) >>> 0;
  x = (x ^ (x >>> 15)) >>> 0;
  return (x >>> 8) / 16777216;
}

/** Seconds copy `index` of a placed set plays ahead of the clock on this track (`Track::phase`). */
export const trackPhase = (track: AnimationTrack, index: number): number =>
  (track.phase_step ?? 0) * index + (track.phase_jitter ?? 0) * jitterFraction(index);

/** The transform the tracks give a part `seconds` into the clip (a placed copy plays each track `phase(track)` ahead). */
export function poseMatrix(tracks: AnimationTrack[], seconds: number, phase?: (track: AnimationTrack) => number): Mat4 {
  let matrix = IDENTITY;
  for (const track of tracks) {
    const value = sampleTrack(track, seconds + (phase?.(track) ?? 0));
    const about = (change: Mat4) => multiply(multiply(translation(track.pivot), change), translation(track.pivot.map(v => -v) as Vec3));
    if (track.property === 'glow' || track.property === 'visible' || track.property === 'color') continue;
    const step = track.property === 'position' ? translation(value)
      : track.property === 'rotation' ? about(rotation(value)) : about(scaling(value));
    matrix = multiply(step, matrix);
  }
  return matrix;
}

export type Trs = { translation: Vec3; rotation: [number, number, number, number]; scale: Vec3 };

/** Splits a translate-rotate-scale matrix into its parts (no shear is expected from tracks). */
export function decompose(m: Mat4): Trs {
  const scale: Vec3 = [Math.hypot(m[0], m[1], m[2]), Math.hypot(m[4], m[5], m[6]), Math.hypot(m[8], m[9], m[10])];
  const r = [m[0] / scale[0], m[1] / scale[0], m[2] / scale[0], m[4] / scale[1], m[5] / scale[1], m[6] / scale[1], m[8] / scale[2], m[9] / scale[2], m[10] / scale[2]];
  // r is column-major: r[0..3] is the X axis column, r[3..6] the Y axis, r[6..9] the Z axis.
  const [m00, m10, m20, m01, m11, m21, m02, m12, m22] = r;
  const trace = m00 + m11 + m22;
  let q: [number, number, number, number];
  if (trace > 0) {
    const s = Math.sqrt(trace + 1) * 2;
    q = [(m21 - m12) / s, (m02 - m20) / s, (m10 - m01) / s, s / 4];
  } else if (m00 > m11 && m00 > m22) {
    const s = Math.sqrt(1 + m00 - m11 - m22) * 2;
    q = [s / 4, (m01 + m10) / s, (m02 + m20) / s, (m21 - m12) / s];
  } else if (m11 > m22) {
    const s = Math.sqrt(1 + m11 - m00 - m22) * 2;
    q = [(m01 + m10) / s, s / 4, (m12 + m21) / s, (m02 - m20) / s];
  } else {
    const s = Math.sqrt(1 + m22 - m00 - m11) * 2;
    q = [(m02 + m20) / s, (m12 + m21) / s, s / 4, (m10 - m01) / s];
  }
  const length = Math.hypot(...q) || 1;
  return { translation: [m[12], m[13], m[14]], rotation: q.map(v => v / length) as Trs['rotation'], scale };
}

/** The times, in seconds, at which a part's clip is sampled for export: sorted, from 0, without repeats. */
export function sampleTimes(tracks: AnimationTrack[]): number[] {
  const duration = Math.max(0, ...tracks.map(track => track.keys[track.keys.length - 1]?.time ?? 0));
  const times = new Set<number>([0, duration]);
  for (const track of tracks) {
    const first = track.keys[0].time, last = track.keys[track.keys.length - 1].time;
    const span = last - first;
    // A looping track that is shorter than the clip repeats; its keys repeat with it.
    const repeats = track.looped && span > 0 ? Math.ceil(duration / span) : 1;
    for (let cycle = 0; cycle < repeats; cycle++) {
      const offset = cycle * span;
      track.keys.forEach((key, i) => {
        const at = key.time + offset;
        if (at > duration + 1e-9) return;
        times.add(at);
        if (track.interpolation === 'step' && i > 0) times.add(Math.max(0, at - 1e-4)); // hold, then jump
        const next = track.keys[i + 1];
        if (next && track.interpolation !== 'step' && (track.interpolation !== 'linear' || track.property !== 'position')) {
          // Bounces and springs have sharp turns, so they get more samples inside each segment.
          const inside = track.interpolation === 'bounce' || track.interpolation === 'elastic' ? 4 * SUBSAMPLES : SUBSAMPLES;
          for (let k = 1; k <= inside; k++) times.add(key.time + offset + ((next.time - key.time) * k) / (inside + 1));
        }
      });
    }
  }
  // Samples from different tracks can land a hair apart; glTF needs strictly increasing times in 32-bit floats,
  // so anything closer than 10 microseconds to the previous sample is merged into it.
  const sorted: number[] = [];
  for (const t of [...times].filter(t => t >= 0 && t <= duration + 1e-9).sort((a, b) => a - b)) {
    if (sorted.length === 0 || t - sorted[sorted.length - 1] > 1e-5) sorted.push(t);
    else if (t === duration) sorted[sorted.length - 1] = t; // the clip's end stays exact
  }
  if (sorted.length <= MAX_SAMPLES) return sorted;
  return Array.from({ length: MAX_SAMPLES }, (_, i) => (duration * i) / (MAX_SAMPLES - 1));
}

export type SampledClip = { times: Float32Array; translations: Float32Array; rotations: Float32Array; scales: Float32Array };

/** A material property sampled over time: one number per sample for glow, red, green and blue for colour. */
export type LookClip = { times: Float32Array; values: Float32Array };

/** Samples a part's `glow` tracks (their product), its last `color` track, or whether it is `visible` (1 or 0), at the key times and inside curved segments. */
export function sampleLook(tracks: AnimationTrack[], property: 'glow' | 'color' | 'visible'): LookClip | null {
  const subset = tracks.filter(track => track.property === property);
  if (subset.length === 0) return null;
  const times = sampleTimes(subset);
  const width = property === 'color' ? 3 : 1;
  const values = new Float32Array(times.length * width);
  times.forEach((t, i) => {
    if (property === 'glow') values[i] = subset.reduce((product, track) => product * sampleTrack(track, t)[0], 1);
    else if (property === 'visible') values[i] = subset.every(track => sampleTrack(track, t)[0] >= 0.5) ? 1 : 0; // shown only if every track says so
    else values.set(sampleTrack(subset[subset.length - 1], t), i * 3);
  });
  return { times: Float32Array.from(times), values };
}

/** A part's tracks sampled into per-node translation, rotation and scale (glTF's animation channels). */
export function sampleClip(tracks: AnimationTrack[], placed?: { base: Mat4; index: number }): SampledClip {
  const times = sampleTimes(tracks);
  const out: SampledClip = {
    times: Float32Array.from(times),
    translations: new Float32Array(times.length * 3),
    rotations: new Float32Array(times.length * 4),
    scales: new Float32Array(times.length * 3),
  };
  let previous: Trs['rotation'] | undefined;
  times.forEach((t, i) => {
    // A placed copy: its own placement first-class in the node's transform, then its tracks in its own frame.
    const pose = poseMatrix(tracks, t, placed ? track => trackPhase(track, placed.index) : undefined);
    const trs = decompose(placed ? multiply(placed.base, pose) : pose);
    let q = trs.rotation;
    // Keep neighbouring quaternions on the same side of the sphere so interpolation takes the short way.
    if (previous && previous[0] * q[0] + previous[1] * q[1] + previous[2] * q[2] + previous[3] * q[3] < 0) q = q.map(v => -v) as Trs['rotation'];
    previous = q;
    out.translations.set(trs.translation, i * 3);
    out.rotations.set(q, i * 4);
    out.scales.set(trs.scale, i * 3);
  });
  return out;
}
