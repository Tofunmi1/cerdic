# Handoff to Codex: ONE operator, `poly.loop_cut`, with the exact tests below

Read `engine/AGENTS.md` first and follow it (no `unwrap`/`expect`/`panic!`/`println!` in library code, no `#[allow]`, Rust files under 400 lines, tests, no commits, no `Co-Authored-By` or tool attribution).
**Never run `cargo fmt`/`cargo fmt --all`; format only your Rust files with `rustfmt --edition 2024 --config-path rustfmt.toml <file>` from `engine`.** Do not build the browser bundle. No network. **This one operator only.**
Cargo may wait on a build lock: wait and retry. If something blocks you, read the vendored Blender source (`third_party/blender/source/blender/bmesh/...`: Blender 5.3 differs from old documentation), try the obvious
fix, and report exactly what you tried. Write the Status into **this file** at the end.

## Shared tree: hard rules (two other engineers work in the same working tree right now)
- **NEVER run `git checkout`, `git clean`, `git stash`, `git restore`, `git reset`, or write a file from `git show` over a working file.** If a file you did not change fails to compile, say so in the Status; do not "fix" it by
  reverting it. Another engineer's half-finished edit can break the build for a minute: wait and retry.
- **Files you own this round** (edit only these): `cpp/blender/**`, `cpp/include/ve_kernel.h`, `crates/ve-kernel/**`, `crates/ve-kernel-sys/**`, `crates/ve-geometry/**`, `crates/ve-modeling/src/poly_ops/blender.rs`,
  `crates/ve-modeling/src/plan/estimate.rs`, `crates/ve-graph/src/builtin/poly_output.rs`, `crates/ve-graph/src/builtin/ready.rs` (one line), `crates/ve-modeling/examples/corpus/` (one fixture line), and the new test.
- **Others are editing**: `crates/ve-modeling/src/instances/**`, `crates/ve-graph/src/builtin/instances.rs` (a helper), and `crates/ve-graph/src/spec.rs`, `value.rs`, `links.rs`, `crates/ve-modeling/src/cache.rs`,
  `evaluate.rs` (the manager). Do not touch them.

## The pattern to copy
`poly.bisect` (commit `2142dd7`), `poly.spin` (`861043e`) and `poly.mirror` (`6a536af`) are done: `git show <commit> --stat` lists the files and `git show <commit> -- <file>` shows how (read-only commands are fine).
Same recipe: `MeshOp::LoopCut` in `ve-geometry` and `ve-kernel` (twelve-number `params`), `run_loop_cut` in `cpp/blender/ops/mesh_ops.cc`, the family in `poly_ops/blender.rs`, the spec in `builtin/poly_output.rs` and
`ready.rs`, the estimate in `plan/estimate.rs` (a loop cut adds at most `cuts` times the ring length to the mesh).

## The operator
`poly.loop_cut@1(in)`: cut a ring of quad faces with new edges, like Blender's Loop Cut. Parameters: `at` (vec3, metres: the edge of the mesh nearest to this point picks the ring; refuse a mesh with no edge within the
mesh's bounding diagonal with a suggestion), `cuts` (int 1..32, default 1: how many parallel cuts), `smooth` (0..1, default 0, passed to the subdivide as its smoothness). Algorithm: find the edge whose midpoint is nearest `at`,
walk the **edge ring** from it with Blender's ring walker (`BMW_EDGERING` in `bmesh/intern/bmesh_walkers*.cc`; check it is in the vendored closure and in `cpp/blender/sources.cmake`; if a file is missing, say exactly which and
stop that part, no network), select the ring's edges, and call `subdivide_edges` on them (`edges=%he cuts=%i smooth=%f use_grid_fill=%b` or the real slot names: read `bmesh_opdefines.cc`). The ring walks through quads only:
a ring that meets a triangle or an n-gon stops there, and if it does not return to its start the cut is refused with a typed code and a suggestion ("the ring must run through quads only; merge triangles with poly.merge_coplanar first").

## Tests: create `crates/ve-modeling/tests/g11_loopcut.rs` (model it on `tests/g11_mirror.rs` and `g11_spin.rs`). Fixed; do not weaken any assertion; if one fails, say which and why.
Fixture: `box = solid.box@1(size=[1,1,1])`, `tris = poly.from_solid@1(box)`, `mesh = poly.merge_coplanar@1(tris)` (6 quad faces, 8 points). Read a poly output's point count with `output <poly node>` and
`parts[0].positions.len()` (see `g11_mirror.rs`).
1. One cut around the middle: `cut = poly.loop_cut@1(mesh, at=[0.5, 0, 0.5], cuts=1)` (the nearest edge is a vertical edge, so the ring runs around the four side faces): the point count goes from 8 to **12** (four new points).
2. Three cuts: `cuts=3` gives 8 + 4*3 = **20** points.
3. The shape is unchanged: after `poly.to_solid` the volume is 1.0 within 1e-6 and the bounding box is unchanged, for `cuts=1` and `cuts=3`.
4. The cuts are evenly spaced: with `cuts=3` the new points have y coordinates -0.25, 0, 0.25 (within 1e-6): collect the distinct y values of the output points and compare with [-0.5, -0.25, 0, 0.25, 0.5].
5. The ring is chosen by `at`: `at=[0.5, 0.5, 0]` (a top edge, so the ring runs around the other four faces, about the x axis... whichever way the ring of that edge goes) still gives 12 points and the distinct x or z values of the new points
   are 0 on the axis the ring crosses; assert the point count and that exactly four new points exist at a coordinate of 0 on one axis.
6. A ring through a triangle refuses: use `tris` (unmerged, triangles): `poly.loop_cut@1(tris, at=[0.5, 0, 0.5])` refuses with a typed code starting `poly.` and a suggestion that mentions `poly.merge_coplanar`.
7. `cuts=0` and `cuts=33` are refused by the parameter range (typed, with the allowed range).
8. Determinism: two evaluations of test 1 give bit-identical positions.
Also add one corpus fixture line for it (like the others'), and confirm the existing corpus lines are unchanged (the corpus example is `cargo run -q -p ve-modeling --example corpus`; save its output before you start, to a file under your own directory).

## Acceptance
`cargo test -p ve-modeling --test g11_loopcut` and `cargo test -p ve-kernel -p ve-geometry -p ve-modeling -p ve-graph` (report any failing test by name), `cargo clippy -p ve-modeling --tests` shows nothing new from your files,
`cargo xtask rules` shows your files under 400 lines.

## Status (Codex: fill in with real numbers for each of the eight tests, then stop)
- State: implemented; all eight fixed tests pass.
- Per test:
  1. Pass — 12 points after one cut (8 input + 4 new).
  2. Pass — 20 points after three cuts (8 input + 12 new).
  3. Pass — `poly.to_solid` volume measured 1.0 for both; bounds on x, y, z each measured [-0.5, 0.5].
  4. Pass — distinct y coordinates measured [-0.5, -0.25, 0, 0.25, 0.5].
  5. Pass — 12 points; exactly four new points lie at z=0 (the ring crosses the z=0 plane).
  6. Pass — typed refusal `poly.loop_cut_non_quad`; suggestion contains `poly.merge_coplanar`.
  7. Pass — both 0 and 33 return typed `param.range`; diagnostic allows 1 to 32.
  8. Pass — repeated one-cut evaluations have bit-identical positions.
- Failing tests anywhere: none in the final focused test or `cargo test -p ve-kernel -p ve-geometry -p ve-modeling -p ve-graph` run. `cargo clippy -p ve-modeling --tests` fails on concurrent `g11_values.rs` using `eprintln!`; the focused `cargo clippy -p ve-modeling --test g11_loopcut` completed. `cargo xtask quick ve-kernel ve-geometry ve-modeling ve-graph` stopped at unrelated formatting diffs in other engineers' files. `cargo xtask cpp` stopped at a clang-format violation in unrelated `cpp/src/field_tape.cpp:133`.
- Corpus: 105 output lines with this fixture, versus 104 without it; line comparison found all 104 existing output lines unchanged and one added output line.
- Gates: `cargo xtask rules` passed; native C++ build and Rust package tests passed. Strict gate remains pending.
- What you tried when something blocked: read Blender's `bmesh_walkers.hh`, `bmesh_walkers_impl.cc`, `bmo_subdivide_edgering.cc`, and `bmesh_opdefines.cc`; confirmed both walker source files are in `cpp/blender/sources.cmake`. Fixed an initial `LoadedMesh` edge-iteration compile error by using `BM_ITER_MESH`. The first operator attempt used `BMO_op_initf` with the wrong edge-buffer argument and refused; switched to the established `BMO_op_callf` selected-edge pattern and selected ring endpoint vertices too. Removed a false closure test based on shared endpoints because Blender's edge-ring edges are parallel and do not share endpoints. Left all concurrent engineers' unrelated files untouched.
