# Handoff to Codex: ONE operator, `poly.spin`, with the exact tests below

Read `engine/AGENTS.md` first and follow it (no `unwrap`/`expect`/`panic!`/`println!` in library code, no `#[allow]`, Rust files under 400 lines, tests, no commits, no
`Co-Authored-By` or tool attribution). **Never run `cargo fmt`/`cargo fmt --all`; format only your Rust files with `rustfmt --edition 2024 --config-path rustfmt.toml
<file>` from `engine`.** Do not build the browser bundle. No network. **This one operator only.** Cargo may wait on a build lock: wait and retry. **If something blocks you, try the obvious fix and report
exactly what you tried; do not stop at the first obstacle** (last round you stopped on a four-number interface limit that a three-line change fixed).

## The pattern to copy
`poly.bisect` has just landed (commit `2142dd7`): `git show 2142dd7 --stat` lists every file it touched, and `git show 2142dd7 -- <file>` shows how. It used the twelve-number `params` of `ve_mesh_op_input`
(point and normal in `params[0..6]`, distance in `params[6]`, bits in `flags`). Do the same for spin: the spec goes in `crates/ve-graph/src/builtin/poly_output.rs` and is listed in `ready.rs`; the run path is
`poly_ops/blender.rs` (add the family to `handles`, `operator`, and to `selection_optional` since a spin without a selection spins the whole mesh); the C++ is `run_spin` in `cpp/blender/ops/mesh_ops.cc`;
`MeshOp::Spin` in `ve-geometry` and `ve-kernel`; the plan estimate in `crates/ve-modeling/src/plan/estimate.rs` (`poly.spin` grows the mesh by about `steps` times the input: add it to the growth list with
that factor, bounded by the existing triangle budget).

## Where the first run stopped (diagnosed by the manager; the process was killed mid-work, not by your choice)
The wiring compiles and `cargo test -p ve-modeling --test g11_spin` runs, but 6 of 8 tests fail with `kernel.invalid_input: the mesh is not a closed manifold`. The full-turn result has 640 positions and 1784
triangles for 64 steps of a 10-vertex strip: **the seam is not merged** (the last copy of the strip sits on top of the first). Blender's `spin` operator does not weld by itself; Blender's own editor operator
(`edbm_spin_exec` in `source/blender/editors/mesh/editmesh_tools.cc`, not vendored: read it from memory of the design) runs `remove_doubles verts=<the spin's geom_last output> dist=<distance>` afterwards when
Merge is on. Do the same: after `spin`, when `merge` is true call `remove_doubles` on the last-copy geometry (the slot is `geom_last.out`; check the real slot names in `bmesh_opdefines.cc`), or on all
vertices within `distance`. If the original strip faces survive inside the ring after the weld (an internal double face makes the solid non-manifold), delete them (`delete geom=%hf context=FACES`).
Keep your wiring: do not redo it. Run `cargo test -p ve-modeling --test g11_spin` after each change and show me the failing/passing list in the Status.

## The operator
`poly.spin@1(in, [selection])`: revolve the selected faces or edges (or the whole mesh) around an axis, like a lathe or Blender's Spin. Parameters: `center` (vec3, metres, default [0,0,0]), `axis` (vec3, any length,
default [0,1,0]; refuse a zero vector with a suggestion), `angle` (radians, 0.001..2π, default 2π), `steps` (int 2..256, default 32), `merge` (bool, default true: weld the first and last copy when the angle is a
full turn, and merge vertices that land within `distance`), `distance` (metres, default 0.0001). Blender call: `BMO_op_callf(bm, BMO_FLAG_DEFAULTS, "spin geom=%hef cent=%v axis=%v dvec=%v angle=%f steps=%i
use_merge=%b use_normal_flip=%b use_duplicate=%b", ...)` (check the real slot names in `third_party/blender/source/blender/bmesh/intern/bmesh_opdefines.cc`; Blender 5.3 differs from old docs). With
`dvec` zero and `use_duplicate` false the selected geometry itself is revolved.

## Tests: create `crates/ve-modeling/tests/g11_spin.rs` (model it on `tests/g11_bisect.rs`). Fixed; do not weaken any assertion; if one fails, say which and why.
Fixture: a flat strip from `poly.grid@1(width=0.2, height=1.0, columns=1, rows=4)` (it lies in the z=0 plane, centred on x=0, y from 0 to 1). Spin it around an axis that does not touch it:
`spun = poly.spin@1(strip, center=[-1, 0, 0], axis=[0, 1, 0], angle=6.283185307179586, steps=64, merge=true)`, then `solid = poly.to_solid@1(spun)`, `output solid`.
The strip is then 0.9 to 1.1 m from the axis, 1 m tall: a ring.
1. Full turn, steps=64: `poly.to_solid` succeeds (closed) and the volume is within 0.5% of `0.5 * 64 * sin(2*pi/64) * (1.1^2 - 0.9^2) * 1.0` (the exact polygonal ring; about 1.2549).
2. The bounding box x extent is within 0.5% of 2.2 and z extent within 0.5% of 2.2 (the ring's outer diameter, centred at x = -1: x from -2.1 to 0.1).
3. steps=16 gives a smaller volume than steps=64 (the polygon is coarser), within 0.5% of the same formula with n=16.
4. A half turn (`angle=3.141592653589793`, steps=32, merge=true) is **open**: `poly.to_solid` refuses with a typed code and a suggestion that mentions `angle` or `merge` (not a panic, not a silent open mesh).
5. `merge=false` on a full turn: the result is not closed (or has duplicate seam vertices): `poly.to_solid` refuses with a typed code and a suggestion mentioning `merge`.
6. A zero axis refuses with a suggestion that mentions `axis`.
7. Determinism: two evaluations of test 1 give bit-identical positions.
8. A selected-edges spin: build the same strip, `edges = poly.select@1(strip, element="edge", by="box", box_min=[-0.2, -0.01, -0.01], box_max=[0.2, 0.01, 0.01])` (the bottom edge, y = 0) and spin it a full turn around the same axis: the result is a flat washer, an open surface with area within 0.5% of `0.5 * 64 * sin(2*pi/64) * (1.1^2 - 0.9^2)`; assert that through a successful evaluation that is **not** converted to a solid (output the poly via `poly.triangulate` then `poly.to_solid` must refuse), and the point count is greater than the strip's.
Also add one corpus fixture line for it (like `G10-07`, `G10-08`, and bisect's), and confirm the existing corpus lines are unchanged: compare with `/tmp/blender-ops-corpus-before.txt`.

## Acceptance
`cargo test -p ve-modeling --test g11_spin` and `cargo test -p ve-kernel -p ve-geometry -p ve-modeling -p ve-graph` (report any failing test by name), `cargo clippy -p ve-modeling --tests` shows nothing new
from your files, `cargo xtask rules` shows your files under 400 lines.

## Status (Codex: fill in with real numbers for each of the eight tests, then stop)
- State: not started
- Per test: pass/fail and the measured value:
- Failing tests anywhere:
- What you tried when something blocked you:
