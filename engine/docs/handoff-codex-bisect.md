# Handoff to Codex: ONE operator, `poly.bisect`, with the exact tests below

Read `engine/AGENTS.md` first and follow it (no `unwrap`/`expect`/`panic!`/`println!` in library code, no `#[allow]`, Rust files under 400 lines, tests, no commits, no
`Co-Authored-By` or tool attribution). **Never run `cargo fmt`/`cargo fmt --all`; format only your Rust files with `rustfmt --edition 2024 --config-path rustfmt.toml
<file>` from `engine`.** Do not build the browser bundle. No network. **Do this one operator only. Do not start another.** Cargo may wait on a build lock: wait, do not stop.

## What exists
Blender's `bmo_bisect_plane.cc` and `bmesh_bisect_plane.cc` are already vendored and compiled (`cpp/blender/sources.cmake` lines 177 and 221). `poly.triangulate` and
`poly.remove_doubles` are wired end to end: copy that path exactly. Read: `cpp/blender/ops/mesh_ops.cc` (`run_triangulate`, and how `ve_mesh_op_input` carries params),
`cpp/include/ve_kernel.h`, `crates/ve-kernel/src/mesh_ops*` , `crates/ve-geometry` (`MeshOp`, `blender_edit`), `crates/ve-modeling/src/poly_ops/blender.rs` (`handles`,
`ignores_selection`, `operator`) and the spec of `poly.triangulate@1` in `crates/ve-graph/src`. Put the new spec in `crates/ve-graph/src/builtin/poly_output.rs` and register
it in `ready.rs` (see how `poly.pack_uv@1` is listed).

## Correction after your first attempt (you stopped because the interface carried four numbers)
**That blocker is removed.** I widened `ve_mesh_op_input.params` from 4 to **12** doubles (`cpp/include/ve_kernel.h`, `ve-kernel-sys/src/polymesh.rs`, `ffi/polymesh.rs`); `mesh_ops.rs::edit_mesh` pads
the existing operators' four numbers with zeros, and all 12 `mesh_ops` tests still pass. For bisect, change `MeshOp::encode` (or add an `encode_wide`) so `Bisect` returns a 12-number array: `params[0..3]` point,
`params[3..6]` normal, `params[6]` distance; `flags` bit 1 = keep above removed (clear_outer or inner as the plane's side requires), bit 2 = keep below removed, bit 4 = fill. Do not stop for an interface
reason again: if something else blocks you, try the obvious fix, and report exactly what you tried and what failed.

## The operator
`poly.bisect@1(in)`: cut the whole mesh with a plane. Parameters: `point` (vec3, metres, default [0,0,0]), `normal` (vec3, any length, default [0,1,0]; refuse a zero vector
with a suggestion), `keep` (choice `both` | `above` | `below`, default `both`; above means the side the normal points to), `fill` (bool, default true: close the cut with a face when a
side is removed), `distance` (number, metres, default 0.0001: how near the plane a vertex must be to count as on it). Use `BMO_op_callf(... "bisect_plane geom=%avef
dist=%f plane_co=%v plane_no=%v use_snap_center=%b clear_inner=%b clear_outer=%b" ...)` (check the real slot names in
`third_party/blender/source/blender/bmesh/intern/bmesh_opdefines.cc`; Blender 5.3 differs from older docs, which is why `triangulate` broke before) and, when `fill` is true and a side was
removed, close the open boundary the cut leaves (try `edgenet_prepare`/`edgenet_fill` or `holes_fill` as the `bisect` tool does; if filling is not possible say so, and make `fill=true`
refuse with a typed error and suggestion instead of returning an open mesh).
A selection is not used (`ignores_selection`).

## Tests: create `crates/ve-modeling/tests/g11_bisect.rs` (model it on `g11_round_edges.rs`). Every assertion below is fixed; do not weaken one to make it pass. If one fails, say which.
Fixture: `box = solid.box@1(size=[1, 1, 1])` (it spans -0.5..0.5 on each axis), `tris = poly.from_solid@1(box)`, `mesh = poly.merge_coplanar@1(tris)`, then
`cut = poly.bisect@1(mesh, point=[0, 0.2, 0], normal=[0, 1, 0], keep=KEEP)`, `solid = poly.to_solid@1(cut)`, `output solid`.
1. `keep="below"`, `fill=true`: volume is 0.7 within 1e-6, bounding box y max is 0.2 within 1e-6, and `poly.to_solid` succeeds (closed).
2. `keep="above"`, `fill=true`: volume is 0.3 within 1e-6, bounding box y min is 0.2 within 1e-6.
3. `keep="both"`, `fill=true` (nothing removed): volume is 1.0 within 1e-6 and the triangle count is greater than the uncut box's.
4. A tilted plane, normal=[1, 1, 0], point=[0,0,0], `keep="below"`: volume is 0.5 within 1e-6 (a plane through the centre halves the box).
5. A plane that misses the mesh (point=[0, 5, 0], `keep="below"`): result volume is 1.0 within 1e-6 (nothing cut, nothing lost).
6. A plane that removes everything (point=[0, 5, 0], `keep="above"`): refused with a typed code and a non-empty suggestion (not a panic, not an empty mesh).
7. A zero normal refuses with a suggestion that mentions `normal`.
8. Determinism: the same program twice gives bit-identical positions.
9. On a UV-sphere solid (`solid.sphere@1(radius=0.5, segments=32)`) cut at y=0 keeping below: volume within 1% of half the sphere's volume.

Also add one corpus fixture line for it the way `G10-07` and `G10-08` are done, and confirm the existing corpus lines are unchanged: compare with `/tmp/blender-ops-corpus-before.txt`
(only your new line may differ; if other lines differ because of other engineers' work, list them).

## Acceptance
`cargo test -p ve-modeling --test g11_bisect` and `cargo test -p ve-kernel -p ve-geometry -p ve-modeling -p ve-graph` (report any failing test by name), `cargo clippy -p ve-modeling --tests`
shows nothing new from your files, `cargo xtask rules` shows your files under 400 lines.

## Status (Codex: fill in with real numbers for each of the nine tests, then stop)
- State: implemented; all nine bisect tests pass.
- Per test, measured values:
  1. `below`, fill: pass; volume `0.699999988079071`, y max `0.19999999`; `poly.to_solid` succeeded.
  2. `above`, fill: pass; volume `0.30000001192092896`, y min `0.19999999`; `poly.to_solid` succeeded.
  3. `both`: pass; volume `1.0`, 20 triangles versus 12 uncut.
  4. Tilted plane: pass; volume `0.5`.
  5. Missed plane: pass; volume `1.0`.
  6. Removes everything: pass; code `poly.bisect_empty`, suggestion `move the plane or choose the other side to keep`.
  7. Zero normal: pass; code `poly.bisect_normal_zero`, suggestion `choose a non-zero plane normal`.
  8. Determinism: pass; 8 positions, bit-identical across runs.
  9. UV sphere: pass; source volume `0.5119024938664052`, measured half `0.2559512469332026`, cut volume `0.25595124693320265` (relative difference about `2.2e-16`).
- Failing tests anywhere: none in `cargo test -p ve-modeling --test g11_bisect` or `cargo test -p ve-kernel -p ve-geometry -p ve-modeling -p ve-graph`.
- Corpus: `/tmp/blender-ops-corpus-before.txt` has 100 lines; current output has 102. All 100 historical lines are byte-identical. The two added outputs are the requested bisect result (`n_c2zd6268`, volume `0.699999988079071`) and an additional emitted part (`n_38z1x16x`, volume `1.0`); no existing line changed.
- Clippy: `cargo clippy -p ve-modeling --tests` completed. It reports existing warnings in other files (including existing shared poly/UV and modeling code), but no warning from `g11_bisect.rs`, the bisect additions in `poly_ops/blender.rs`, or the new graph spec after splitting it into its own function.
- Rules and formatting: `cargo xtask rules` passed; Rust changes were formatted only with targeted `rustfmt`. `cargo xtask quick ve-kernel ve-geometry ve-graph ve-modeling` stopped at its workspace format check on existing differences in `crates/ve-measure/tests/query.rs`, `symmetry.rs`, and `uv.rs`; no workspace formatter was run.
- C++: `cargo xtask cpp` stops at format because untouched `cpp/src/field_tape.cpp:133` has a pre-existing clang-format violation. The touched `mesh_ops.cc` and `ve_kernel.h` were formatted; retry still reports only `field_tape.cpp`.
- Obstacle and fix tried: the first bisect run SIGSEGVed because an extra vararg was passed after `geom=%avef`; Blender's format already encodes all vertex/edge/face geometry, so that argument shifted every later parameter. Removed the extra vararg, then changed fill to pass the bisect operator's `geom_cut.out` directly to `holes_fill`. The targeted nine tests then passed.
- Anything not done: browser checks and the strict gate remain pending per the engine rules; no browser bundle or network was used, and no commit was made.
