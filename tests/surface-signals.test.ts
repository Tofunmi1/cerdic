import { expect, test } from 'bun:test';
import { compileTexture } from '../src/engine/texture/compile';
import { loadHelpers, loadLibrary } from '../src/engine/texture/library';
import { parseTextureProgram } from '../src/engine/texture/program';
import { cornerOcclusion } from '../src/engine/texture/signals';

// The curvature itself is measured per texel on the GPU and checked in engine/web/tests/g10-surface-signals.mjs.
const root = new URL('../engine/third_party/material-maker/', import.meta.url).pathname;
const library = loadLibrary(root);
const helpers = loadHelpers(root);

test('occlusion is passed through per corner, and is open when there is none', () => {
  const indices = new Uint32Array([0, 1, 2, 2, 1, 3]);
  expect(Array.from(cornerOcclusion(indices)).filter((_, i) => i % 3 === 0)).toEqual([1, 1, 1, 1, 1, 1]);
  const occlusion = new Float32Array([0.25, 0.5, 2, -1]);
  const corners = Array.from(cornerOcclusion(indices, occlusion)).filter((_, i) => i % 3 === 0);
  expect(corners).toEqual([0.25, 0.5, 1, 1, 0.5, 0]);
});

test('surface nodes compile to a shader that declares the maps it reads', () => {
  const program = parseTextureProgram(`
    n = fbm(scale_x=6, scale_y=6)
    edge = surface_convex(gain=2)
    dirt = surface_cavity()
    albedo = colorize(edge, gradient=[0:#3b2210, 1:#e8d9b5])
    roughness = surface_ao()
    height = n
  `, library);
  const albedo = compileTexture(library, { nodes: program.nodes, output: program.channels.albedo as { node: string } }, helpers);
  expect(albedo.needs).toEqual({ position: false, surface: true });
  expect(albedo.source).toContain('uniform sampler2D uSurface;');
  expect(albedo.source).not.toContain('uniform sampler2D uPosition;');
  const plain = compileTexture(library, { nodes: program.nodes, output: program.channels.height as { node: string } }, helpers);
  expect(plain.needs).toEqual({ position: false, surface: false });
  const height = parseTextureProgram('h = surface_height(low=0, high=1.7)\nalbedo = h', library);
  const compiled = compileTexture(library, { nodes: height.nodes, output: height.channels.albedo as { node: string } }, helpers);
  expect(compiled.needs).toEqual({ position: true, surface: false });
});

test('every surface node is described in the catalog so a model can find it', async () => {
  const { nodeInfo } = await import('../src/engine/texture/catalog');
  for (const name of ['surface_convex', 'surface_cavity', 'surface_curvature', 'surface_ao', 'surface_up', 'surface_height']) {
    const info = nodeInfo(name, library[name]);
    expect(info?.description.length).toBeGreaterThan(20);
    expect(info?.params.length).toBeGreaterThan(0);
  }
});
