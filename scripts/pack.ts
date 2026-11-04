// Builds the shipped packs (engine/packs/<name>/) from their sources, or checks that they are current.
//   bun scripts/pack.ts            writes engine/packs/<name>/{files, pack.json}
//   bun scripts/pack.ts --check    fails if any shipped pack differs from what the sources make
// The manifest is generated, not written: file hashes, the operators the texts use and the macro and group names they define all come from the files themselves, so it cannot drift from them.
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { definedGroups, definedMacros, operatorsUsed, sha256, type PackManifest } from '../src/engine/packs';

const root = new URL('../', import.meta.url).pathname;
const kits = `${root}engine/crates/ve-modeling/kits/`;

type Source = { name: string; version: string; description: string; license: string; files: { file: string; kind: 'macros' | 'group' }[] };
export const SOURCES: Source[] = [
  { name: 'autorig_human', version: '1.0.0', license: 'GPL-2.0-or-later', files: [{ file: 'autorig_human.txt', kind: 'macros' }],
    description: 'Rigs a body.human in three lines: human_rig makes every bone a part with its parent link and output, human_skin skins it with the pack\'s weights, human_ik adds reach targets; the fingers carry fist and point clips.' },
  { name: 'hair_cards', version: '1.0.0', license: 'GPL-2.0-or-later', files: [{ file: 'hair_cards.txt', kind: 'macros' }],
    description: 'Hair that moves: hair_cards builds strips hanging from a ring of roots and simulates them as cloth pinned to a head bone, so they swing, trail a turning head and blow in the wind.' },
  { name: 'face_presets', version: '1.0.0', license: 'GPL-2.0-or-later', files: [{ file: 'face_presets.txt', kind: 'macros' }],
    description: 'Facial expressions for body.human as text constants: face_happy, face_sad, face_angry, face_surprised, face_wink, face_blink, face_talk and face_disgust, each a set of ARKit face actions.' },
];

export async function build(source: Source): Promise<{ manifest: PackManifest; files: Record<string, string> }> {
  const files: Record<string, string> = {};
  for (const { file } of source.files) files[file] = await readFile(`${kits}${file}`, 'utf8');
  const operators = new Set<string>();
  const macros: string[] = [], groups: string[] = [];
  for (const text of Object.values(files)) { operatorsUsed(text).forEach((o) => operators.add(o)); }
  const listed: PackManifest['files'] = {};
  for (const { file, kind } of source.files) {
    listed[file] = { kind, sha256: await sha256(files[file]) };
    macros.push(...(kind === 'macros' ? definedMacros(files[file]) : []));
    groups.push(...(kind === 'group' ? definedGroups(files[file]) : []));
  }
  const manifest: PackManifest = {
    name: source.name, version: source.version, description: source.description, license: source.license,
    requires: { operators: [...operators].sort() },
    // Every macro and group the files define, helpers included: their names are global in a program, so two packs must not define the same one.
    provides: { macros, groups, scripts: [] },
    files: listed,
  };
  return { manifest, files };
}

if (import.meta.main) {
  const check = process.argv.includes('--check');
  let drift = 0;
  for (const source of SOURCES) {
    const { manifest, files } = await build(source);
    const dir = `${root}engine/packs/${source.name}/`;
    const outputs: Record<string, string> = { ...files, 'pack.json': `${JSON.stringify(manifest, null, 2)}\n` };
    for (const [file, text] of Object.entries(outputs)) {
      if (check) {
        const have = await readFile(`${dir}${file}`, 'utf8').catch(() => null);
        if (have !== text) { console.error(`pack ${source.name}: ${file} is ${have === null ? 'missing' : 'out of date'}`); drift++; }
      } else {
        await mkdir(dir, { recursive: true });
        await writeFile(`${dir}${file}`, text);
      }
    }
  }
  if (check && drift) { console.error('run bun scripts/pack.ts to rebuild the packs'); process.exit(1); }
  console.log(check ? 'packs are current' : `built ${SOURCES.length} packs`);
}
