// Evaluates a shader expression program on the CPU, with the same meaning as the GLSL that `glsl.ts` emits.
//
// It computes in double precision (the GPU uses float32), so results agree to about 1e-6 for smooth functions
// and can differ more for `hash`, `noise` and `fbm`, whose sin() of a large argument is sensitive to rounding.
import { simplex } from './simplex';
import type { Expr, Program, Stmt } from './types';

type Value = number[];

/** The component of `v` for index `i`: a float broadcasts to every component. */
const at = (v: Value, i: number): number => (v.length === 1 ? v[0] : v[i]);

/** Applies a function component by component, broadcasting floats. */
function each(args: Value[], fn: (...x: number[]) => number): Value {
  const size = Math.max(...args.map((arg) => arg.length));
  return Array.from({ length: size }, (_, i) => fn(...args.map((arg) => at(arg, i))));
}

const fract = (x: number): number => x - Math.floor(x);
const clamp = (x: number, low: number, high: number): number => Math.min(high, Math.max(low, x));
const dot = (a: Value, b: Value): number => a.reduce((sum, v, i) => sum + v * b[i], 0);
const length = (a: Value): number => Math.sqrt(dot(a, a));

/** Floored modulo, as GLSL `mod`. */
const mod = (x: number, y: number): number => x - y * Math.floor(x / y);

function hash(x: number, y: number): number {
  return fract(Math.sin(x * 127.1 + y * 311.7) * 43758.5453123);
}

/** Value noise: hash at the four corners of the cell, smoothly interpolated. 0 to 1. */
function noise(x: number, y: number): number {
  const ix = Math.floor(x);
  const iy = Math.floor(y);
  const u = x - ix;
  const v = y - iy;
  const sx = u * u * (3 - 2 * u);
  const sy = v * v * (3 - 2 * v);
  const bottom = hash(ix, iy) * (1 - sx) + hash(ix + 1, iy) * sx;
  const top = hash(ix, iy + 1) * (1 - sx) + hash(ix + 1, iy + 1) * sx;
  return bottom * (1 - sy) + top * sy;
}

/** Five octaves of value noise, scaled to 0..1. */
function fbm(x: number, y: number): number {
  let sum = 0;
  let amplitude = 0.5;
  for (let i = 0; i < 5; i++) {
    sum += noise(x, y) * amplitude;
    x *= 2;
    y *= 2;
    amplitude *= 0.5;
  }
  return sum / 0.96875;
}

/** Cellular noise over the 3 by 3 neighbouring cells: [nearest, second nearest] distance to a jittered point per cell. */
function voronoi(x: number, y: number): Value {
  const ix = Math.floor(x);
  const iy = Math.floor(y);
  let first = 8;
  let second = 8;
  for (let j = -1; j <= 1; j++) {
    for (let i = -1; i <= 1; i++) {
      const cx = ix + i;
      const cy = iy + j;
      const dx = cx + hash(cx, cy) - x;
      const dy = cy + hash(cx + 37.1, cy + 17.3) - y;
      const d = Math.sqrt(dx * dx + dy * dy);
      if (d < first) {
        second = first;
        first = d;
      } else if (d < second) second = d;
    }
  }
  return [first, second];
}

/** The coordinate turned by -angle, then widened by ratio along the first axis. */
function stretch(x: number, y: number, angle: number, ratio: number): Value {
  const c = Math.cos(angle);
  const s = Math.sin(angle);
  return [(c * x + s * y) / ratio, -s * x + c * y];
}

function warp(x: number, y: number, amount: number): Value {
  return [x + amount * clamp(simplex(x + 5.2, y + 1.3), -1, 1), y + amount * clamp(simplex(x + 9.7, y + 8.1), -1, 1)];
}

function hsv(h: number, s: number, v: number): Value {
  return [5, 3, 1].map((offset) => {
    const k = mod(offset + h * 6, 6);
    return v - v * s * clamp(Math.min(k, 4 - k), 0, 1);
  });
}

function smoothstep(low: number, high: number, x: number): number {
  const t = clamp((x - low) / (high - low), 0, 1);
  return t * t * (3 - 2 * t);
}

/** A function of the language applied to evaluated arguments. */
function call(name: string, a: Value[]): Value {
  const [x, y, z] = a;
  switch (name) {
    case 'vec2':
    case 'vec3':
    case 'vec4':
      return a.length === 1 && x.length === 1 ? Array(Number(name[3])).fill(x[0]) : a.flat();
    case 'sin': return each(a, Math.sin);
    case 'cos': return each(a, Math.cos);
    case 'tan': return each(a, Math.tan);
    case 'asin': return each(a, Math.asin);
    case 'acos': return each(a, Math.acos);
    case 'atan': return a.length === 2 ? each(a, Math.atan2) : each(a, Math.atan);
    case 'abs': return each(a, Math.abs);
    case 'sign': return each(a, Math.sign);
    case 'floor': return each(a, Math.floor);
    case 'ceil': return each(a, Math.ceil);
    case 'fract': return each(a, fract);
    case 'sqrt': return each(a, Math.sqrt);
    case 'exp': return each(a, Math.exp);
    case 'log': return each(a, Math.log);
    case 'pow': return each(a, Math.pow);
    case 'min': return each(a, Math.min);
    case 'max': return each(a, Math.max);
    case 'mod': return each(a, mod);
    // GLSL step(edge, x): 0 below the edge, 1 at or above it.
    case 'step': return each(a, (edge, v) => (v < edge ? 0 : 1));
    case 'clamp': return each(a, clamp);
    case 'mix': return each(a, (from, to, t) => from * (1 - t) + to * t);
    case 'smoothstep': return each(a, smoothstep);
    case 'length': return [length(x)];
    case 'distance': return [length(x.map((v, i) => v - y[i]))];
    case 'dot': return [dot(x, y)];
    case 'cross': return [x[1] * y[2] - x[2] * y[1], x[2] * y[0] - x[0] * y[2], x[0] * y[1] - x[1] * y[0]];
    case 'normalize': return x.map((v) => v / length(x));
    // GLSL reflect(incident, normal).
    case 'reflect': return x.map((v, i) => v - 2 * dot(y, x) * y[i]);
    case 'hash': return [hash(x[0], x[1])];
    case 'noise': return [noise(x[0], x[1])];
    case 'snoise': return [clamp(simplex(x[0], x[1]), -1, 1)];
    case 'fbm': return [fbm(x[0], x[1])];
    case 'voronoi': return voronoi(x[0], x[1]);
    case 'stretch': return stretch(x[0], x[1], y[0], z[0]);
    case 'warp': return warp(x[0], x[1], y[0]);
    case 'hsv': return hsv(x[0], y[0], z[0]);
    case 'rot': return [Math.cos(y[0]) * x[0] - Math.sin(y[0]) * x[1], Math.sin(y[0]) * x[0] + Math.cos(y[0]) * x[1]];
    default: throw new Error(`unknown function ${name}`);
  }
}

const OPERATORS: Record<string, (a: number, b: number) => number> = {
  '+': (a, b) => a + b,
  '-': (a, b) => a - b,
  '*': (a, b) => a * b,
  '/': (a, b) => a / b,
  '%': mod,
};

function evaluate(e: Expr, env: Map<string, Value>): Value {
  switch (e.kind) {
    case 'num': return [e.value];
    case 'name': return env.get(e.name) ?? [0];
    case 'unary': return evaluate(e.value, env).map((v) => -v);
    case 'binary': return each([evaluate(e.left, env), evaluate(e.right, env)], OPERATORS[e.op]);
    case 'call': return call(e.name, e.args.map((arg) => evaluate(arg, env)));
    case 'swizzle': {
      const value = evaluate(e.value, env);
      const letters = 'xyzw'.includes(e.field[0]) ? 'xyzw' : 'rgba';
      return [...e.field].map((c) => value[letters.indexOf(c)]);
    }
  }
}

function run(statements: Stmt[], env: Map<string, Value>): void {
  for (const statement of statements) {
    if (statement.kind === 'assign') {
      env.set(statement.name, evaluate(statement.value, env));
      continue;
    }
    for (let i = 0; i < statement.count; i++) {
      env.set(statement.variable, [i]);
      run(statement.body, env);
    }
    env.delete(statement.variable);
  }
}

/** Evaluates a validated program at uv `p` (0..1) and time `t`, giving RGBA. A float `out` is grey; a vec3 has alpha 1. */
export function evalExpr(program: Program, p: [number, number], t = 0): [number, number, number, number] {
  const env = new Map<string, Value>([
    ['p', [...p]],
    ['q', p.map((v) => (v - 0.5) * 2)],
    ['t', [t]],
    ['PI', [Math.PI]],
    ['TAU', [2 * Math.PI]],
  ]);
  run(program.statements, env);
  const out = evaluate(program.output, env);
  if (out.length === 1) return [out[0], out[0], out[0], 1];
  if (out.length === 3) return [out[0], out[1], out[2], 1];
  return [out[0], out[1], out[2], out[3]];
}
