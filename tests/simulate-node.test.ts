import { expect, test } from 'bun:test';
import { compileTexture } from '../src/engine/texture/compile';
import { loadHelpers, loadLibrary } from '../src/engine/texture/library';
import { rawPicture } from '../src/engine/texture/outfit-images';
import { parseTextureProgram } from '../src/engine/texture/program';

const root = new URL('../engine/lib/material-maker/', import.meta.url).pathname;
const library = loadLibrary(root);
const helpers = loadHelpers(root);

const compile = (source: string) => {
  const program = parseTextureProgram(source, library);
  return compileTexture(library, { nodes: program.nodes, output: program.channels.albedo as { node: string } }, helpers);
};

test('a reaction_diffusion node compiles to a shader that samples its simulated picture, and the picture is made', () => {
  const compiled = compile('skin = reaction_diffusion(preset="spots", size=64, steps=1500, repeat_x=2, repeat_y=2)\nalbedo = colorize(skin, gradient=[0:#2b1a0e, 1:#d9a441])');
  expect(compiled.needs?.images.length).toBe(1);
  const name = compiled.needs!.images[0];
  expect(name).toMatch(/^rd_[0-9a-f]{8}$/);
  expect(compiled.source).toContain(`uniform sampler2D uImg_${name};`);
  const picture = rawPicture(name);
  expect(picture?.width).toBe(64);
  expect(picture?.pixels.length).toBe(64 * 64 * 4);
  // grey, opaque, with contrast
  const values = Array.from({ length: 64 * 64 }, (_, i) => picture!.pixels[i * 4]);
  expect(Math.max(...values)).toBe(255);
  expect(Math.min(...values)).toBe(0);
  expect(picture!.pixels[3]).toBe(255);
});

test('the same settings are the same picture, other settings or another seed another', () => {
  const name = (extra: string) => compile(`s = reaction_diffusion(size=24, steps=80${extra})\nalbedo = s`).needs!.images[0];
  expect(name('')).toBe(name(''));
  expect(name(', seed=2')).not.toBe(name(''));
  expect(name(', preset="maze"')).not.toBe(name(''));
  expect(name(', feed=0.05, kill=0.06')).not.toBe(name(''));
});

test('a program can use the simulated pattern next to ordinary nodes', () => {
  const compiled = compile('skin = reaction_diffusion(size=24, steps=80)\nn = fbm(scale_x=4, scale_y=4)\nmix = blend(skin, n, blend_type=multiply)\nalbedo = colorize(mix, gradient=[0:#000000, 1:#ffffff])');
  expect(compiled.needs?.images.length).toBe(1);
  expect(compiled.source).toContain('fbm');
});

test('refusals name the node and the fix', () => {
  expect(() => compile('s = reaction_diffusion(preset="leopard")\nalbedo = s')).toThrow(/preset "leopard" is not one of spots/);
  expect(() => compile('s = reaction_diffusion(size=512, steps=9000)\nalbedo = s')).toThrow(/million cell updates/);
  expect(() => compile('s = reaction_diffusion(size=10)\nalbedo = s')).toThrow(/size must be/);
  expect(() => compile('s = reaction_diffusion(repeat_x=0)\nalbedo = s')).toThrow(/repeat_x/);
});
