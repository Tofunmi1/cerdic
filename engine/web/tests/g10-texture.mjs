import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir, writeFile } from 'node:fs/promises';
import { PNG } from 'pngjs';
import { CURATED } from '../../../src/engine/texture/catalog.ts';
import { compileTexture } from '../../../src/engine/texture/compile.ts';
import { loadHelpers, loadLibrary } from '../../../src/engine/texture/library.ts';

// G10: the texture generator. Material Maker's node library (vendored, MIT) is compiled into GLSL by our
// compiler and run in WebGL2. This checks that (1) a set of graphs bakes to real, non-flat, repeatable
// images, and (2) how much of the library compiles and links at all, reported as a number.
const root = new URL('../../lib/material-maker/', import.meta.url).pathname;
const reports = new URL('../../target/reports/g10/', import.meta.url).pathname;
const library = loadLibrary(root);
const helpers = loadHelpers(root);

const colour = (r, g, b, a = 1) => ({ r, g, b, a });
const ramp = (...stops) => ({ type: 'Gradient', interpolation: 1, points: stops.map(([pos, r, g, b]) => ({ pos, r, g, b, a: 1 })) });
const graphs = {
  clouds: { nodes: [{ id: 'n', type: 'fbm' }, { id: 'c', type: 'colorize', params: { gradient: ramp([0, 0.05, 0.12, 0.35], [1, 0.95, 0.97, 1]) }, inputs: { input: { node: 'n' } } }], output: { node: 'c' } },
  cells: { nodes: [{ id: 'v', type: 'voronoi' }], output: { node: 'v', output: 0 } },
  bricks: { nodes: [{ id: 'b', type: 'bricks' }], output: { node: 'b', output: 0 } },
  weave: { nodes: [{ id: 'w', type: 'weave' }], output: { node: 'w', output: 0 } },
};

const attempts = {};
for (const [name, graph] of Object.entries(graphs)) {
  try { attempts[name] = { source: compileTexture(library, graph, helpers).source }; } catch (error) { attempts[name] = { error: String(error.message) }; }
}
const every = {};
for (const name of Object.keys(library)) {
  try { every[name] = { source: compileTexture(library, { nodes: [{ id: 'x', type: name }], output: { node: 'x' } }, helpers).source }; } catch (error) { every[name] = { error: String(error.message).slice(0, 120) }; }
}

const browser = await chromium.launch({
  channel: process.env.PLAYWRIGHT_CHANNEL || undefined,
  args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'],
});
try {
  await mkdir(reports, { recursive: true });
  const page = await browser.newPage();
  await page.setContent('<canvas id="c" width="256" height="256"></canvas>');
  const bake = (sources, size) => page.evaluate(({ sources, size }) => {
    const canvas = new OffscreenCanvas(size, size);
    const gl = canvas.getContext('webgl2', { preserveDrawingBuffer: true, antialias: false });
    if (!gl) return { error: 'no webgl2' };
    const vertex = '#version 300 es\nout vec2 vUv; void main(){ vec2 p = vec2((gl_VertexID<<1)&2, gl_VertexID&2); vUv = p; gl_Position = vec4(p*2.0-1.0, 0.0, 1.0); }';
    const shader = (type, text) => { const s = gl.createShader(type); gl.shaderSource(s, text); gl.compileShader(s); return gl.getShaderParameter(s, gl.COMPILE_STATUS) ? s : { log: gl.getShaderInfoLog(s) }; };
    const results = {};
    for (const [name, source] of Object.entries(sources)) {
      const v = shader(gl.VERTEX_SHADER, vertex), f = shader(gl.FRAGMENT_SHADER, source);
      if (v.log || f.log) { results[name] = { error: (v.log || f.log).split('\n').find(l => l.includes('ERROR')) ?? 'compile error' }; continue; }
      const program = gl.createProgram();
      gl.attachShader(program, v); gl.attachShader(program, f); gl.linkProgram(program);
      if (!gl.getProgramParameter(program, gl.LINK_STATUS)) { results[name] = { error: 'link: ' + gl.getProgramInfoLog(program) }; continue; }
      gl.viewport(0, 0, size, size);
      gl.useProgram(program);
      if (source.includes('uPosition')) {
        // 3D shaders read one object-space position per texel: here a smooth ramp through the unit cube.
        const positions = new Float32Array(size * size * 4);
        for (let y = 0; y < size; y++) for (let x = 0; x < size; x++) positions.set([x / size, y / size, ((x + y) / (2 * size)), 1], (y * size + x) * 4);
        const texture = gl.createTexture();
        gl.activeTexture(gl.TEXTURE0);
        gl.bindTexture(gl.TEXTURE_2D, texture);
        gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA8, size, size, 0, gl.RGBA, gl.UNSIGNED_BYTE, new Uint8Array(Array.from(positions, (v) => Math.round(v * 255))));
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
        gl.uniform1i(gl.getUniformLocation(program, 'uPosition'), 0);
      }
      gl.drawArrays(gl.TRIANGLES, 0, 3);
      const pixels = new Uint8Array(size * size * 4);
      gl.readPixels(0, 0, size, size, gl.RGBA, gl.UNSIGNED_BYTE, pixels);
      results[name] = { pixels: Array.from(pixels) };
    }
    return results;
  }, { sources, size });

  const sources = Object.fromEntries(Object.entries(attempts).filter(([, a]) => a.source).map(([n, a]) => [n, a.source]));
  const first = await bake(sources, 256);
  const second = await bake(sources, 256);
  for (const [name, attempt] of Object.entries(attempts)) {
    assert(attempt.source, `${name} compiles: ${attempt.error}`);
    const image = first[name];
    assert(image?.pixels, `${name} bakes: ${image?.error}`);
    const data = Uint8Array.from(image.pixels);
    const grey = [];
    for (let i = 0; i < data.length; i += 4) grey.push((data[i] + data[i + 1] + data[i + 2]) / 3);
    const mean = grey.reduce((s, v) => s + v, 0) / grey.length;
    const deviation = Math.sqrt(grey.reduce((s, v) => s + (v - mean) ** 2, 0) / grey.length);
    assert(deviation > 8, `${name} is not flat (deviation ${deviation.toFixed(1)})`);
    assert.deepEqual(second[name].pixels, image.pixels, `${name} bakes the same twice`);
    const png = new PNG({ width: 256, height: 256 });
    png.data = Buffer.from(data);
    await writeFile(`${reports}texture-${name}.png`, PNG.sync.write(png));
    console.log(`G10 texture ${name}: baked, deviation ${deviation.toFixed(1)}`);
  }

  // Coverage: how many library nodes compile in our compiler and in the GPU driver.
  const compiled = Object.entries(every).filter(([, e]) => e.source);
  const gpu = await bake(Object.fromEntries(compiled.map(([n, e]) => [n, e.source])), 8);
  const works = compiled.filter(([n]) => gpu[n]?.pixels).map(([n]) => n);
  const compileErrors = Object.entries(every).filter(([, e]) => e.error);
  const gpuErrors = compiled.filter(([n]) => gpu[n]?.error).map(([n]) => `${n}: ${gpu[n].error}`);
  const tally = {};
  for (const [, e] of compileErrors) { const key = e.error.replace(/"[^"]*"/g, '"_"').replace(/\$\w+/g, '$_').slice(0, 70); tally[key] = (tally[key] ?? 0) + 1; }
  await writeFile(`${reports}texture-coverage.json`, JSON.stringify({ total: Object.keys(library).length, works: works.length, workingNodes: works, ourCompilerRefused: tally, gpuRefused: gpuErrors }, null, 2));
  console.log(`G10 texture library: ${works.length} of ${Object.keys(library).length} nodes compile and link on their own (our compiler refused ${compileErrors.length}, the GPU refused ${gpuErrors.length})`);
  assert(works.length >= 340, 'most of the library works');
  const missing = CURATED.filter((name) => !works.includes(name));
  assert.deepEqual(missing, [], 'every curated node compiles and links on the GPU');
} finally {
  await browser.close();
}
