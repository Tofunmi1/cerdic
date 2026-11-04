import { afterAll, expect, test } from 'bun:test';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
// @ts-expect-error plain module with no declarations
import { MAX_BYTES, buildImport, nodeName, quote } from '../engine/tools/mcp/import-file.mjs';

const dir = mkdtempSync(join(tmpdir(), 'import-file-'));
afterAll(() => rmSync(dir, { recursive: true, force: true }));
const write = (name: string, data: string | Uint8Array) => { const file = join(dir, name); writeFileSync(file, data); return file; };

test('a file name becomes a valid node name', () => {
  expect(nodeName('/x/My Chair-v2.OBJ')).toBe('my_chair_v2');
  expect(nodeName('/x/3d model.stl')).toBe('mesh_3d_model');
  expect(nodeName('/x/' + 'a'.repeat(60) + '.ply').length).toBe(32);
  expect(nodeName('/x/___.obj')).toBe('mesh');
});

test('quote() escapes what the text language needs and nothing else', () => {
  expect(quote('v 1 2 3\r\nf "a" \\')).toBe('"v 1 2 3\\nf \\"a\\" \\\\"');
});

test('an obj becomes one poly.import line plus an output, without its comment lines', () => {
  const file = write('chair.obj', '# made by me\nv 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n');
  const built = buildImport({ path: file, scale: 0.001, up: 'z' });
  expect(built.error).toBeUndefined();
  expect(built.text).toBe('chair = poly.import@1(format="obj", data="v 0 0 0\\nv 1 0 0\\nv 0 1 0\\nf 1 2 3\\n", scale=0.001, up="z")\noutput chair = chair\n');
  expect(buildImport({ path: file, output: false, name: 'seat' }).text).not.toContain('output');
  expect(buildImport({ path: file, name: 'seat' }).text.startsWith('seat = poly.import@1')).toBe(true);
});

test('the format comes from the extension, or from format when the extension says nothing', () => {
  expect(buildImport({ path: write('a.ply', 'ply\n') }).format).toBe('ply');
  expect(buildImport({ path: write('b.STL', 'solid x\n') }).format).toBe('stl');
  expect(buildImport({ path: write('c.txt', 'v 0 0 0\n'), format: 'obj' }).format).toBe('obj');
  expect(buildImport({ path: write('d.txt', 'v 0 0 0\n') }).error).toContain('cannot tell the format');
});

test('refusals say what to do: missing file, binary file, too large, bad options', () => {
  expect(buildImport({}).error).toContain('needs path');
  expect(buildImport({ path: join(dir, 'nope.obj') }).error).toContain('does not exist');
  expect(buildImport({ path: write('bin.stl', new Uint8Array([115, 111, 0, 1, 2])) }).error).toContain('looks binary');
  expect(buildImport({ path: write('big.obj', 'v 0 0 0\n'.repeat(MAX_BYTES / 8 + 10)) }).error).toContain('the most a node holds');
  const ok = write('ok.obj', 'v 0 0 0\n');
  expect(buildImport({ path: ok, up: 'x' }).error).toContain('"y" or "z"');
  expect(buildImport({ path: ok, scale: -1 }).error).toContain('positive');
  expect(buildImport({ path: ok, name: 'Bad Name' }).error).toContain('lowercase');
});
