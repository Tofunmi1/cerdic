import { describe, expect, test } from "bun:test";
import { emitGlsl, evalExpr, parseExpr } from "../src/engine/texture/expr";
import type { ErrorCode } from "../src/engine/texture/expr/types";
const parsed = (s: string) => {
  const r = parseExpr(s);
  if ("error" in r) throw new Error(`${r.error.code}: ${r.error.message}`);
  return r.program;
};
const value = (s: string, p: [number, number] = [0.25, 0.5]) =>
  evalExpr(parsed(s), p);
const close = (a: number[], b: number[], eps = 1e-6) =>
  a.forEach((v, i) => expect(Math.abs(v - b[i])).toBeLessThan(eps));
describe("bounded shader expressions", () => {
  test("1. scalar and vector output", () => {
    expect(value("out = p.x")).toEqual([0.25, 0.25, 0.25, 1]);
    expect(value("out = vec3(p, 0)")).toEqual([0.25, 0.5, 0, 1]);
    expect(value("out = vec4(1,0,0,0.5)")).toEqual([1, 0, 0, 0.5]);
  });
  test("2. arithmetic and swizzles", () => {
    expect(value("a = vec2(3, 4)\nout = length(a) / 10")[0]).toBeCloseTo(
      0.5,
      6,
    );
    close(value("out = vec3(1,2,3).zyx / 3").slice(0, 3), [1, 2 / 3, 1 / 3]);
  });
  test("3. bounded loops", () => {
    expect(
      value("s = 0\nfor i in 0..10 { s = s + i }\nout = s / 100")[0],
    ).toBeCloseTo(0.45, 6);
    expect(
      value("s = 0\nfor i in 0..4 { for j in 0..4 { s = s + 1 } }\nout = s")[0],
    ).toBe(16);
  });
  test("4. deterministic snippets and bounds", () => {
    const h = parsed("out = hash(p)");
    let total = 0;
    for (let i = 0; i < 1000; i++) {
      const p: [number, number] = [(i * 0.618) % 1, (i * 0.414) % 1],
        a = evalExpr(h, p)[0];
      expect(a).toBeGreaterThanOrEqual(0);
      expect(a).toBeLessThan(1);
      total += a;
    }
    expect(total / 1000).toBeCloseTo(0.5, 1);
    for (const fn of ["noise", "fbm"]) {
      const pr = parsed(`out = ${fn}(p)`);
      for (let i = 0; i < 200; i++) {
        const p: [number, number] = [(i * 0.037) % 1, (i * 0.071) % 1],
          a = evalExpr(pr, p)[0],
          b = evalExpr(pr, [p[0] + 1e-4, p[1]])[0];
        expect(a).toBeGreaterThanOrEqual(0);
        expect(a).toBeLessThanOrEqual(1);
        expect(Math.abs(a - b)).toBeLessThan(0.01);
      }
    }
    const sn = parsed("out = snoise(p)");
    for (let i = 0; i < 100; i++) {
      const a = evalExpr(sn, [i * 0.03, i * 0.07])[0];
      expect(a).toBeGreaterThanOrEqual(-1);
      expect(a).toBeLessThanOrEqual(1);
    }
    close(value("out = hsv(0,1,1)").slice(0, 3), [1, 0, 0], 1e-5);
    close(value("out = hsv(1/3,1,1)").slice(0, 3), [0, 1, 0], 1e-5);
  });
  test("5. rejects unsupported programs with useful typed errors", () => {
    const cases: [string, ErrorCode][] = [
      ["a = 1", "expr.no_out"],
      ["out = foo(1)", "expr.unknown_name"],
      ["a = vec2(1)\nout = a + vec3(1)", "expr.type"],
      ["for i in 0..65 { }\nout = 1", "expr.loop"],
      [
        "for i in 0..2 { for j in 0..2 { for k in 0..2 { } } }\nout = 1",
        "expr.loop",
      ],
      ["x".repeat(2001), "expr.limit"],
      ["out = while", "expr.syntax"],
      ["out = if", "expr.syntax"],
      ["out = texture(p)", "expr.unknown_name"],
      ["out = 1 < 2", "expr.syntax"],
    ];
    for (const [src, code] of cases) {
      const r = parseExpr(src);
      expect("error" in r).toBe(true);
      if ("error" in r) {
        expect(r.error.code).toBe(code);
        expect(r.error.suggestion.length).toBeGreaterThan(0);
      }
    }
    const sin = parseExpr("out = sinn(1)");
    if ("error" in sin) expect(sin.error.suggestion).toContain("sin");
  });
  test("6. emits bounded GLSL with prefixed identifiers", () => {
    const p = parsed(
      "float1 = hash(p)\nsampler = noise(p)\ninput = fbm(p)\nout2 = hsv(t, 1, 1)\nfor i in 0..3 { float1 = float1 + snoise(p) }\nout = vec4(rot(p, t), float1, 1)",
    );
    const g = emitGlsl(p);
    expect(g.body).not.toMatch(/\bwhile\b|\btexture\b|#/);
    expect(g.body).toContain("u_float1");
    expect(g.body).toContain("u_sampler");
    expect(g.body).toContain("u_input");
    expect(g.body).toContain("u_out2");
    expect(g.body).toMatch(/i < 3/);
  });
  test("7. seeded token fuzz never throws", () => {
    let seed = 18471;
    const rand = () => {
      seed = (seed * 1664525 + 1013904223) >>> 0;
      return seed;
    };
    const tokens = [
      "out",
      "=",
      "p",
      "q",
      "t",
      "1",
      "0.5",
      "+",
      "-",
      "*",
      "/",
      "%",
      "(",
      ")",
      "vec2",
      "sin",
      "hash",
      "foo",
      ";",
      "{",
      "}",
      "for",
      "i",
      "in",
      "0",
      "..",
      "3",
      ".x",
      ",",
    ];
    for (let i = 0; i < 300; i++) {
      const s = Array.from(
        { length: rand() % 20 },
        () => tokens[rand() % tokens.length],
      ).join(" ");
      try {
        const r = parseExpr(s);
        if ("program" in r) {
          emitGlsl(r.program);
          evalExpr(r.program, [0.2, 0.7]);
        } else expect(r.error.code).toMatch(/^expr\./);
      } catch (e) {
        throw new Error(`Fuzz case threw: ${s}: ${String(e)}`);
      }
    }
  });
  test("8. CPU formulas agree with independent reference samples", () => {
    const programs = [
      "out = p.x + t",
      "out = length(p)",
      "s = 0\nfor i in 0..4 { s = s + i }\nout = s / 10",
      "out = hash(p)",
      "out = noise(p)",
    ];
    const refHash = (x: number, y: number) => {
      const q = Math.sin(x * 127.1 + y * 311.7) * 43758.5453123;
      return q - Math.floor(q);
    };
    const refNoise = (x: number, y: number) => {
      const ix = Math.floor(x),
        iy = Math.floor(y),
        u = x - ix,
        v = y - iy,
        sx = u * u * (3 - 2 * u),
        sy = v * v * (3 - 2 * v),
        h = refHash;
      return (
        (h(ix, iy) * (1 - sx) + h(ix + 1, iy) * sx) * (1 - sy) +
        (h(ix, iy + 1) * (1 - sx) + h(ix + 1, iy + 1) * sx) * sy
      );
    };
    for (let k = 0; k < programs.length; k++) {
      const pr = parsed(programs[k]);
      for (let i = 0; i < 50; i++) {
        const p: [number, number] = [i * 0.021, i * 0.017],
          t = 0.4;
        const actual = evalExpr(pr, p, t)[0];
        const expected =
          k === 0
            ? p[0] + t
            : k === 1
            ? Math.hypot(...p)
            : k === 2
            ? 0.6
            : k === 3
            ? refHash(...p)
            : refNoise(...p);
        expect(Math.abs(actual - expected)).toBeLessThan(1e-5);
      }
    }
  });
});
