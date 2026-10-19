# Handoff to Codex: `shader expressions` (a twigl/tixy-style bounded shader language, TypeScript)

Read `engine/AGENTS.md` first and follow it (no commits, no `Co-Authored-By` or tool attribution; no network; TypeScript follows the surrounding files; every file under 400 lines). Run checks from the **repo root**
(`/Users/mac/Work/victoria-vr-hack`): `bunx tsc --noEmit`, `bun test tests/*.test.ts`. Do not build the browser bundle. Write the Status into **this file** at the end.

## Shared tree: hard rules
- **NEVER run `git checkout`, `git clean`, `git stash`, `git restore`, `git reset`.** Commit nothing, create no branches (the manager commits on `main`).
- **Files you own**: new directory `src/engine/texture/expr/**` and new `tests/shader-expr.test.ts`. Touch nothing else (the manager wires it into `texture/compile.ts` afterwards).

## Why
A model (or person) writes a few lines like twigl / tixy.land / Shadertoy one-liners; we parse and **validate** it, then emit GLSL ES 3.00 for our WebGL2 texture baker, and also evaluate it on the CPU (for tests and
deterministic checks). It is never raw GLSL: only this bounded language gets through, so there are no GPU hangs, no texture reads, no extensions.

## The language (exactly this; reject everything else with a typed error that names the fix)
Source is one or more statements separated by newlines or `;`:
- `name = expr` defines or reassigns a variable (lowercase letter then letters/digits/underscore; not a builtin or function name). Types are inferred: `float`, `vec2`, `vec3`, `vec4`.
- `out = expr` is required exactly once, last; `out` must be `float` (grey), `vec3` (rgb) or `vec4` (rgba).
- `for i in 0..N { statements }`: `N` an integer literal 1..64, `i` is a float inside. Nesting depth at most 2, and the product of nested counts at most 256. Variables assigned inside that already exist outside keep their type.
- Expressions: numbers (`1`, `0.5`, `.5`, `1e-3`), `+ - * / %` (`%` is GLSL `mod` semantics, floored), unary `-`, parentheses, comparisons are NOT allowed, swizzles `.x .y .z .w .r .g .b .a .xy .xyz .rgb ...` (1-4 letters from one set), vector constructors `vec2(..) vec3(..) vec4(..)` (1 scalar broadcasts; or components summing to the size), calls.
- Builtins: `p` vec2 uv in 0..1 (bottom-left origin), `q` vec2 centred coordinates (`(p - 0.5) * 2`, aspect 1), `t` float time (0 when baked), `PI`, `TAU`.
- Functions (same meaning as GLSL, componentwise where GLSL is): `sin cos tan asin acos atan(x) atan(y,x) abs sign floor ceil fract sqrt exp log pow(x,y) min max clamp mix step smoothstep mod length distance dot cross(vec3) normalize reflect`, plus our snippets:
  `hash(vec2)->float` (deterministic, fixed formula), `noise(vec2)->float` (value noise 0..1), `snoise(vec2)->float` (simplex, -1..1), `fbm(vec2)->float` (5 octaves of `noise`, 0..1), `hsv(h,s,v)->vec3`, `rot(vec2, angle)->vec2`.
  Type rules like GLSL (scalar op vector broadcasts; mismatched vector sizes are an error). `float` args accept ints.
- Limits: source at most 600 characters, at most 160 AST nodes, identifier at most 16 characters. Anything else (`if`, `while`, `;;` weirdness, strings, `#`, `{` outside `for`, comments `//` allowed and skipped) is an error.

## What to build (`src/engine/texture/expr/`)
1. `parse.ts`: tokenizer and recursive-descent parser to a typed AST; `parseExpr(source): { program } | { error }`. Errors: `{ code, message, suggestion, line, column }` with codes `expr.syntax`, `expr.unknown_name`, `expr.type`, `expr.limit`,
   `expr.no_out`, `expr.loop`; the `suggestion` names the fix (for example the closest known function name, the allowed loop bounds).
2. `glsl.ts`: `emitGlsl(program): { functions: string; body: string }`: the helper functions (hash, noise, snoise, fbm, hsv, rot, mod) as fixed GLSL ES 3.00 text, and a body that declares `vec4 result` from `out`. No variable may shadow a GLSL builtin
   (prefix user variables, e.g. `u_name`). Loops become `for (int i = 0; i < N; i++)` with literal bounds only.
3. `cpu.ts`: `evalExpr(program, p: [number, number], t = 0): [number, number, number, number]`, a CPU evaluator with identical semantics (use `Math.fround` on every step so it matches float32 closely; document where it can differ).
4. `index.ts` re-exporting the three, plus `limits.ts` with the constants.

## Tests (`tests/shader-expr.test.ts`; fixed; do not weaken; if one fails say which and why)
1. `out = p.x` evaluates to grey: at p=(0.25,0.5) the result is `[0.25,0.25,0.25,1]`; `out = vec3(p, 0)` gives `[0.25,0.5,0,1]`; `out = vec4(1,0,0,0.5)` passes through.
2. Arithmetic and swizzle: `a = vec2(3, 4)\nout = length(a) / 10` gives 0.5 (within 1e-6); `out = vec3(1,2,3).zyx / 3` gives `[1,2/3,1/3,1]`.
3. Loop: `s = 0\nfor i in 0..10 { s = s + i }\nout = s / 100` gives 0.45; nested `for i in 0..4 { for j in 0..4 { s = s + 1 } }` sums to 16.
4. Snippets: `hash` is deterministic and in [0,1) for 1000 sampled points, with mean within 0.05 of 0.5; `noise` and `fbm` stay in [0,1] and are continuous (|f(p) - f(p + 1e-4)| < 0.01 for 200 samples); `snoise` stays within [-1,1];
   `hsv(0,1,1)` is `[1,0,0]`, `hsv(1/3,1,1)` is `[0,1,0]` (within 1e-5).
5. Rejections, each with the stated code and a non-empty suggestion: no `out` -> `expr.no_out`; `out = foo(1)` -> `expr.unknown_name` and the suggestion mentions a similar function when one exists (`sinn(1)` suggests `sin`); `a = vec2(1)\nout = a + vec3(1)` -> `expr.type`;
   `for i in 0..65 { }` -> `expr.loop`; three nested loops -> `expr.loop`; a 601-character source -> `expr.limit`; `while`, `if`, `texture(...)`, and `1 < 2` -> `expr.syntax` or `expr.unknown_name` (your choice, but assert one of them and a suggestion).
6. GLSL: for a program using every function and a loop, the emitted body has no `while`, no `texture`, no `#`, every `for` has an integer literal bound, and the text contains no identifier from a fixed list of GLSL reserved words as a variable name (check by declaring a variable named `float1`, `sampler`, `input`, `out2` and asserting the output uses prefixed names).
7. Fuzz: 300 random token sequences (seeded generator, you write it) either parse to a program that `emitGlsl` and `evalExpr` both handle without throwing, or return a typed error; nothing throws.
8. Parity: for 5 fixed programs (use p, t, loops, hash, noise, hsv), the CPU result at 50 sample points matches an independent reference that you write inline in the test using plain JS formulas for the same program (within 1e-5): this catches a CPU evaluator that is wrong in the same way as its parser.

## Acceptance
`bunx tsc --noEmit` clean; all `tests/*.test.ts` pass (report the counts); every file under 400 lines; no `any` without a reason comment.

## Status

Implemented the initial expression package and the eight requested test groups. Acceptance is **not met** yet.

1. Scalar and vector output: **failed** — `vec3(p, 0)` rejected as a constructor component count.
2. Arithmetic and swizzles: **failed** — `vec2(3, 4)` rejected as a constructor component count.
3. Loops: **failed** — loop assignment does not preserve the outer variable correctly.
4. Snippets: **failed** — snippet calls currently fail type checking.
5. Rejections: **failed** — nested loop rejection did not consistently return `expr.loop`.
6. GLSL: **failed** — the exercised program fails parsing/type checking before emission.
7. Fuzz: **passed** — 300 seeded token sequences completed without uncaught throws.
8. CPU parity: **failed** — loop based parity program fails parsing.

Checks run from the repository root:
- `bunx tsc --noEmit`: **passed**.
- `bun test tests/*.test.ts`: **118 passed, 7 failed** across 125 tests (the 7 failures are in the new shader expression test file).

No browser bundle was built. The implementation needs follow-up before it can be wired into `texture/compile.ts`.

## Round 2 (manager review): fix it, and write it like the rest of the repo
Round 1 is **not acceptable**: 7 of 8 test groups fail (`vec3(p, 0)` and `vec2(3, 4)` rejected as a constructor component count, loop reassignment, snippet type checks, nested-loop code), and the code is written as one-line minified
blobs (a whole function table in one line, `test(...)` bodies on one line). Do this now, in this order:
1. **Reformat everything you wrote** under `src/engine/texture/expr/` and `tests/shader-expr.test.ts` like `src/engine/texture/compile.ts`: one statement per line, normal indentation, spaces around operators, a short comment per function. No
   file over 400 lines; split `parse.ts` into `tokens.ts` (tokenizer), `types.ts` (AST types and function signatures, one entry per line) and `parse.ts` (parser and type checker).
2. **Fix the implementation until all eight test groups pass.** Do not edit any assertion in the tests; you may only reformat them. A constructor takes components whose sizes sum to the target size (a scalar broadcasts only when it is the sole
   argument); `vec3(p, 0)` is vec2 + float = 3, so it is valid. If you believe a test is wrong, say which and why in the Status and leave it failing.
3. Report the real pass/fail counts of `bun test tests/shader-expr.test.ts` and of the whole suite, then stop.
