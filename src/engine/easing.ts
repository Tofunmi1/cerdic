// Interpolation curves for `anim.track` beyond linear and smoothstep, mirroring ve-modeling's `easing.rs` and
// `bezier.rs` (ports of Blender's BLI_easing and its automatic clamped F-curve handles). The glTF exporter samples
// tracks with these, so an exported clip matches what the viewer plays.
import type { AnimationTrack } from './worker-types';

type Key = AnimationTrack['keys'][number];
type Point = [number, number];

const BACK_OVERSHOOT = 1.70158;
const ELASTIC_AMPLITUDE = 0.8;
const ELASTIC_PERIOD = 0.3;

export const backOut = (fraction: number): number => {
  const t = fraction - 1;
  return t * t * ((BACK_OVERSHOOT + 1) * t + BACK_OVERSHOOT) + 1;
};

export function bounceOut(fraction: number): number {
  let t = fraction;
  if (t < 1 / 2.75) return 7.5625 * t * t;
  if (t < 2 / 2.75) { t -= 1.5 / 2.75; return 7.5625 * t * t + 0.75; }
  if (t < 2.5 / 2.75) { t -= 2.25 / 2.75; return 7.5625 * t * t + 0.9375; }
  t -= 2.625 / 2.75;
  return 7.5625 * t * t + 0.984375;
}

export function elasticOut(fraction: number): number {
  if (fraction === 0) return 0;
  if (fraction === 1) return 1;
  const time = -fraction;
  const s = ELASTIC_PERIOD / 4;
  let f = ELASTIC_AMPLITUDE;
  if (Math.abs(time) < Math.abs(s)) {
    const l = Math.abs(time) / Math.abs(s);
    f = f * l + (1 - l);
  }
  return f * (2 ** (10 * time) * Math.sin(((time - s) * (2 * Math.PI)) / ELASTIC_PERIOD)) + 1;
}

/** The left and right handles of key `i` on one axis (automatic clamped handles, first and last keys level). */
function handles(keys: Key[], axis: number, i: number): [Point, Point] {
  const at = (k: number): Point => [keys[k].time, keys[k].value[axis]];
  const p2 = at(i), last = keys.length - 1;
  const prev = i > 0 ? at(i - 1) : null, next = i < last ? at(i + 1) : null;
  const mirror = (o: Point): Point => [2 * p2[0] - o[0], 2 * p2[1] - o[1]];
  let p1: Point, p3: Point;
  if (prev && next) { p1 = prev; p3 = next; }
  else if (next) { p1 = mirror(next); p3 = next; }
  else if (prev) { p1 = prev; p3 = mirror(prev); }
  else return [[...p2], [...p2]];
  const dA = [p2[0] - p1[0], p2[1] - p1[1]], dB = [p3[0] - p2[0], p3[1] - p2[1]];
  let lenA = dA[0] === 0 ? 1 : dA[0], lenB = dB[0] === 0 ? 1 : dB[0];
  const tvec = [dB[0] / lenB + dA[0] / lenA, dB[1] / lenB + dA[1] / lenA];
  const len = tvec[0] * 2.5614;
  const left: Point = [...p2], right: Point = [...p2];
  if (len === 0) return [left, right];
  lenA = Math.min(lenA, 5 * lenB);
  lenB = Math.min(lenB, 5 * lenA);
  let leftViolates = false, rightViolates = false;
  const both = prev !== null && next !== null;
  const y1 = p1[1] - p2[1], y2 = p3[1] - p2[1];
  const extremum = (y1 <= 0 && y2 <= 0) || (y1 >= 0 && y2 >= 0);
  lenA /= len;
  left[0] = p2[0] - tvec[0] * lenA; left[1] = p2[1] - tvec[1] * lenA;
  if (both) {
    if (extremum) left[1] = p2[1];
    else if (y1 <= 0) { if (p1[1] > left[1]) { left[1] = p1[1]; leftViolates = true; } }
    else if (p1[1] < left[1]) { left[1] = p1[1]; leftViolates = true; }
  }
  lenB /= len;
  right[0] = p2[0] + tvec[0] * lenB; right[1] = p2[1] + tvec[1] * lenB;
  if (both) {
    if (extremum) right[1] = p2[1];
    else if (y1 <= 0) { if (p3[1] < right[1]) { right[1] = p3[1]; rightViolates = true; } }
    else if (p3[1] > right[1]) { right[1] = p3[1]; rightViolates = true; }
  }
  if (leftViolates || rightViolates) {
    const h1x = left[0] - p2[0], h2x = p2[0] - right[0];
    if (leftViolates) right[1] = p2[1] + ((p2[1] - left[1]) / h1x) * h2x;
    else left[1] = p2[1] + ((p2[1] - right[1]) / h2x) * h1x;
  }
  if (i === 0 || i === last) { left[1] = p2[1]; right[1] = p2[1]; }
  return [left, right];
}

/** One axis of a `bezier` track at `time`, in the segment from key `a` to key `a + 1`. */
export function bezierSample(keys: Key[], a: number, time: number, axis: number): number {
  const k0 = keys[a], k1 = keys[a + 1];
  const v1: Point = [k0.time, k0.value[axis]];
  let v2 = handles(keys, axis, a)[1], v3 = handles(keys, axis, a + 1)[0];
  const v4: Point = [k1.time, k1.value[axis]];
  if (time <= v1[0]) return v1[1];
  if (time >= v4[0]) return v4[1];
  const eps = 1.1920929e-7;
  if (Math.abs(v1[1] - v4[1]) < eps && Math.abs(v2[1] - v3[1]) < eps && Math.abs(v3[1] - v4[1]) < eps) return v1[1];
  const span = v4[0] - v1[0];
  const h1 = [v1[0] - v2[0], v1[1] - v2[1]], h2 = [v4[0] - v3[0], v4[1] - v3[1]];
  const l1 = Math.abs(h1[0]), l2 = Math.abs(h2[0]);
  if (l1 + l2 !== 0) {
    if (l1 > span) { const f = span / l1; v2 = [v1[0] - f * h1[0], v1[1] - f * h1[1]]; }
    if (l2 > span) { const f = span / l2; v3 = [v4[0] - f * h2[0], v4[1] - f * h2[1]]; }
  }
  const cubic = (p0: number, p1: number, p2: number, p3: number, t: number) => {
    const u = 1 - t;
    return u * u * u * p0 + 3 * u * u * t * p1 + 3 * u * t * t * p2 + t * t * t * p3;
  };
  let lo = 0, hi = 1;
  for (let n = 0; n < 48; n++) {
    const mid = (lo + hi) / 2;
    if (cubic(v1[0], v2[0], v3[0], v4[0], mid) < time) lo = mid; else hi = mid;
  }
  return cubic(v1[1], v2[1], v3[1], v4[1], (lo + hi) / 2);
}
