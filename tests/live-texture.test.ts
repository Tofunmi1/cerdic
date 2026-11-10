import { expect, test } from 'bun:test';
import { compileTexture } from '../src/engine/texture/compile';
import { loadHelpers, loadLibrary } from '../src/engine/texture/library';
import { animatedAlbedo } from '../src/engine/texture/live';
import { parseTextureProgram } from '../src/engine/texture/program';

const root = new URL('../engine/lib/material-maker/', import.meta.url).pathname;
const library = loadLibrary(root);
const helpers = loadHelpers(root);

const compile = (source: string) => {
  const program = parseTextureProgram(source, library);
  return compileTexture(library, { nodes: program.nodes, output: program.channels.albedo as { node: string } }, helpers);
};

test('a program that reads the clock is animated and declares the time uniform once', () => {
  const compiled = compile('n = expr(code="out = vec3(0.5 + 0.5 * sin(p.x * 12.0 + t * 2.0))")\nalbedo = n');
  expect(compiled.animated).toBe(true);
  expect(compiled.source.match(/uniform float uTime;/g)?.length).toBe(1);
  expect(compiled.source).toContain('float t = uTime;');
});

test('a program that does not read the clock is still, with no time uniform', () => {
  const compiled = compile('n = fbm(scale_x=4, scale_y=4)\nalbedo = n');
  expect(compiled.animated).toBe(false);
  expect(compiled.source).not.toContain('uTime');
  // An expr node that never mentions t is still too (the clock is read only by expressions that use it).
  const still = compile('n = expr(code="out = vec3(p.x, p.y, 0.0)")\nalbedo = n');
  expect(still.animated).toBe(false);
  expect(still.source).not.toContain('uTime');
});

test('animatedAlbedo gives the compiled shader of an animated program that needs nothing from the mesh, and nothing for the rest', () => {
  expect(animatedAlbedo(library, helpers, 'n = expr(code="out = vec3(sin(t))")\nalbedo = n')?.animated).toBe(true);
  expect(animatedAlbedo(library, helpers, 'n = fbm(scale_x=4, scale_y=4)\nalbedo = n')).toBeUndefined();
  expect(animatedAlbedo(library, helpers, 'this is not a program')).toBeUndefined();
  expect(animatedAlbedo(library, helpers, '')).toBeUndefined();
  // A program with no albedo channel has nothing to draw live.
  expect(animatedAlbedo(library, helpers, 'n = expr(code="out = vec3(sin(t))")\nroughness = n')).toBeUndefined();
});
