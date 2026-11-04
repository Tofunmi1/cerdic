# ADR 0073: Packs, and what a pack's scripts may do

- Status: accepted (roadmap item G4)
- Date: 2026-11-04

## Context

The engine's reusable work now comes in three forms: macros and group kits written in the text language (an autorig, hair, face presets), scripts that run in the sealed script worker (ADR 0065), and, later, texture programs. They were shipped as loose files in the engine's source (`ve-modeling/kits`), with no statement of what a bundle needs, what it provides, or what its scripts may do. A script from anywhere could call every world method, draw UI and move the player. That is fine for the page's own tool, and wrong for code that arrives in a bundle someone else made.

## Decision

A **pack** is a folder of text and JavaScript files with a `pack.json` manifest (`src/engine/packs.ts`). It is data: no native code, no network, and it cannot add operators. Everything it builds goes through the same typed commands, limits and validation as anything else.

The manifest names the pack and its version, lists the operator ids it **requires** (`poly.grid@1`), the macros, groups and scripts it **provides**, every file with its **kind** and **SHA-256**, and, per script, its **permissions**.

Loading checks, and refuses with the problem named by file or name and the fix:

- the manifest's shape, name and version; file names are plain (no folders, no `..`); at most 32 files, 256 KiB each, 1 MiB in all;
- every file's hash (a changed byte fails), no unlisted or missing file;
- a text file uses only operators listed in `requires`; every required operator exists and is ready in the running engine (the ids come from the registry through the generated `src/engine/operators.d.ts`, roadmap item G3); a required operator no file uses is an error, so the list stays honest;
- the macro and group names in `provides` equal what the files define, and no name is already provided by another pack;
- each script has a permissions entry (one that grants nothing is valid); entries are well formed; `*` alone is not a permission; a script's literal `world.call('method')` must be inside its permissions.

A script from a pack runs in its own worker as before (ADR 0065) and the script host enforces its permissions **on every use**, not only at load: a world method outside its list is answered with `permission denied` and never reaches the world; UI and view requests (pose, teleport) are denied unless granted. Permissions are a list of world method names (exact, or a family with a star: `game.*`), and two switches, `ui` and `view`. A script that is not from a pack keeps the page's own rules, unchanged.

`installPack` (`src/engine/pack-loader.ts`) validates, then starts the scripts as `<pack>_<script>` with their permissions, or starts nothing. The shipped packs (`engine/packs/`) are built by `scripts/pack.ts` from the kit sources, with the manifest generated from the files, and checked by `bun scripts/pack.ts --check`.

## Consequences

- A model or person can read a pack's manifest to learn what it needs and may do before running it.
- Tampering and drift are caught by hashes; engine drift (an operator removed or renamed) is caught by `requires`.
- Static scanning of a script's literal method names is a courtesy; the run-time check is the guarantee, so a computed method name is still refused if it is not granted.
- The permission model covers world methods, UI and the view. It does not limit CPU or memory beyond the script host's existing frame deadline, line and size caps. Scripts from unknown sources remain a trust decision for the owner; this ADR makes the decision narrow and visible.
- Not done: a signature scheme (hashes pin content, they do not say who made it), pack versions with dependencies on other packs, packs of texture programs.

## Validation

`tests/packs.test.ts` (validator), `tests/script-permissions.test.ts` (enforcement through the script host with a stand-in worker, and install), and `bun scripts/pack.ts --check`.
