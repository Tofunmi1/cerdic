# Handoff to Codex: more Blender operators in the polygon-mesh pipeline

Read `engine/AGENTS.md` first and follow it (no `unwrap`/`expect`/`panic!`/`println!` in library code, no `#[allow]`: use `#[expect(lint, reason)]` on the
smallest item only, files under 400 lines in Rust (C++ follows the style of the neighbouring files), docs on public items, tests, no commits, no
`Co-Authored-By` or tool attribution). **Never run `cargo fmt`/`cargo fmt --all`; format only your Rust files with `rustfmt --edition 2024
--config-path rustfmt.toml <file>` from `engine`.** Do not run the browser build; build with `cargo check`/tests and a native C++ build only. Another
engineer works in `crates/ve-render`, `crates/ve-runtime`, `web/`: do not touch those.

## Context
Blender is vendored in `third_party/blender` (read `VENDOR.md` there: it states how files were fetched, the patches, the flags and the deterministic libm).
It is built by `cpp/blender/` (`sources.cmake` lists compiled files; `ops/` holds our code over BMesh, with bevel and UV unwrap already behind the C ABI in
`cpp/include/ve_kernel.h`, the Rust wrapper in `crates/ve-kernel`, and graph nodes in `crates/ve-graph/src/builtin` + `crates/ve-modeling`). Study how
`poly.bevel_network` and `poly.unwrap_uv` go from the graph node to Blender and copy that path exactly. `tools/blender-closure.py` computes include closures;
fetch missing upstream files by git sparse checkout at the pinned commit, byte-identical, mark our edits `VE patch`, update `VENDOR.md`. GPL is allowed (ADR
0024); Blender's bundled assets are not pulled.

## Build, in this order (each one finished and tested before the next)
1. **`poly.triangulate`** (`bmesh/operators/bmo_triangulate.cc`, beauty or ear clip, optional quad-to-tri method) and **`poly.remove_doubles`** (merge by distance,
   `bmo_removedoubles.cc`, parameter `distance` in metres, bounded).
2. **`poly.bisect`** (cut a mesh with a plane, keep one side or both, optional cap fill; `bmo_bisect_plane.cc`) and **`poly.mirror_bisect`** (bisect then
   mirror and weld on the plane, for symmetric models; reuses 1 and the existing `poly.symmetrize` code where sensible).
3. **`poly.spin`** (`bmo_spin.cc`: revolve a profile/edge loop around an axis with steps and angle, optional weld) and **`poly.dual`** if the closure is small.
4. **`poly.connect_verts`** and **`poly.loop_cut`** (edge ring subdivide; `bmo_connect*.cc`, `bmo_subdivide_edgering.cc`) taking a *semantic* selection already
   supported by `poly.select` (not raw indices).
5. **`poly.pack_uv`** (`geometry/intern/uv_pack.cc`, `uv_islands.cc`): pack islands with margin, rotation allowed or not, keep texel density option.
6. **`poly.fair`** (`geometry/intern/mesh_fair.cc`, Eigen): smooth a selected region with position/tangency/curvature continuity depth.

## For every operator
Follow the repo's operator recipe: a spec in `ve-graph/src/builtin/` with bounded, typed, unit-bearing parameters and ranges, a plan estimate (so a refusal
happens before the work), cancellation if the loop is long, typed diagnostics with a `suggestion`, a `cases.md` entry (find where the existing ones live), a
parity corpus line (native and wasm bit-identical: the corpus example is `cargo run -p ve-modeling --example corpus`) and tests with an **analytic check**
(triangle count, Euler characteristic, area, volume, symmetry error, island overlap area is zero, etc.). Never let a Blender assert or abort reach the user:
refuse with a typed error first. Report each new `.o` size contribution (brotli of the wasm delta if you can measure it cheaply, otherwise say not measured).

## Acceptance
`cargo test -p ve-kernel -p ve-modeling -p ve-graph` pass; native C++ build clean; `cargo clippy -p ve-kernel -p ve-modeling -- -D warnings` shows nothing new;
`cargo xtask rules` shows none of your files over 400 lines; the corpus output for existing programs is **unchanged** (save it first to
`/tmp/blender-ops-corpus-before.txt` and diff after).

## Status (Codex: fill in as you finish each item, then stop)
- State: not started
- Done (per item, with what was measured):
- Commands run and results:
- Blocked / could not verify:
