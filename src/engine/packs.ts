// Packs (G4): a bundle of macros, group kits and scripts with a manifest, and the rules for running them safely.
//
// A pack is DATA: text in the engine's own language (macros, groups) and JavaScript for the sealed script worker. It carries no native code and cannot add operators; everything it builds goes through the
// same typed commands, caps and validation as anything a person or model writes. The manifest says what the pack is, what it needs and what it may do:
//
//   { "name": "autorig_human", "version": "1.0.0", "description": "...", "license": "...",
//     "requires": { "operators": ["body.human@1", ...] },
//     "provides": { "macros": ["human_rig", ...], "groups": [], "scripts": [] },
//     "files": { "autorig_human.txt": { "kind": "macros", "sha256": "..." } },
//     "permissions": { "scripts": { "boss": { "world": ["game.spawn", "game.place"], "ui": true, "view": false } } } }
//
// Security model (ADR 0073): files are pinned by SHA-256 (a changed byte fails the load); a text file may use only operators the manifest requires, and every required operator must exist and be ready in the
// running engine; names a pack provides must be defined in its files and may not shadow another pack's; paths are plain file names (no folders, no `..`); each script runs in its own worker with ONLY the
// permissions its manifest entry grants (world method names, UI, view), enforced by the script host on every call, so a script cannot do what its manifest does not say; sizes are capped. A pack can be refused
// for any of these with a message that names the file or name and the fix.

export const MAX_PACK_FILES = 32;
export const MAX_PACK_FILE_BYTES = 256 * 1024;
export const MAX_PACK_BYTES = 1024 * 1024;

export type PackFileKind = 'macros' | 'group' | 'script';

export type ScriptPermissions = {
  /** World methods the script may call: exact names (`game.spawn`) or a family with a star (`game.*`). Nothing else is reachable. */
  world: string[];
  /** May draw UI (bars, text, flashes). */
  ui: boolean;
  /** May read where the player looks from and move the player. */
  view: boolean;
};

export type PackManifest = {
  name: string;
  version: string;
  description: string;
  license?: string;
  requires: { operators: string[] };
  provides: { macros: string[]; groups: string[]; scripts: string[] };
  files: Record<string, { kind: PackFileKind; sha256: string }>;
  permissions?: { scripts?: Record<string, ScriptPermissions> };
};

export type PackProblem = { code: string; file?: string; message: string };

/** Script permissions that grant nothing: the default for a pack script whose manifest entry is missing. */
export const NO_PERMISSIONS: ScriptPermissions = { world: [], ui: false, view: false };

/** Whether `method` is one of the world methods `permissions` grants. */
export function allows(permissions: ScriptPermissions | undefined, method: string): boolean {
  if (!permissions) return true; // a script that is not from a pack has the page's own, unchanged rules
  return permissions.world.some((rule) => (rule.endsWith('.*') ? method.startsWith(rule.slice(0, -1)) : rule === method));
}

const NAME = /^[a-z][a-z0-9_]{0,31}$/;
const SEMVER = /^\d{1,4}\.\d{1,4}\.\d{1,4}$/;
const FILE = /^[A-Za-z0-9][A-Za-z0-9_.-]{0,63}$/;
const OPERATOR_USE = /\b([a-z][a-z0-9_]*\.[a-z][a-z0-9_]*)@(\d+)\b/g;
const METHOD_RULE = /^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)*(\.\*)?$/;

/** SHA-256 of text (UTF-8) as lowercase hex. */
export async function sha256(text: string): Promise<string> {
  const bytes = new TextEncoder().encode(text);
  const digest = await crypto.subtle.digest('SHA-256', bytes);
  return [...new Uint8Array(digest)].map((b) => b.toString(16).padStart(2, '0')).join('');
}

/** The operator ids (`family.name@N`) a text uses, in order of first use. */
export function operatorsUsed(text: string): string[] {
  const seen = new Set<string>();
  for (const m of text.matchAll(OPERATOR_USE)) seen.add(`${m[1]}@${m[2]}`);
  return [...seen];
}

/** The names a macro text defines (`def name(`), and a group text (`group name`). */
export function definedMacros(text: string): string[] {
  return [...text.matchAll(/^def\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(/gm)].map((m) => m[1]);
}
export function definedGroups(text: string): string[] {
  return [...text.matchAll(/^group\s+([A-Za-z_][A-Za-z0-9_]*)/gm)].map((m) => m[1]);
}

/** World method names a script calls with a literal (`world.call('game.spawn', ...)`), for a check against its permissions (a computed name is caught at run time instead). */
export function methodsCalled(source: string): string[] {
  return [...new Set([...source.matchAll(/world\.call\(\s*(['"`])([A-Za-z0-9_.]+)\1/g)].map((m) => m[2]))];
}

/**
 * Checks a pack: its manifest's shape, every file's hash and size, the operators its texts use against what it requires and what the engine has, the names it provides against what its files define
 * (and against `taken`, names other packs already provide), and its scripts' permissions. Returns every problem found (none means the pack may be loaded).
 */
export async function validatePack(manifest: unknown, files: Record<string, string>, engine: { operators: ReadonlySet<string>; taken?: ReadonlySet<string> }): Promise<PackProblem[]> {
  const problems: PackProblem[] = [];
  const bad = (code: string, message: string, file?: string) => problems.push({ code, message, ...(file ? { file } : {}) });
  if (typeof manifest !== 'object' || manifest === null || Array.isArray(manifest)) return [{ code: 'pack.manifest', message: 'the manifest must be a JSON object' }];
  const m = manifest as Partial<PackManifest>;
  if (typeof m.name !== 'string' || !NAME.test(m.name)) bad('pack.name', 'name must be lowercase letters, digits and underscores, starting with a letter (32 at most)');
  if (typeof m.version !== 'string' || !SEMVER.test(m.version)) bad('pack.version', 'version must look like 1.2.3');
  if (typeof m.description !== 'string' || m.description.length < 10 || m.description.length > 600) bad('pack.description', 'description must say in 10 to 600 characters what the pack makes');
  const operators = m.requires?.operators;
  if (!Array.isArray(operators) || operators.some((o) => typeof o !== 'string')) bad('pack.requires', 'requires.operators must list the operator ids the pack uses, like "poly.grid@1"');
  const provides = m.provides;
  if (!provides || !Array.isArray(provides.macros) || !Array.isArray(provides.groups) || !Array.isArray(provides.scripts)) bad('pack.provides', 'provides must list macros, groups and scripts (each may be empty)');
  const listed = m.files;
  if (typeof listed !== 'object' || listed === null || Array.isArray(listed) || Object.keys(listed).length === 0) {
    bad('pack.files', 'files must name at least one file with its kind and sha256');
    return problems;
  }
  const names = Object.keys(listed);
  if (names.length > MAX_PACK_FILES) bad('pack.too_many_files', `at most ${MAX_PACK_FILES} files, got ${names.length}`);
  let total = 0;
  const macros = new Set<string>(), groups = new Set<string>(), scripts = new Set<string>();
  const used = new Set<string>();
  for (const file of names) {
    const entry = listed[file];
    if (!FILE.test(file) || file.includes('..')) { bad('pack.path', `"${file}" must be a plain file name (letters, digits, _ . -), no folders and no ..`, file); continue; }
    if (!entry || !['macros', 'group', 'script'].includes(entry.kind) || typeof entry.sha256 !== 'string') { bad('pack.file_entry', `"${file}" needs a kind (macros, group or script) and a sha256`, file); continue; }
    const text = files[file];
    if (typeof text !== 'string') { bad('pack.missing_file', `"${file}" is listed in the manifest but was not supplied`, file); continue; }
    const size = new TextEncoder().encode(text).length;
    total += size;
    if (size > MAX_PACK_FILE_BYTES) bad('pack.file_size', `"${file}" is ${size} bytes; the most is ${MAX_PACK_FILE_BYTES}`, file);
    const hash = await sha256(text);
    if (hash !== entry.sha256) bad('pack.hash', `"${file}" does not match its sha256 (it is ${hash}); the file was changed or the manifest is out of date`, file);
    if (entry.kind === 'script') {
      scripts.add(file.replace(/\.js$/, ''));
      continue;
    }
    for (const id of operatorsUsed(text)) used.add(id);
    for (const id of operatorsUsed(text)) {
      if (Array.isArray(operators) && !operators.includes(id)) bad('pack.undeclared_operator', `"${file}" uses ${id}, which the manifest does not list under requires.operators`, file);
    }
    (entry.kind === 'macros' ? definedMacros(text) : definedGroups(text)).forEach((n) => (entry.kind === 'macros' ? macros : groups).add(n));
  }
  for (const extra of Object.keys(files)) if (!names.includes(extra)) bad('pack.unlisted_file', `"${extra}" was supplied but is not listed in the manifest`, extra);
  if (total > MAX_PACK_BYTES) bad('pack.size', `the pack is ${total} bytes; the most is ${MAX_PACK_BYTES}`);
  if (Array.isArray(operators)) {
    for (const id of operators as string[]) if (!engine.operators.has(id)) bad('pack.unknown_operator', `requires ${id}, which this engine does not have (or is not ready)`);
    for (const id of operators as string[]) if (!used.has(id)) bad('pack.unused_operator', `requires ${id} but none of its files uses it; remove it from requires.operators`);
  }
  if (provides && Array.isArray(provides.macros) && Array.isArray(provides.groups) && Array.isArray(provides.scripts)) {
    const compare = (kind: string, declared: string[], defined: Set<string>) => {
      for (const n of declared) if (!defined.has(n)) bad('pack.provides_missing', `provides ${kind} "${n}" but no file defines it`);
      for (const n of defined) if (!declared.includes(n)) bad('pack.provides_undeclared', `a file defines ${kind} "${n}" that provides does not list`);
      for (const n of declared) if (engine.taken?.has(n)) bad('pack.name_taken', `${kind} "${n}" is already provided by another pack`);
    };
    compare('macro', provides.macros, macros);
    compare('group', provides.groups, groups);
    compare('script', provides.scripts, scripts);
  }
  // Scripts: each needs a permissions entry (granting nothing is fine), the entries must be well formed, and a script may not call what it does not hold.
  const granted = m.permissions?.scripts ?? {};
  for (const script of scripts) {
    const file = `${script}.js`;
    const permissions = granted[script];
    if (!permissions) { bad('pack.script_permissions', `script "${script}" has no entry in permissions.scripts (an entry that grants nothing is { "world": [], "ui": false, "view": false })`, file); continue; }
    if (!Array.isArray(permissions.world) || permissions.world.some((rule) => typeof rule !== 'string' || !METHOD_RULE.test(rule)) || typeof permissions.ui !== 'boolean' || typeof permissions.view !== 'boolean') {
      bad('pack.script_permissions', `permissions of "${script}" must be { "world": ["game.spawn", "game.*"], "ui": true|false, "view": true|false }`, file);
      continue;
    }
    if (permissions.world.includes('*') || permissions.world.some((rule) => rule === '*.*')) bad('pack.script_permissions', `"${script}" asks for every world method; list the ones it needs`, file);
    for (const method of methodsCalled(files[file] ?? '')) {
      if (!allows(permissions, method)) bad('pack.script_call', `script "${script}" calls ${method}, which its permissions do not grant (add it to permissions.scripts.${script}.world or remove the call)`, file);
    }
  }
  for (const script of Object.keys(granted)) if (!scripts.has(script)) bad('pack.script_permissions', `permissions name a script "${script}" the pack does not have`);
  return problems;
}
