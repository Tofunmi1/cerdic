import { expect, test } from 'bun:test';
import { readFile, readdir } from 'node:fs/promises';
import { NO_PERMISSIONS, allows, methodsCalled, operatorsUsed, sha256, validatePack, type PackManifest } from '../src/engine/packs';

const root = new URL('../', import.meta.url).pathname;
// The engine's operators, from the declarations generated from its registry.
const declared = await readFile(`${root}src/engine/operators.d.ts`, 'utf8');
const operators = new Set([...declared.slice(declared.indexOf('export type OperatorId'), declared.indexOf('declare const kindOf')).matchAll(/'([a-z0-9_.]+@\d+)'/g)].map((m) => m[1]));

const macroText = 'def make(src)\n  a_{src} = solid.box@1(size=[1, 1, 1])\n  output {src} = a_{src}\nend\n';
const scriptText = "on('frame', async () => { await world.call('game.spawn', { kind: 'x' }); });\n";

async function pack(extra: Partial<PackManifest> = {}, files: Record<string, string> = {}) {
  const all = { 'tools.txt': macroText, 'boss.js': scriptText, ...files };
  const manifest: PackManifest = {
    name: 'tools', version: '1.0.0', description: 'A macro that makes a box and a script that spawns a thing.',
    requires: { operators: ['solid.box@1'] },
    provides: { macros: ['make'], groups: [], scripts: ['boss'] },
    files: { 'tools.txt': { kind: 'macros', sha256: await sha256(all['tools.txt']) }, 'boss.js': { kind: 'script', sha256: await sha256(all['boss.js']) } },
    permissions: { scripts: { boss: { world: ['game.spawn'], ui: false, view: false } } },
    ...extra,
  };
  return { manifest, files: all };
}
const check = async (extra: Partial<PackManifest> = {}, files: Record<string, string> = {}, engine = { operators }) => {
  const { manifest, files: supplied } = await pack(extra, files);
  return validatePack(manifest, supplied, engine);
};
const codes = (problems: { code: string }[]) => problems.map((p) => p.code);

test('a well formed pack with honest permissions is accepted', async () => {
  expect(await check()).toEqual([]);
});

test('a changed byte fails the hash, a file not listed or not supplied is refused, and paths must be plain names', async () => {
  const { manifest } = await pack();
  expect(codes(await validatePack(manifest, { 'tools.txt': `${macroText}# edit\n`, 'boss.js': scriptText }, { operators }))).toContain('pack.hash');
  expect(codes(await check({}, { 'extra.txt': 'x' }))).toContain('pack.unlisted_file');
  expect(codes(await validatePack(manifest, { 'tools.txt': macroText }, { operators }))).toContain('pack.missing_file');
  const sneaky = { ...manifest, files: { ...manifest.files, '../evil.txt': { kind: 'macros' as const, sha256: 'x' } } };
  expect(codes(await validatePack(sneaky, { 'tools.txt': macroText, 'boss.js': scriptText }, { operators }))).toContain('pack.path');
});

test('operators: a text may use only what the manifest requires, and what is required must exist and be used', async () => {
  const used = `${macroText}  b = poly.grid@1(width=1, height=1)\n`;
  const { manifest, files } = await pack({}, { 'tools.txt': used });
  manifest.files['tools.txt'].sha256 = await sha256(used);
  expect(codes(await validatePack(manifest, files, { operators }))).toContain('pack.undeclared_operator');
  expect(codes(await check({ requires: { operators: ['solid.box@1', 'solid.nonsense@1'] } }))).toEqual(expect.arrayContaining(['pack.unknown_operator', 'pack.unused_operator']));
  expect(codes(await check({}, {}, { operators: new Set(['solid.sphere@1']) }))).toContain('pack.unknown_operator');
});

test('provided names must be defined, defined names must be provided, and another pack\'s names are taken', async () => {
  expect(codes(await check({ provides: { macros: ['make', 'ghost'], groups: [], scripts: ['boss'] } }))).toContain('pack.provides_missing');
  expect(codes(await check({ provides: { macros: [], groups: [], scripts: ['boss'] } }))).toContain('pack.provides_undeclared');
  expect(codes(await check({}, {}, { operators, taken: new Set(['make']) } as never))).toContain('pack.name_taken');
});

test('script permissions: every script needs an entry, calls outside it are refused, and "everything" is not a permission', async () => {
  expect(codes(await check({ permissions: { scripts: {} } }))).toContain('pack.script_permissions');
  expect(codes(await check({ permissions: { scripts: { boss: { world: [], ui: false, view: false } } } }))).toContain('pack.script_call');
  expect(codes(await check({ permissions: { scripts: { boss: { world: ['*'], ui: false, view: false } } } }))).toContain('pack.script_permissions');
  expect(codes(await check({ permissions: { scripts: { boss: { world: ['game.*'], ui: true, view: false } } } }))).toEqual([]);
  expect(codes(await check({ permissions: { scripts: { boss: { world: ['game.spawn'], ui: false, view: false }, ghost: { world: [], ui: false, view: false } } } }))).toContain('pack.script_permissions');
  expect(codes(await check({ permissions: { scripts: { boss: { world: ['Not A Method'], ui: false, view: false } } } }))).toContain('pack.script_permissions');
});

test('shape and size problems are reported together with what to do', async () => {
  const problems = await check({ name: 'Bad Name', version: '1', description: 'short' });
  expect(codes(problems)).toEqual(expect.arrayContaining(['pack.name', 'pack.version', 'pack.description']));
  expect(await validatePack(42, {}, { operators })).toEqual([{ code: 'pack.manifest', message: 'the manifest must be a JSON object' }]);
  const big = 'x'.repeat(300 * 1024);
  const { manifest } = await pack({}, { 'tools.txt': big });
  manifest.files['tools.txt'].sha256 = await sha256(big);
  expect(codes(await validatePack(manifest, { 'tools.txt': big, 'boss.js': scriptText }, { operators }))).toContain('pack.file_size');
});

test('allows(): exact names, families with a star, nothing for no permissions, everything for a script that is not from a pack', () => {
  expect(allows(undefined, 'anything.at_all')).toBe(true);
  expect(allows(NO_PERMISSIONS, 'game.spawn')).toBe(false);
  const grant = { world: ['game.spawn', 'world.*'], ui: false, view: false };
  expect(allows(grant, 'game.spawn')).toBe(true);
  expect(allows(grant, 'game.place')).toBe(false);
  expect(allows(grant, 'world.insert_components')).toBe(true);
  expect(allows(grant, 'worldly')).toBe(false);
});

test('the helpers read operators and called methods out of text', () => {
  expect(operatorsUsed('a = solid.box@1(x)\nb = xform.move@2(a)\nc = solid.box@1(y)')).toEqual(['solid.box@1', 'xform.move@2']);
  expect(methodsCalled("await world.call('game.spawn', {}); world.call(\"game.place\", {}); world.call(name, {})")).toEqual(['game.spawn', 'game.place']);
});

test('every shipped pack validates against the engine, its files match the kit sources, and no two packs provide the same name', async () => {
  const dirs = await readdir(`${root}engine/packs`);
  expect(dirs.sort()).toEqual(['autorig_human', 'face_presets', 'hair_cards']);
  const taken = new Set<string>();
  for (const dir of dirs) {
    const manifest = JSON.parse(await readFile(`${root}engine/packs/${dir}/pack.json`, 'utf8')) as PackManifest;
    const files: Record<string, string> = {};
    for (const file of Object.keys(manifest.files)) {
      files[file] = await readFile(`${root}engine/packs/${dir}/${file}`, 'utf8');
      // The pack's file IS the kit the engine ships (no copy that can drift).
      expect(files[file]).toBe(await readFile(`${root}engine/crates/ve-modeling/kits/${file}`, 'utf8'));
    }
    expect(await validatePack(manifest, files, { operators, taken })).toEqual([]);
    for (const name of [...manifest.provides.macros, ...manifest.provides.groups]) taken.add(name);
  }
});
