# Handoff to Codex: production output (glTF, UV packing, LOD)

Read `engine/AGENTS.md` first and follow it (no `unwrap`/`expect`/`panic!`/`println!` in library code, no `#[allow]`: use `#[expect(lint, reason)]` on the
smallest item only, Rust files under 400 lines, docs on public items, tests, no commits, no `Co-Authored-By` or tool attribution). **Never run
`cargo fmt`/`cargo fmt --all`; format only your Rust files with `rustfmt --edition 2024 --config-path rustfmt.toml <file>` from `engine`.** Do not build the
browser bundle (the owner's other engineer does); use `cargo check`, Rust tests, `bun test`/`bunx tsc --noEmit` and native C++ builds.

## Ownership (other engineers work at the same time)
- You own: `src/engine/gltf.ts` (+ new tests next to it), new files `crates/ve-graph/src/builtin/poly_output.rs` and `crates/ve-modeling/src/poly_ops/output*.rs`,
  `cpp/blender/ops/uv_pack*` and the matching lines in `cpp/blender/sources.cmake`, `cpp/include/ve_kernel.h`, `crates/ve-kernel*`.
- **Do not edit** `crates/ve-graph/src/builtin/poly.rs`, `crates/ve-modeling/src/creature/**`, `crates/ve-measure/**`, `ve-render`, `ve-runtime`, `web/`.
  Register new operators through a single line in `builtin.rs`. A previous Codex run was stopped early and may have left half-finished Blender edits in
  `cpp/blender/` or `ve-kernel`: look at `git diff` there, finish or revert only what is clearly an unfinished triangulate/remove-doubles attempt, and say what you did.

## Why
Models must be usable outside our viewer and cheap in a game. Today: glTF export exists but is unaudited; UV unwrap exists but not packing; no LOD chain.

## Corrections after the first attempt
- This run's writable root is the **repo root** (`/Users/mac/Work/victoria-vr-hack`), so `src/engine/gltf.ts` and `tests/` at the repo root are writable. Run Rust
  commands from `engine/` and bun commands from the repo root. Paths in this file that say `src/engine/...` mean the repo-root `src/engine/...`.
- The first attempt found the glTF gaps: indices always u32 (use u16 when small), no vertex colours or occlusion map, skin without inverse bind matrices, scene extras
  without units, clips without loop info. Fix those.
- **There is no `uv_islands.cc` in this Blender version (5.3); my earlier handoff was wrong.** `source/blender/geometry/intern/uv_pack.cc` and
  `GEO_uv_pack.hh` are already vendored and already listed in `cpp/blender/sources.cmake` (line 233), so **no network and no fetching is needed**. Your sandbox has
  no network anyway. Task 2 is only to wire `uv_pack.cc` through `cpp/blender/ops` (read how `ops/` calls `uv_parametrizer` for unwrap and copy it), the C ABI, the
  Rust wrapper and the node. If a symbol it needs is missing, add it to `cpp/blender/unresolved.txt` only if it is truly unreachable, otherwise a shim.
- Task 1 is done enough (27 tests pass); skip its remaining items. **Start at task 2.**

## Tasks, in order (tasks 1 to 3, then 4 if you have time)
1. **glTF audit and hardening** (`src/engine/gltf.ts`). Export must be valid glTF 2.0 and carry: positions, normals, UV0, MikkTSpace tangents when a normal map
   exists, vertex colours if present, indices (u16 when small), PBR metallic-roughness materials with our baked textures embedded (base colour, MR, normal,
   occlusion, emissive), `KHR_materials_emissive_strength` where needed, skins (joints, weights, inverse bind matrices), animation clips (all of our clip names,
   loop info in `extras`), node hierarchy and names, scene extras with our units (metres), and `KHR_mesh_quantization` is **not** used (compatibility).
   Validate with the Khronos `gltf-validator` (install into the scratchpad or a temp dir, never into the repo's lockfile; if the network is unavailable,
   write strict structural checks and say so). Fix every error and warning in the exporter; test with a character (`make_character`) and a textured solid.
   Add bun tests that export, parse the JSON/binary chunks and check accessors' counts, bounds (`min`/`max`), byte alignment and index ranges.
2. **`poly.pack_uv`**: Blender `geometry/intern/uv_pack.cc` + `uv_islands.cc` (fetch the include closure with `tools/blender-closure.py` at the pinned commit,
   mark edits `VE patch`, update `third_party/blender/VENDOR.md`; read how `poly.unwrap_uv` is wired and copy it). Parameters: `margin` (fraction), `rotate`
   (bool), `keep_density` (bool), `shape` (`aabb`|`convex`). Output: the same mesh with new UVs, all inside 0..1, no island overlap. Analytic tests: overlap area is 0,
   all UVs in range, coverage above 0.5 for a cube unwrap, determinism, native == wasm hash (use the existing corpus line mechanism).
3. **`poly.lod`**: build a chain of simplified meshes from one `Poly`/`Solid` using the existing `poly.decimate` code path: parameters `levels` (1..4),
   `ratio` (per level, 0.05..0.9), `protect_uv_seams`, `protect_boundary`. Output a node that yields the chain; the glTF exporter writes it as separate
   nodes `<name>_LOD0..N` with `extras.lod` (screen-size hints) so engines can pick. Tests: triangle counts follow the ratio within 10%, bounding box
   drift under 2% of size, still watertight if the input was, UV seams preserved when asked, deterministic.

4. **More Blender operators**, as specified in `docs/handoff-codex-blender-ops.md` (triangulate and remove doubles first, then bisect and mirror-bisect, spin,
   connect verts and loop cut, fair). Same ownership rules as above; register through `poly_output.rs`.

## For every operator
Follow the repo recipe: spec with bounded typed parameters and units, plan estimate, cancellation if long, typed diagnostics with a `suggestion`, a `cases.md`
entry (find where the existing ones are), a parity corpus line, tests with analytic checks. Never let a Blender assert or abort reach the user.

## Acceptance
`cargo test -p ve-kernel -p ve-modeling -p ve-graph`, `bunx tsc --noEmit` and the new bun tests pass; native C++ build clean; `cargo clippy -p ve-kernel
-p ve-modeling -- -D warnings` shows nothing new; `cargo xtask rules` shows none of your files over 400 lines; the corpus output for existing programs is
unchanged (compare with `/tmp/blender-ops-corpus-before.txt`).

## Status (Codex: fill in per task as you finish, then stop)
- State: Task 1 was already complete enough per the correction. Task 2 has a first implementation and focused kernel coverage; tasks 3 and 4 remain incomplete.
- Done (per task, with what was measured):
  - Task 1: already complete enough per the correction; no Task 1 edits made during this run.
  - Task 2 (partial implementation): added the C ABI, Rust kernel entry point, geometry wrapper, `poly.pack_uv` graph parameters and modeling dispatch over already vendored `uv_pack.cc`. Existing UV discontinuities define islands; non-finite UVs are rejected at the C ABI. Added deterministic/unit-square UV test and `G10-08` case. `cargo test -p ve-kernel --test uv` — 4 passed. `cargo check -p ve-graph -p ve-modeling` — passed after correcting a duplicate match arm.
  - Task 4 inspection: the shared tree already contains Blender `poly.triangulate` and `poly.remove_doubles` implementations and tests (`G10-07`); no changes made to those shared edits.
- Commands run and results:
  - Rust files formatted individually with `rustfmt --edition 2024 --config-path rustfmt.toml`; no workspace formatter run.
  - `cargo xtask quick --help` — failed in the workspace `cargo fmt --all --check` stage because numerous unrelated shared files are not rustfmt-clean. No formatter was run.
  - `cargo xtask rules` — failed on three overlength files: `crates/ve-measure/src/collision.rs` (451), `crates/ve-graph/src/builtin/poly_network.rs` (407), and `crates/ve-runtime/src/lib.rs` (411). The UV node was moved into the owned `builtin/poly_output.rs`; the remaining `poly_network.rs` overage predates that extraction.
- Blocked / could not verify:
  - Task 2 still needs analytic island-overlap and coverage tests, corpus parity, `cargo xtask rules`, and clippy. It has only the focused range/determinism test so far.
  - Task 3 (`poly.lod`) was not implemented.
  - Task 4 bisect, mirror-bisect, spin, dual, connect-verts, loop-cut, pack-UV parity, and fair operators remain unimplemented here; only the already-present triangulate/remove-doubles work was inspected. The task is therefore incomplete.
  - Strict gate remains pending. No browser bundle was built; no network or fetch was used; no commit was created.
