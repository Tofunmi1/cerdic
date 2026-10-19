// Meaning of each function of the shader expression language, against values worked out by hand (GLSL semantics).
import { describe, expect, test } from 'bun:test';
import { emitGlsl, evalExpr, parseExpr } from '../src/engine/texture/expr';

function run(source: string, p: [number, number] = [0.25, 0.5], t = 0): number[] {
  const parsed = parseExpr(source);
  if ('error' in parsed) throw new Error(`${parsed.error.code}: ${parsed.error.message}`);
  return evalExpr(parsed.program, p, t);
}

function first(source: string, p?: [number, number], t?: number): number {
  return run(source, p, t)[0];
}

describe('function semantics', () => {
  test('step(edge, x) is 0 below the edge and 1 at or above it', () => {
    expect(first('out = step(0.5, 0.2)')).toBe(0);
    expect(first('out = step(0.5, 0.5)')).toBe(1);
    expect(first('out = step(0.5, 0.9)')).toBe(1);
  });

  test('smoothstep(low, high, x) eases between the edges', () => {
    expect(first('out = smoothstep(0, 1, 0)')).toBe(0);
    expect(first('out = smoothstep(0, 1, 1)')).toBe(1);
    expect(first('out = smoothstep(0, 1, 0.5)')).toBeCloseTo(0.5, 12);
    expect(first('out = smoothstep(2, 4, 2.5)')).toBeCloseTo(0.15625, 12);
    expect(first('out = smoothstep(2, 4, 9)')).toBe(1);
  });

  test('mix(a, b, t) and clamp(x, low, high)', () => {
    expect(first('out = mix(10, 20, 0.25)')).toBeCloseTo(12.5, 12);
    expect(run('out = mix(vec3(0), vec3(1, 2, 3), 0.5)').slice(0, 3)).toEqual([0.5, 1, 1.5]);
    expect(first('out = clamp(5, 0, 1)')).toBe(1);
    expect(first('out = clamp(-5, 0, 1)')).toBe(0);
  });

  test('rot turns a vector counter-clockwise by the angle', () => {
    const [x, y] = run('out = vec3(rot(vec2(1, 0), PI / 2), 0)');
    expect(x).toBeCloseTo(0, 12);
    expect(y).toBeCloseTo(1, 12);
  });

  test('mod and % are floored, so negative values wrap upwards', () => {
    expect(first('out = mod(5.5, 2)')).toBeCloseTo(1.5, 12);
    expect(first('out = -1 % 3')).toBe(2);
    expect(first('out = mod(-0.25, 1)')).toBeCloseTo(0.75, 12);
  });

  test('a float broadcasts against a vector in either order', () => {
    expect(run('out = 2 * vec3(1, 2, 3)').slice(0, 3)).toEqual([2, 4, 6]);
    expect(run('out = max(vec3(1, 5, 3), 2)').slice(0, 3)).toEqual([2, 5, 3]);
    expect(run('out = max(2, vec3(1, 5, 3))').slice(0, 3)).toEqual([2, 5, 3]);
    expect(run('out = pow(2, vec3(1, 2, 3))').slice(0, 3)).toEqual([2, 4, 8]);
  });

  test('geometry functions: dot, cross, normalize, distance, reflect', () => {
    expect(first('out = dot(vec3(1, 2, 3), vec3(4, 5, 6))')).toBe(32);
    expect(run('out = cross(vec3(1, 0, 0), vec3(0, 1, 0))').slice(0, 3)).toEqual([0, 0, 1]);
    expect(first('out = length(normalize(vec3(3, 4, 12)))')).toBeCloseTo(1, 12);
    expect(first('out = distance(vec2(1, 1), vec2(4, 5))')).toBe(5);
    expect(run('out = vec3(reflect(vec2(1, -1), vec2(0, 1)), 0)').slice(0, 2)).toEqual([1, 1]);
  });

  test('q is p centred on 0 and t is the time given', () => {
    expect(run('out = vec3(q, 0)', [0.25, 0.5]).slice(0, 2)).toEqual([-0.5, 0]);
    expect(first('out = t', [0, 0], 3)).toBe(3);
    expect(first('out = PI / 2')).toBeCloseTo(Math.PI / 2, 12);
  });

  test('simplex noise is 0 at the origin, varies, and stays in range', () => {
    expect(first('out = snoise(vec2(0, 0))')).toBeCloseTo(0, 12);
    const values = Array.from({ length: 500 }, (_, i) => first('out = snoise(p)', [i * 0.137, i * 0.0731]));
    expect(Math.min(...values)).toBeGreaterThanOrEqual(-1);
    expect(Math.max(...values)).toBeLessThanOrEqual(1);
    expect(Math.max(...values)).toBeGreaterThan(0.4);
    expect(Math.min(...values)).toBeLessThan(-0.4);
  });
});

describe('scoping and types', () => {
  test('a variable first assigned inside a loop is not visible after it', () => {
    const parsed = parseExpr('for i in 0..3 { a = i }\nout = a');
    expect('error' in parsed && parsed.error.code).toBe('expr.unknown_name');
  });

  test('a variable assigned before a loop keeps what the loop did to it', () => {
    expect(first('a = 1\nfor i in 0..3 { a = a * 2 }\nout = a / 8')).toBe(1);
  });

  test('a variable cannot change size, and a loop variable cannot reuse a name', () => {
    const resized = parseExpr('a = 1\na = vec2(1)\nout = a.x');
    expect('error' in resized && resized.error.code).toBe('expr.type');
    const reused = parseExpr('i = 1\nfor i in 0..3 { }\nout = i');
    expect('error' in reused && reused.error.code).toBe('expr.loop');
  });

  test('built-ins cannot be assigned, out must be last and not a vec2', () => {
    expect('error' in parseExpr('p = 1\nout = p')).toBe(true);
    expect('error' in parseExpr('out = 1\na = 2')).toBe(true);
    const vec2 = parseExpr('out = p');
    expect('error' in vec2 && vec2.error.code).toBe('expr.type');
  });

  test('errors carry the line and column of the problem', () => {
    const parsed = parseExpr('a = 1\nb = nosuch(a)\nout = b');
    expect('error' in parsed && [parsed.error.line, parsed.error.column]).toEqual([2, 5]);
  });
});

describe('GLSL output', () => {
  const emitted = (source: string) => {
    const parsed = parseExpr(source);
    if ('error' in parsed) throw new Error(parsed.error.message);
    return emitGlsl(parsed.program);
  };

  test('declares each variable once, with its type, and a vec4 result', () => {
    const { body } = emitted('a = p * 2\na = a + 1\nout = vec3(a, 0)');
    expect(body.match(/vec2 u_a =/g)?.length).toBe(1);
    expect(body).toContain('  u_a = (u_a + 1.0);');
    expect(body).toContain('vec4 result = vec4(vec3(u_a, 0.0), 1.0);');
  });

  test('nested loops count with i and j, from zero to the literal bound', () => {
    const { body } = emitted('s = 0\nfor a in 0..4 { for b in 0..3 { s = s + a * b } }\nout = s');
    expect(body).toContain('for (int i = 0; i < 4; i++)');
    expect(body).toContain('for (int j = 0; j < 3; j++)');
    expect(body).toContain('float u_a = float(i);');
    expect(body).toContain('float u_b = float(j);');
  });

  test('% becomes mod and the helper source defines no GLSL built-in', () => {
    const { functions, body } = emitted('out = p.x % 0.3');
    expect(body).toContain('mod((p).x, 0.3)');
    for (const builtin of ['mod', 'clamp', 'mix', 'step', 'fract', 'floor', 'min', 'max']) {
      expect(functions).not.toMatch(new RegExp(`^(float|vec[234]) ${builtin}\\(`, 'm'));
    }
  });
});
