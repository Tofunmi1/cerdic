// The `expr` texture node: shader expression code inside a texture program, through the real Material Maker library.
import { expect, test } from 'bun:test';
import { compileTexture } from '../src/engine/texture/compile';
import { loadHelpers, loadLibrary } from '../src/engine/texture/library';
import { parseTextureProgram, TextureError } from '../src/engine/texture/program';

const root = new URL('../engine/third_party/material-maker/', import.meta.url).pathname;
const library = loadLibrary(root);
const helpers = loadHelpers(root);

const rust = `
  n = expr(code="a = fbm(p * 6); out = hsv(0.07, 0.8, 0.2 + a * 0.6)")
  albedo = n
`;

function shaderOf(text: string, channel: 'albedo' | 'height' = 'albedo'): string {
  const program = parseTextureProgram(text, library);
  const output = program.channels[channel]!;
  return compileTexture(library, { nodes: program.nodes, output }, helpers).source;
}

test('an expr node parses its code as a quoted string, even with ; and // inside', () => {
  const program = parseTextureProgram(`
    n = expr(code="a = p.x; // not a comment of the program\\nout = vec3(a)")   // this one is
    albedo = n
  `, library);
  expect(program.nodes[0].type).toBe('expr');
  expect(program.nodes[0].params?.code).toBe('a = p.x; // not a comment of the program\nout = vec3(a)');
});

test('an expr node compiles into a function of its own and a call to it', () => {
  const source = shaderOf(rust);
  expect(source).toMatch(/vec4 \w+_expr\(vec2 uv\) \{/);
  expect(source).toContain('float u_a = ex_fbm((p * 6.0));');
  expect(source).toContain('ex_hsv(');
  expect(source).toContain('vec4 result =');
  expect(source).toContain('return result;');
});

test('the helper functions are in the shader once, however many expr nodes use them', () => {
  const source = shaderOf(`
    a = expr(code="out = fbm(p * 3)")
    b = expr(code="out = snoise(p * 9)")
    c = colorize(a, gradient=[0:#000000, 1:#ffffff])
    albedo = c
    height = b
  `);
  expect(source.match(/float ex_fbm\(/g)).toHaveLength(1);
  const height = shaderOf(`
    a = expr(code="out = fbm(p * 3)")
    b = expr(code="out = snoise(p * 9)")
    albedo = a
    height = b
  `, 'height');
  expect(height.match(/float ex_snoise\(/g)).toHaveLength(1);
  expect(height.match(/_expr\(vec2 uv\) \{/g)).toHaveLength(1);
});

test('an expr node composes with other nodes: its output feeds colorize, and two of them can blend', () => {
  const source = shaderOf(`
    a = expr(code="out = fbm(p * 4)")
    b = expr(code="out = 0.5 + 0.5 * sin(p.x * 40)")
    c = colorize(a, gradient=[0:#223311, 1:#99bb55])
    albedo = c
    roughness = b
  `);
  expect(source).toContain('_expr(vec2 uv)');
  expect(source.match(/_expr\(vec2 uv\) \{/g)).toHaveLength(1);
});

test('a code mistake names the node, the line and column, and the fix', () => {
  const program = parseTextureProgram(`n = expr(code="a = sinn(p.x)\\nout = a")\nalbedo = n`, library);
  expect(() => compileTexture(library, { nodes: program.nodes, output: program.channels.albedo! }, helpers))
    .toThrow(/expr node "n" \(expr\.unknown_name, line 1, column 5\).*Did you mean sin\(\)/);
});

test('code that is missing, unquoted or has no out is refused with a fix', () => {
  expect(() => parseTextureProgram('n = expr(code=out = 1)\nalbedo = n', library)).toThrow(TextureError);
  expect(() => parseTextureProgram('n = expr(code=out = 1)\nalbedo = n', library)).toThrow(/double quotes/);
  const noOut = parseTextureProgram('n = expr(code="a = 1")\nalbedo = n', library);
  expect(() => compileTexture(library, { nodes: noOut.nodes, output: noOut.channels.albedo! }, helpers)).toThrow(/expr\.no_out/);
  const empty = parseTextureProgram('n = expr()\nalbedo = n', library);
  expect(() => compileTexture(library, { nodes: empty.nodes, output: empty.channels.albedo! }, helpers)).toThrow(/needs code=/);
});

test('graphs without an expr node compile exactly as before', () => {
  const program = parseTextureProgram('n = perlin(scale_x=4, scale_y=4)\nalbedo = n', library);
  const compiled = compileTexture(library, { nodes: program.nodes, output: program.channels.albedo! }, helpers).source;
  expect(compiled).not.toContain('ex_');
});
