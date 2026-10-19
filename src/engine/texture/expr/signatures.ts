// The functions a shader expression may call, each as a rule from its argument sizes to its result size.
// A rule throws a message when the arguments do not fit; the parser reports it as `expr.type`.
import type { Size } from './types';

type Rule = (sizes: Size[]) => Size;

function count(sizes: Size[], wanted: number): void {
  if (sizes.length !== wanted) throw new Error(`expects ${wanted} argument(s), got ${sizes.length}`);
}

/** One argument, same size back (sin, abs, normalize). */
const unary: Rule = (sizes) => {
  count(sizes, 1);
  return sizes[0];
};

/** Two arguments of one size, or a vector with a scalar (min, pow, mod). */
const binary: Rule = (sizes) => {
  count(sizes, 2);
  if (sizes[0] === sizes[1] || sizes[1] === 1) return sizes[0];
  if (sizes[0] === 1) return sizes[1];
  throw new Error('arguments must have the same size, or one must be a float');
};

/** Three arguments (clamp, mix, smoothstep): the common size of all three. */
const ternary: Rule = (sizes) => {
  count(sizes, 3);
  return binary([binary([sizes[0], sizes[1]]), sizes[2]]);
};

/** Two arguments of one size, a float back (dot, distance). */
const measure: Rule = (sizes) => {
  count(sizes, 2);
  if (sizes[0] !== sizes[1]) throw new Error('arguments must have the same size');
  return 1;
};

/** Exact argument sizes and a fixed result size (the snippets). */
const exact = (args: Size[], result: Size): Rule => (sizes) => {
  count(sizes, args.length);
  if (sizes.some((size, i) => size !== args[i])) throw new Error(`expects ${args.map((a) => (a === 1 ? 'float' : `vec${a}`)).join(', ')}`);
  return result;
};

/** A constructor: one float fills every component, otherwise the components must add up to the size. */
const constructor = (target: Size): Rule => (sizes) => {
  if (sizes.length === 1 && sizes[0] === 1) return target;
  const total = sizes.reduce((sum, size) => sum + size, 0);
  if (total !== target) throw new Error(`vec${target} needs ${target} components, got ${total}`);
  return target;
};

export const FUNCTIONS: Record<string, Rule> = {
  sin: unary,
  cos: unary,
  tan: unary,
  asin: unary,
  acos: unary,
  atan: (sizes) => (sizes.length === 2 ? binary(sizes) : unary(sizes)),
  abs: unary,
  sign: unary,
  floor: unary,
  ceil: unary,
  fract: unary,
  sqrt: unary,
  exp: unary,
  log: unary,
  normalize: unary,
  pow: binary,
  min: binary,
  max: binary,
  step: binary,
  mod: binary,
  reflect: binary,
  clamp: ternary,
  mix: ternary,
  smoothstep: ternary,
  length: (sizes) => {
    count(sizes, 1);
    return 1;
  },
  distance: measure,
  dot: measure,
  cross: exact([3, 3], 3),
  hash: exact([2], 1),
  noise: exact([2], 1),
  snoise: exact([2], 1),
  fbm: exact([2], 1),
  hsv: exact([1, 1, 1], 3),
  rot: exact([2, 1], 2),
  vec2: constructor(2),
  vec3: constructor(3),
  vec4: constructor(4),
};

/** The nearest known function name to a misspelt one (edit distance of 2 or less), for the error's suggestion. */
export function nearest(name: string): string | undefined {
  return Object.keys(FUNCTIONS).find((known) => distance(known, name) <= 2);
}

function distance(a: string, b: string): number {
  const row = Array.from({ length: b.length + 1 }, (_, i) => i);
  for (let i = 1; i <= a.length; i++) {
    let previous = row[0];
    row[0] = i;
    for (let j = 1; j <= b.length; j++) {
      const above = row[j];
      row[j] = Math.min(row[j] + 1, row[j - 1] + 1, previous + (a[i - 1] === b[j - 1] ? 0 : 1));
      previous = above;
    }
  }
  return row[b.length];
}
