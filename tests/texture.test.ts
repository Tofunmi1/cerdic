import { expect, test } from 'bun:test';
import { CURATED, describe, nodeInfo, summary } from '../src/engine/texture/catalog';
import { compileTexture } from '../src/engine/texture/compile';
import { loadHelpers, loadLibrary } from '../src/engine/texture/library';
import { parseTextureProgram, TextureError } from '../src/engine/texture/program';
import { boxChartUvs, cornerPositions } from '../src/engine/texture/uv';

const root = new URL('../engine/third_party/material-maker/', import.meta.url).pathname;
const library = loadLibrary(root);
const helpers = loadHelpers(root);

const wood = `
  n = fbm(scale_x=6, scale_y=6, iterations=6, noise=perlin)   // grain
  albedo = colorize(n, gradient=[0:#3b2210, 1:#c98a4b])
  roughness = perlin(scale_x=30, scale_y=30)
  height = n
`;

test('a texture program parses into nodes, links, typed parameters and channels', () => {
  const program = parseTextureProgram(wood, library);
  expect(program.nodes.map((n) => n.id)).toEqual(['n', 'albedo', 'roughness']);
  expect(program.nodes[0].params).toMatchObject({ scale_x: 6, iterations: 6, noise: 1 });
  expect(program.nodes[1].inputs).toEqual({ input: { node: 'n', output: undefined } });
  expect((program.nodes[1].params?.gradient as { points: unknown[] }).points).toHaveLength(2);
  expect(Object.keys(program.channels).sort()).toEqual(['albedo', 'height', 'roughness']);
});

test('every channel of a program compiles to a shader', () => {
  const program = parseTextureProgram(wood, library);
  for (const [channel, output] of Object.entries(program.channels)) {
    const compiled = compileTexture(library, { nodes: program.nodes, output: output as { node: string } }, helpers);
    expect(compiled.source).toContain('void main()');
    expect(compiled.space).toBe('2d');
    expect(channel.length).toBeGreaterThan(0);
  }
});

test('numeric enum choices compile as GLSL floats for Material Maker shader parameters', () => {
  const compiled = compileTexture(library, { nodes: [{ id: 'w', type: 'wavelet_noise' }], output: { node: 'w' } }, helpers);
  expect(compiled.source).toContain('wavelet_noise(uv, vec2((1.0), (1.0)), int((3.0)), (0.5), (0.0), (2.0), (0.0), 1.0)');
  const truchet = compileTexture(library, { nodes: [{ id: 't', type: 'truchet' }], output: { node: 't' } }, helpers);
  expect(truchet.source).toContain('truchet1(uv*(4.0)');
});

test('a mistake names the line and what would fix it', () => {
  const fails = (text: string) => { try { parseTextureProgram(text, library); } catch (e) { return (e as TextureError).message; } return ''; };
  expect(fails('albedo = nothing_like_it(1)')).toContain('unknown texture node');
  expect(fails('n = fbm(sacle_x=4)\nalbedo = n')).toMatch(/line 1: fbm has no parameter or input "sacle_x"; it has .*scale_x/);
  expect(fails('n = fbm(noise=plasma)\nalbedo = n')).toContain('is not one of value, perlin');
  expect(fails('c = colorize(missing)\nalbedo = c')).toContain('must be a node defined above');
  expect(fails('n = fbm()')).toContain('fills no channel');
  expect(fails('n = fbm(); n = fbm()\nalbedo = n')).toContain('defined twice');
  expect(fails('n = fbm(scale_x=#fff)\nalbedo = n')).toContain('is not a number');
});

test('box chart UVs keep closed cube faces in bounded, non-overlapping atlas cells', () => {
  const positions = new Float32Array([-1,-1,-1, 1,-1,-1, 1,1,-1, -1,1,-1, -1,-1,1, 1,-1,1, 1,1,1, -1,1,1]);
  const indices = new Uint32Array([0,2,1, 0,3,2, 4,5,6, 4,6,7, 0,1,5, 0,5,4, 1,2,6, 1,6,5, 2,3,7, 2,7,6, 3,0,4, 3,4,7]);
  const uvs = boxChartUvs(positions, indices);
  expect(uvs).toHaveLength(indices.length * 2);
  for (let i = 0; i < uvs.length; i += 2) {
    expect(uvs[i]).toBeGreaterThanOrEqual(0);
    expect(uvs[i]).toBeLessThanOrEqual(1);
    expect(uvs[i + 1]).toBeGreaterThanOrEqual(0);
    expect(uvs[i + 1]).toBeLessThanOrEqual(1);
  }
  expect(new Set(Array.from({ length: 12 }, (_, face) => `${Math.floor(uvs[face * 6] * 3)}:${Math.floor(uvs[face * 6 + 1] * 2)}`)).size).toBe(6);
  expect(cornerPositions(positions, indices)).toHaveLength(indices.length * 3);
});

test('3D nodes compile against a position map', () => {
  const program = parseTextureProgram('f = tex3d_fbm(scale_x=3, scale_y=3, scale_z=3)\nalbedo = tex3d_colorize(f, g=[0:#000, 1:#fff])', library);
  const compiled = compileTexture(library, { nodes: program.nodes, output: program.channels.albedo! }, helpers);
  expect(compiled.space).toBe('3d');
  expect(compiled.source).toContain('uniform sampler2D uPosition');
});

test('subgraph nodes expand: clouds_noise is a graph of buffers, tilers and blurs', () => {
  const compiled = compileTexture(library, { nodes: [{ id: 'x', type: 'clouds_noise' }], output: { node: 'x' } }, helpers);
  expect(compiled.source.length).toBeGreaterThan(5000);
});

test('every curated node compiles on its own and has a catalog entry', () => {
  const broken: string[] = [];
  for (const name of CURATED) {
    try {
      compileTexture(library, { nodes: [{ id: 'x', type: name }], output: { node: 'x' } }, helpers);
      const info = nodeInfo(name, library[name]);
      if (!info || !summary(info).startsWith(name)) broken.push(`${name}: no catalog entry`);
    } catch (error) {
      broken.push(`${name}: ${(error as Error).message}`);
    }
  }
  expect(broken).toEqual([]);
});

test('describe lists parameters with their ranges, defaults and choices', () => {
  const text = describe(library, 'fbm');
  expect(text).toContain('noise=value|perlin|cellular');
  expect(text).toMatch(/scale_x=1\.\.32\(2\)/);
});
