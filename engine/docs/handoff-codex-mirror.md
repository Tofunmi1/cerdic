# Handoff to Codex: ONE operator, `poly.mirror`, with the exact tests below

Read `engine/AGENTS.md` first and follow it (no `unwrap`/`expect`/`panic!`/`println!` in library code, no `#[allow]`, Rust files under 400 lines, tests, no commits, no
`Co-Authored-By` or tool attribution). **Never run `cargo fmt`/`cargo fmt --all`; format only your Rust files with `rustfmt --edition 2024 --config-path rustfmt.toml
<file>` from `engine`.** Do not build the browser bundle. No network. **This one operator only.** Cargo may wait on a build lock: wait and retry. **If something blocks you, read the vendored Blender source
for the real behaviour, try the obvious fix, and report exactly what you tried** (the last two operators needed that: Blender 5.3 differs from older documentation, and `third_party/blender/source/blender/bmesh/operators/bmo_dupe.cc`
and `bmesh/intern/bmesh_opdefines.cc` are the truth). Make your final message the Status below: also write it into the Status section of this file.

## The pattern to copy
`poly.bisect` (commit `2142dd7`) and `poly.spin` (commit `861043e`) are done: `git show <commit> --stat` lists the files, `git show <commit> -- <file>` shows how. Same recipe: `MeshOp::Mirror` in `ve-geometry` and `ve-kernel`
(twelve-number `params` carry the plane: `params[0..3]` point, `[3..6]` normal, `[6]` distance), `run_mirror` in `cpp/blender/ops/mesh_ops.cc`, the family in `poly_ops/blender.rs`, the spec in
`crates/ve-graph/src/builtin/poly_output.rs` and `ready.rs`, the estimate in `plan/estimate.rs` (mirror at most doubles the mesh).

## The operator
`poly.mirror@1(in)`: add the mirror image of the whole mesh across a plane, and weld the two halves where they meet. Parameters: `point` (vec3, metres, default [0,0,0]), `normal` (vec3, any length, default [1,0,0];
refuse a zero vector with a suggestion), `merge` (bool, default true: weld vertices of the original and the copy that land within `distance`), `distance` (metres, default 0.0001). Blender: the `mirror` operator in
`bmo_mirror.cc` (read its slots in `bmesh_opdefines.cc`: it takes a 4 x 4 matrix, an axis and a merge distance, so build the reflection across an arbitrary plane yourself, or rotate/translate the plane to a coordinate
axis, call it, and transform back; choose the simpler correct way and say which). A mirror of a mesh that touches the plane leaves coincident faces: do not try to remove them; document that a half should be left **open**
at the plane (`poly.bisect fill=false`) so the weld closes it.

## Tests: create `crates/ve-modeling/tests/g11_mirror.rs` (model it on `tests/g11_bisect.rs` and `g11_spin.rs`). Fixed; do not weaken any assertion; if one fails, say which and why.
1. An open half sphere mirrored closes into a sphere: `ball = solid.sphere@1(radius=0.5, segments=32)`, `tris = poly.from_solid@1(ball)`, `half = poly.bisect@1(tris, point=[0,0,0], normal=[0,1,0], keep="below", fill=false)`
   (check the real parameter names of `poly.bisect@1` in `crates/ve-graph/src/builtin/poly_output.rs`), `whole = poly.mirror@1(half, point=[0,0,0], normal=[0,1,0], merge=true)`, `solid = poly.to_solid@1(whole)`:
   `poly.to_solid` succeeds (closed manifold) and the volume is within 1% of the whole sphere's volume (evaluate `ball` alone for the reference).
2. A disjoint copy: `box = solid.box@1(size=[1,1,1])`, `far = xform.move@1(box, by=[2,0,0])`, `tris = poly.from_solid@1(far)`, `m = poly.mirror@1(tris, point=[0,0,0], normal=[1,0,0], merge=true)`: the result has volume 2.0 within 1e-6
   and its bounding box x extent is -2.5 to 2.5 within 1e-6.
3. The mirror across a tilted plane: the same far box mirrored across the plane with normal [1,1,0] through the origin: volume 2.0 within 1e-6, and the copy sits where reflection puts it: the bounding box of the result has
   y min within 1e-6 of -2.5 + 0.0 ... (compute the expected reflected box centre yourself: reflecting the point (2,0,0) across the plane x + y = 0 gives (0,-2,0); the copy's y range is -2.5 to -1.5).
4. `merge=false` on test 1's half: `poly.to_solid` refuses with a typed code starting with `poly.` and a suggestion that mentions `merge`.
5. A zero normal refuses with a suggestion that mentions `normal`.
6. Determinism: two evaluations of test 1 give bit-identical positions.
Also add one corpus fixture line for it (like bisect's and spin's).

## Acceptance
`cargo test -p ve-modeling --test g11_mirror` and `cargo test -p ve-kernel -p ve-geometry -p ve-modeling -p ve-graph` (report any failing test by name), `cargo clippy -p ve-modeling --tests` shows nothing new from your
files, `cargo xtask rules` shows your files under 400 lines.

## Status (Codex: fill in with real numbers for each of the six tests, then stop)
- State: implementation wired end to end; 5/6 fixed tests pass. One requested assertion remains failing.
- Per test:
  1. Pass. Mirrored open half-sphere converted to a solid with volume `0.5119024938664052 m^3`; reference sphere is the same evaluated volume, relative difference `0`.
  2. Pass. Disjoint mirrored boxes volume `2.0 m^3`; x bounds `[-2.5, 2.5] m`.
  3. Pass. Tilted-plane mirrored boxes volume `2.0 m^3`; y minimum `-2.5 m`.
  4. Fail. With `merge=false`, `poly.to_solid` accepts the coincident open seam and reports volume `0.5119024938664052 m^3` instead of refusing with a `merge` suggestion.
  5. Pass. Zero normal refuses with `poly.mirror_normal_zero`; suggestion is `choose a non-zero plane normal`.
  6. Pass. Both evaluations have exactly equal position arrays (bit-identical assertion).
- Failing tests anywhere: `unmerged_half_refuses_to_become_solid_with_merge_suggestion` in `g11_mirror.rs`; no other failing tests were reported by the package test run.
- What you tried when something blocked you: read Blender 5.3 `bmo_mirror.cc` and its slot declaration in `bmesh_opdefines.cc`. Blender's mirror wrapper weld check uses world-axis coordinates even though it accepts a transform-space matrix, so I replaced that route with Blender duplicate + an exact arbitrary-plane reflection transform and reversed the duplicate faces. This fixed both box volumes, the tilted-plane bounds, the welded sphere, and determinism. The remaining case is accepted by `poly.to_solid` because it treats the coincident but unwelded seam as closed; I left the mirror coordinates exact rather than introduce a gap when `merge=false`.
- Checks: focused test ran (5 passed, 1 failed); `cargo test -p ve-kernel -p ve-geometry -p ve-modeling -p ve-graph` ran and reported that same failing test. `cargo clippy -p ve-modeling --tests` completed without warnings from the new mirror test; existing warnings remain elsewhere. `cargo xtask rules` passed. `cargo xtask cpp` stopped on pre-existing clang-format violation in `cpp/src/field_tape.cpp:133`. `cargo xtask quick ve-kernel ve-geometry ve-graph ve-modeling` stopped on pre-existing workspace format diffs in `ve-measure/tests/query.rs`, `symmetry.rs`, and `uv.rs`. Browser bundle was not built; strict gate remains pending.
