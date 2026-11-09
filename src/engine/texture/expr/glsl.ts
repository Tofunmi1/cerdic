// Emits GLSL ES 3.00 for a shader expression program: fixed helper functions, and a statement body.
//
// The body expects `vec2 p` (uv, 0..1), `vec2 q` (centred, -1..1) and `float t` (time) to be in scope and leaves the
// colour in `vec4 result`. Every user variable is prefixed `u_`, so none can collide with a GLSL name or a helper.
// Loops count with `int i` (depth 0) and `int j` (depth 1) from 0 to a literal bound; the language's loop variable is
// a float copy of it.
import type { Expr, Program, Size, Stmt } from './types';

const PI = '3.14159265358979';
const TAU = '6.28318530717959';
const COUNTERS = ['i', 'j'];

/** The functions of the language that are helper source rather than GLSL built-ins; emitted as `ex_<name>` so they cannot clash with other shader code. */
const SNIPPETS = new Set(['hash', 'noise', 'snoise', 'fbm', 'hsv', 'rot', 'voronoi', 'stretch', 'warp']);

const type = (size: Size): string => (size === 1 ? 'float' : `vec${size}`);

function number(value: number): string {
  return Number.isInteger(value) ? value.toFixed(1) : String(value);
}

function expr(e: Expr): string {
  switch (e.kind) {
    case 'num': return number(e.value);
    case 'name':
      if (e.name === 'PI') return PI;
      if (e.name === 'TAU') return TAU;
      return ['p', 'q', 't'].includes(e.name) ? e.name : `u_${e.name}`;
    case 'unary': return `(-${expr(e.value)})`;
    case 'binary': return e.op === '%' ? `mod(${expr(e.left)}, ${expr(e.right)})` : `(${expr(e.left)} ${e.op} ${expr(e.right)})`;
    case 'swizzle': return `(${expr(e.value)}).${e.field}`;
    case 'call': return `${SNIPPETS.has(e.name) ? `ex_${e.name}` : e.name}(${e.args.map(expr).join(', ')})`;
  }
}

/** Statements at one nesting depth; `declared` holds the variables already declared in the enclosing scopes. */
function statements(list: Stmt[], depth: number, declared: Set<string>): string[] {
  const lines: string[] = [];
  const pad = '  '.repeat(depth + 1);
  for (const statement of list) {
    if (statement.kind === 'assign') {
      const prefix = declared.has(statement.name) ? '' : `${type(statement.value.size)} `;
      declared.add(statement.name);
      lines.push(`${pad}${prefix}u_${statement.name} = ${expr(statement.value)};`);
      continue;
    }
    const counter = COUNTERS[depth];
    const inner = new Set(declared);
    lines.push(`${pad}for (int ${counter} = 0; ${counter} < ${statement.count}; ${counter}++) {`);
    lines.push(`${pad}  float u_${statement.variable} = float(${counter});`);
    lines.push(...statements(statement.body, depth + 1, inner), `${pad}}`);
  }
  return lines;
}

export const HELPERS = `float ex_hash(vec2 p) { return fract(sin(p.x * 127.1 + p.y * 311.7) * 43758.5453123); }
float ex_noise(vec2 p) {
  vec2 i = floor(p);
  vec2 f = fract(p);
  f = f * f * (3.0 - 2.0 * f);
  return mix(mix(ex_hash(i), ex_hash(i + vec2(1.0, 0.0)), f.x), mix(ex_hash(i + vec2(0.0, 1.0)), ex_hash(i + vec2(1.0, 1.0)), f.x), f.y);
}
float ex_fbm(vec2 p) {
  float sum = 0.0;
  float amplitude = 0.5;
  for (int k = 0; k < 5; k++) {
    sum += ex_noise(p) * amplitude;
    p *= 2.0;
    amplitude *= 0.5;
  }
  return sum / 0.96875;
}
vec3 ex_permute(vec3 x) { return mod((x * 34.0 + 1.0) * x, 289.0); }
float ex_snoise(vec2 v) {
  const vec4 C = vec4(0.211324865405187, 0.366025403784439, -0.577350269189626, 0.024390243902439);
  vec2 i = floor(v + dot(v, C.yy));
  vec2 x0 = v - i + dot(i, C.xx);
  vec2 i1 = (x0.x > x0.y) ? vec2(1.0, 0.0) : vec2(0.0, 1.0);
  vec4 x12 = x0.xyxy + C.xxzz;
  x12.xy -= i1;
  i = mod(i, 289.0);
  vec3 p = ex_permute(ex_permute(i.y + vec3(0.0, i1.y, 1.0)) + i.x + vec3(0.0, i1.x, 1.0));
  vec3 m = max(0.5 - vec3(dot(x0, x0), dot(x12.xy, x12.xy), dot(x12.zw, x12.zw)), 0.0);
  m = m * m;
  m = m * m;
  vec3 x = 2.0 * fract(p * C.www) - 1.0;
  vec3 h = abs(x) - 0.5;
  vec3 a0 = x - floor(x + 0.5);
  m *= 1.79284291400159 - 0.85373472095314 * (a0 * a0 + h * h);
  vec3 g;
  g.x = a0.x * x0.x + h.x * x0.y;
  g.yz = a0.yz * x12.xz + h.yz * x12.yw;
  return clamp(130.0 * dot(m, g), -1.0, 1.0);
}
vec3 ex_hsv(float h, float s, float v) {
  vec3 k = mod(vec3(5.0, 3.0, 1.0) + h * 6.0, 6.0);
  return v - v * s * clamp(min(k, 4.0 - k), 0.0, 1.0);
}
vec2 ex_rot(vec2 p, float a) { return vec2(cos(a) * p.x - sin(a) * p.y, sin(a) * p.x + cos(a) * p.y); }
vec2 ex_voronoi(vec2 p) {
  vec2 b = floor(p);
  float f1 = 8.0;
  float f2 = 8.0;
  for (int vj = -1; vj <= 1; vj++) {
    for (int vi = -1; vi <= 1; vi++) {
      vec2 c = b + vec2(float(vi), float(vj));
      vec2 d = c + vec2(ex_hash(c), ex_hash(c + vec2(37.1, 17.3))) - p;
      float l = length(d);
      if (l < f1) { f2 = f1; f1 = l; } else if (l < f2) { f2 = l; }
    }
  }
  return vec2(f1, f2);
}
vec2 ex_stretch(vec2 p, float a, float r) { return vec2((cos(a) * p.x + sin(a) * p.y) / r, -sin(a) * p.x + cos(a) * p.y); }
vec2 ex_warp(vec2 p, float a) { return p + a * vec2(ex_snoise(p + vec2(5.2, 1.3)), ex_snoise(p + vec2(9.7, 8.1))); }`;

/** The helper functions (put them before `main`) and the body (put it inside `main`, after defining `p`, `q` and `t`). */
export function emitGlsl(program: Program): { functions: string; body: string } {
  const lines = statements(program.statements, 0, new Set());
  const colour = program.output.size === 1
    ? `vec4(vec3(${expr(program.output)}), 1.0)`
    : program.output.size === 3
      ? `vec4(${expr(program.output)}, 1.0)`
      : expr(program.output);
  lines.push(`  vec4 result = ${colour};`);
  return { functions: HELPERS, body: lines.join('\n') };
}
