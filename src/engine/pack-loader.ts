// Installing a pack: validate it, then start its scripts with exactly the permissions its manifest grants. A pack that does not validate starts nothing and the problems come back.
import { NO_PERMISSIONS, validatePack, type PackManifest, type PackProblem } from './packs';
import type { ScriptHost } from './scripts';

export type InstalledPack = { name: string; version: string; scripts: string[]; macros: string[]; groups: string[]; text: string };

/**
 * Checks `manifest` and `files` against the engine, and on success starts each of the pack's scripts (named `<pack>_<script>`, so two packs cannot collide) with its manifest permissions.
 * `text` is the pack's macro and group source, joined, ready to put above a program.
 */
export async function installPack(
  scripts: ScriptHost,
  manifest: PackManifest,
  files: Record<string, string>,
  engine: { operators: ReadonlySet<string>; taken?: ReadonlySet<string> },
): Promise<{ ok: true; pack: InstalledPack } | { ok: false; problems: PackProblem[] }> {
  const problems = await validatePack(manifest, files, engine);
  if (problems.length) return { ok: false, problems };
  const started: string[] = [];
  for (const script of manifest.provides.scripts) {
    const name = `${manifest.name}_${script}`;
    const result = scripts.start(name, files[`${script}.js`], 'tool', manifest.permissions?.scripts?.[script] ?? NO_PERMISSIONS);
    if (!result.ok) {
      // Leave nothing half installed.
      for (const done of started) scripts.stop(done);
      return { ok: false, problems: [{ code: 'pack.script_start', file: `${script}.js`, message: `the script "${script}" could not start: ${(result as { error?: string }).error ?? 'unknown reason'}` }] };
    }
    started.push(name);
  }
  const text = Object.entries(manifest.files).filter(([, entry]) => entry.kind !== 'script').map(([file]) => files[file]).join('\n');
  return { ok: true, pack: { name: manifest.name, version: manifest.version, scripts: started, macros: manifest.provides.macros, groups: manifest.provides.groups, text } };
}
