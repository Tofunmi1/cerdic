// Simulated textures: patterns that are not a function of position but the result of running a small physical model for some steps. The first is the
// Gray-Scott reaction-diffusion model (Pearson 1993), the pattern of animal coats, coral and fingerprints: two chemicals, U (fed in) and V (which eats U and
// decays), spread at different speeds; spots, stripes, mazes and worms grow out of a few seeded specks. The grid wraps at its edges, so the picture tiles.
//
// It runs on the CPU in a fixed order with only + - * /, so the same settings give the same picture on every machine, and a texture program that uses it
// bakes the same everywhere. The cost is bounded (`size * size * steps` stays under `MAX_WORK`), so a program cannot hang the page.

/** The feed and kill rates that grow each named pattern (with Du = 1, Dv = 0.5 on a 9-point Laplacian). */
export const RD_PRESETS: Record<string, { feed: number; kill: number }> = {
  spots: { feed: 0.0367, kill: 0.0649 },
  stripes: { feed: 0.04, kill: 0.06 },
  maze: { feed: 0.029, kill: 0.057 },
  coral: { feed: 0.0545, kill: 0.062 },
  worms: { feed: 0.078, kill: 0.061 },
  holes: { feed: 0.039, kill: 0.058 },
};

export const RD_MIN_SIZE = 16;
export const RD_MAX_SIZE = 512;
export const RD_MAX_STEPS = 30000;
/** Cell updates one simulation may cost (about two seconds of work). */
export const MAX_WORK = 4e8;

export type ReactionDiffusion = {
  /** Cells along each side of the square grid. */
  size: number;
  /** Time steps to run. */
  steps: number;
  /** How fast U is fed in (0.01 to 0.1). */
  feed: number;
  /** How fast V is removed (0.04 to 0.07). */
  kill: number;
  /** Picks where the first specks are. */
  seed: number;
};

/** Checks the settings; returns the reason a simulation is refused, or undefined. */
export function refusal(p: ReactionDiffusion): string | undefined {
  const whole = (v: number) => Number.isInteger(v);
  if (!whole(p.size) || p.size < RD_MIN_SIZE || p.size > RD_MAX_SIZE) return `size must be a whole number from ${RD_MIN_SIZE} to ${RD_MAX_SIZE}, got ${p.size}`;
  if (!whole(p.steps) || p.steps < 1 || p.steps > RD_MAX_STEPS) return `steps must be a whole number from 1 to ${RD_MAX_STEPS}, got ${p.steps}`;
  if (p.size * p.size * p.steps > MAX_WORK) return `size ${p.size} with ${p.steps} steps is ${(p.size * p.size * p.steps / 1e6).toFixed(0)} million cell updates; the most is ${MAX_WORK / 1e6} million (use a smaller size or fewer steps)`;
  if (!(p.feed > 0 && p.feed < 0.2)) return `feed must be above 0 and below 0.2, got ${p.feed}`;
  if (!(p.kill > 0 && p.kill < 0.1)) return `kill must be above 0 and below 0.1, got ${p.kill}`;
  return undefined;
}

/** A seeded stream of numbers in [0, 1) (xorshift32): the same seed always gives the same specks. */
function stream(seed: number): () => number {
  let state = (Math.imul(seed | 0, 0x9e3779b1) ^ 0x85ebca6b) >>> 0 || 1;
  return () => {
    state ^= state << 13; state >>>= 0;
    state ^= state >>> 17;
    state ^= state << 5; state >>>= 0;
    return state / 4294967296;
  };
}

/** Runs the simulation and returns the concentration of V per cell, scaled so the lowest is 0 and the highest 1 (row-major). */
export function grayScott(p: ReactionDiffusion): Float32Array {
  const why = refusal(p);
  if (why) throw new Error(`reaction_diffusion: ${why}`);
  const { size: n, steps, feed, kill } = p;
  const cells = n * n;
  let u = new Float32Array(cells).fill(1);
  let v = new Float32Array(cells);
  let u2 = new Float32Array(cells);
  let v2 = new Float32Array(cells);
  const next = stream(p.seed);
  // Specks: squares of V in a bath of U (strong enough to take hold at every named rate), as many as the grid has room for.
  const specks = Math.max(3, Math.round(n / 10));
  const reach = Math.max(3, Math.round(n / 24));
  for (let s = 0; s < specks; s++) {
    const cx = Math.floor(next() * n), cy = Math.floor(next() * n);
    for (let dy = -reach; dy <= reach; dy++) for (let dx = -reach; dx <= reach; dx++) {
      const k = ((cy + dy + n) % n) * n + ((cx + dx + n) % n);
      u[k] = 0.5; v[k] = 1;
    }
  }
  const Du = 1, Dv = 0.5;
  for (let step = 0; step < steps; step++) {
    for (let y = 0; y < n; y++) {
      const up = ((y + n - 1) % n) * n, here = y * n, down = ((y + 1) % n) * n;
      for (let x = 0; x < n; x++) {
        const l = (x + n - 1) % n, r = (x + 1) % n, k = here + x;
        const lapU = 0.2 * (u[here + l] + u[here + r] + u[up + x] + u[down + x]) + 0.05 * (u[up + l] + u[up + r] + u[down + l] + u[down + r]) - u[k];
        const lapV = 0.2 * (v[here + l] + v[here + r] + v[up + x] + v[down + x]) + 0.05 * (v[up + l] + v[up + r] + v[down + l] + v[down + r]) - v[k];
        const uvv = u[k] * v[k] * v[k];
        u2[k] = u[k] + Du * lapU - uvv + feed * (1 - u[k]);
        v2[k] = v[k] + Dv * lapV + uvv - (kill + feed) * v[k];
      }
    }
    [u, u2] = [u2, u];
    [v, v2] = [v2, v];
  }
  let lo = Infinity, hi = -Infinity;
  for (let k = 0; k < cells; k++) { lo = Math.min(lo, v[k]); hi = Math.max(hi, v[k]); }
  const range = hi - lo > 1e-6 ? hi - lo : 1;
  const out = new Float32Array(cells);
  for (let k = 0; k < cells; k++) out[k] = (v[k] - lo) / range;
  return out;
}

/** The field as RGBA bytes (grey, opaque), row 0 at v = 0. */
export function toPixels(field: Float32Array): Uint8Array {
  const pixels = new Uint8Array(field.length * 4);
  for (let k = 0; k < field.length; k++) {
    const g = Math.round(Math.min(1, Math.max(0, field[k])) * 255);
    pixels[k * 4] = g; pixels[k * 4 + 1] = g; pixels[k * 4 + 2] = g; pixels[k * 4 + 3] = 255;
  }
  return pixels;
}
