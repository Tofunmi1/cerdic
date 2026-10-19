# Handoff to Codex: export LOD chains in the glTF exporter, with the exact tests below

Read `engine/AGENTS.md` first and follow it (no commits, no `Co-Authored-By` or tool attribution; TypeScript follows the style of `src/engine/gltf.ts`; no network). **Never run `cargo fmt`/`cargo fmt --all`.** Do not build the
browser bundle. Run TypeScript checks from the **repo root** (`/Users/mac/Work/victoria-vr-hack`): `bunx tsc --noEmit` and `bun test tests/gltf.test.ts tests/skin.test.ts tests/animation.test.ts tests/look.test.ts
tests/gltf-lod.test.ts`. Write the Status into **this file** at the end.

## Shared tree: hard rules (other engineers work in the same working tree right now)
- **NEVER run `git checkout`, `git clean`, `git stash`, `git restore`, `git reset`, or write a file from `git show` over a working file.** If a file you did not change fails, say so in the Status; do not revert it.
- **Files you own this round**: `src/engine/gltf.ts` (and new files beside it if it grows past 400 lines; keep the public API unchanged), `tests/gltf-lod.test.ts` (new), `tests/gltf.test.ts` only to add a test if needed.
- **Others are editing**: everything under `engine/crates/**`, `engine/cpp/**`, `engine/tools/mcp/**`. Do not touch them.

## Context
`poly.lod@1` (`engine/crates/ve-graph/src/builtin/poly_output.rs`) makes a simplified copy of a mesh: a program writes one node per level it wants (`lod0 = ...`, `lod1 = poly.lod@1(mesh, levels=2, level=1, ratio=0.5)`,
`lod2 = ... level=2`) and outputs each as a part (`output rock_lod0 = ...` style names). The exporter (`buildGlb` / `buildGlbWithTextures` in `src/engine/gltf.ts`) currently writes every part as an unrelated node, so a game
engine cannot pick a level by distance. Teach it the convention below.

## The convention
Parts whose names end in `_lod<N>` (N a whole number 0 to 7) and share the same base name form a **LOD group**. Exporting a group writes:
1. each level as its own node named `<base>_LOD<N>` with `extras: { lod: { base, level: N, levels: <count in the group>, triangles: <its triangle count> } }`;
2. an `MSFT_lod` extension on the level-0 node: `extensions.MSFT_lod = { ids: [<node index of level 1>, <level 2>, ...] }` (the standard way engines such as Babylon read LOD chains), and `MSFT_lod` listed in
   the file's `extensionsUsed`; the level-0 node's `extras.lod.screenCoverage` is an array with one number per level, from 1 down: level n gets `0.5 ** n` (a hint, documented in the code comment);
3. a group with a gap (levels 0 and 2 but not 1) or with duplicate levels is still written, level numbers preserved, but a warning string is added to `asset.extras.warnings` naming the group and the problem;
4. parts that are not part of any group export exactly as before (the whole existing test suite must still pass unchanged).
Only level-0 nodes (and ungrouped nodes) are scene roots? No: **all** level nodes stay in the scene (so a viewer that ignores `MSFT_lod` still shows something), but the lower levels carry `extras.lod.level` > 0 and the level-0
node is the first of the group in the node list. Skinned and animated parts keep working as before (a LOD group applies to ordinary mesh parts; for a skinned group, write the extension only if every level is skinned with the same
bones, otherwise write a warning).

## Tests: create `tests/gltf-lod.test.ts` (model it on `tests/gltf.test.ts`: `buildGlb`, `gltf-validator`, helpers for boxes). Fixed; do not weaken any assertion; if one fails, say which and why.
1. Three parts `rock_lod0`, `rock_lod1`, `rock_lod2` (boxes of 12, 12, 12 triangles are fine; make level n use a different size so you can tell them apart): the glTF has nodes named `rock_LOD0`, `rock_LOD1`, `rock_LOD2`, each
   with `extras.lod.level` equal to its number and `extras.lod.levels` equal to 3.
2. The level-0 node has `extensions.MSFT_lod.ids` equal to the indices of the `rock_LOD1` and `rock_LOD2` nodes (in that order), and `extensionsUsed` contains `MSFT_lod`.
3. `extras.lod.screenCoverage` on level 0 is `[1, 0.5, 0.25]`.
4. `extras.lod.triangles` on each level equals that part's real triangle count (use parts with different counts: 12, 8 (a pyramid-like mesh you build), 4 (a tetrahedron)).
5. A group with a gap (`bush_lod0`, `bush_lod2`) exports both nodes with their numbers preserved and `asset.extras.warnings` has one string mentioning `bush` and `missing`.
6. Duplicate levels (two parts named `tree_lod1`) export both and warn with a string mentioning `tree` and `twice` (or `duplicate`).
7. Ordinary parts (no `_lod` suffix, or a name like `lodge` or `lod_stand` that merely contains `lod`) export as before: no `MSFT_lod`, no `extras.lod`, and `extensionsUsed` has no `MSFT_lod`.
8. The file validates: `gltf-validator` reports 0 errors for test 1's file (warnings about the unknown extension are acceptable; print them in a comment if any).
9. Two separate groups (`rock_*` and `bush_*`) in one file do not interfere: each level-0 node lists only its own group's ids.

## Acceptance
`bunx tsc --noEmit` clean, the listed bun tests all pass (report the counts), and every file you wrote stays under 400 lines (the rules check is `cargo xtask rules` from `engine/`: run it).

## Status (Codex: fill in with real numbers for each of the nine tests, then stop)
- State: complete
- Per test (all nine passed):
  1. Names and metadata: `rock_LOD0`, `rock_LOD1`, `rock_LOD2`; levels `0, 1, 2`; group count `3` on each.
  2. Level-0 extension: ids `[1, 2]`; `extensionsUsed` contains `MSFT_lod`.
  3. Screen coverage: `[1, 0.5, 0.25]`.
  4. Measured triangles: `[12, 8, 4]`.
  5. Bush gap: nodes `bush_LOD0`, `bush_LOD2`; one warning contains `bush` and `missing`.
  6. Duplicate tree level: two nodes preserved; one warning contains `tree` and `duplicate`.
  7. Ordinary names: `ordinary`, `lodge`, `lod_stand`; no LOD metadata or extension.
  8. gltf-validator: `0` errors for the three-level file.
  9. Independent groups: rock ids `[1]`, bush ids `[3]`.
- Failing tests anywhere: none. Focused command passed `38` tests across 5 files (`178` expect calls). `bunx tsc --noEmit` passed; `cargo xtask rules` passed.
- What you tried when something blocked you: Initial run revealed the pyramid fixture measured 6 rather than the required 8 triangles, and a duplicate-only level-1 group got a spurious gap warning. Added two triangles to the fixture and restricted missing-level warnings to gaps between present levels; rerun passed.
