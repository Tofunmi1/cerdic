import { expect, test } from 'bun:test';

// The generated declarations (src/engine/operators.d.ts) compile, accept the calls in tests/typed-api/usage.ts and reject the ones marked @ts-expect-error
// (the compiler reports an error for an expected error that does not happen, so a type that stops catching a mistake fails here).
test('the usage file compiles against the generated operator declarations, mistakes included', async () => {
  const root = new URL('../', import.meta.url).pathname;
  const run = Bun.spawn(['bunx', 'tsc', '--noEmit', '--strict', '--target', 'es2022', '--moduleResolution', 'bundler', '--module', 'esnext', 'tests/typed-api/usage.ts'], { cwd: root, stdout: 'pipe', stderr: 'pipe' });
  const [out, code] = [await new Response(run.stdout).text(), await run.exited];
  expect(out.trim()).toBe('');
  expect(code).toBe(0);
}, 120000);

test('the declarations name the operators a model is told about', async () => {
  const text = await Bun.file(new URL('../src/engine/operators.d.ts', import.meta.url)).text();
  for (const call of ['box(', 'select(', 'sim(', 'fracture(', 'biomes(', 'retarget(', 'simplify<']) expect(text).toContain(call);
  expect(text.match(/export interface \w+Params \{/g)!.length).toBeGreaterThan(200);
});
