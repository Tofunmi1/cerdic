# Handoff to DeepSeek: `solid.hull` and `solid.collision_boxes`, graph nodes built on `ve-measure`

Read `engine/AGENTS.md` first and follow it (no `unwrap`/`expect`/`panic!`/`println!` in library code, no `#[allow]`, Rust files under 400 lines, docs on public items, tests, no commits, no
`Co-Authored-By` or tool attribution). **Never run `cargo fmt`/`cargo fmt --all`; format only your Rust files with `rustfmt --edition 2024 --config-path rustfmt.toml <file>` from `engine`.** Use the edit
tools. Do not build the browser bundle. Another engineer works in `cpp/`, `crates/ve-kernel*`, `crates/ve-geometry`, `poly_ops/blender.rs` and `builtin/poly_output.rs` at the same time: do not touch those.

## Context
`crates/ve-measure` (yours, verified) has `convex_hull(points)`, `bounding_shapes(mesh)` (box, oriented box, capsule with fit error) and `convex_decomposition`. They are not reachable from the graph, so a model
cannot ask for a collision shape. Add two nodes.

## You may edit
New files `crates/ve-graph/src/builtin/hulls.rs` (specs) and `crates/ve-modeling/src/hulls.rs` (run), one `mod` and one registration line each in `crates/ve-graph/src/builtin.rs` and
`crates/ve-modeling/src/lib.rs`, the dispatch where solid operators are evaluated (`crates/ve-modeling/src/operations.rs`, find how `solid.round` is dispatched, and add the two families there or in your
own module hooked the same way), `crates/ve-modeling/src/plan/estimate.rs` (add both to the estimate with the hull's triangle bound), `crates/ve-graph/src/builtin/ready.rs` (list them),
`crates/ve-modeling/Cargo.toml` (depend on `ve-measure`), a new test `crates/ve-modeling/tests/g11_hulls.rs`, and the Status below. Do not touch `ve-measure` unless a test proves a bug (say what).

## The nodes
1. `solid.hull@1(in)`: the convex hull of a solid, as a solid (a closed mesh the kernel accepts; build it the way `poly.to_solid` builds a solid from a closed mesh, find that path in `poly_ops/convert.rs`).
   No parameters except `max_vertices` (int 4..2000, default 256: refuse with a suggestion when the input has more points than ve-measure's bound allows, saying to decimate first).
2. `solid.collision_boxes@1(in)`: replaces a solid by the best-fitting simple shape from `bounding_shapes`, as a solid: parameter `shape` (choice `best` | `box` | `oriented_box` | `capsule`, default `best`).
   The node's diagnostics (as a warning, not an error) report the fit error, "volume ratio 1.4".

## Tests (`g11_hulls.rs`; model them on `tests/g11_round_edges.rs`). Fixed; do not weaken any assertion; if one fails, report it.
1. Hull of a box `solid.box@1(size=[1,2,3])`: volume 6.0 within 1e-6, bounding box unchanged.
2. Hull of an L shape (union of `solid.box@1(size=[2,1,1])` and `xform.move@1(solid.box@1(size=[1,2,1]), by=[-0.5, 0.5, 0])`, written with intermediate nodes because the language has no nested calls):
   the hull volume is greater than the L's volume and at most the bounding box's volume (print all three).
3. Hull of a sphere `solid.sphere@1(radius=0.5, segments=32)`: volume within 0.5% of the sphere's own volume (a sphere is convex).
4. Hull is idempotent: hull of the hull has the same volume to 1e-9.
5. `solid.collision_boxes shape="box"` on a box rotated 30 degrees about y (`xform.rotate@1`, find its angle parameter name and unit in the spec): the result is an axis-aligned box whose volume is
   greater than the original's.
6. `shape="oriented_box"` on the same rotated box: the volume equals the original box's within 1%.
7. `shape="best"` on a thin long cylinder (`solid.cylinder@1`): the chosen shape's volume ratio to the original is at most 1.5 (print it).
8. A solid with more points than `max_vertices` allows refuses with a typed code and a suggestion that names `max_vertices` or decimation.
9. Determinism: two evaluations give bit-identical positions.

## Acceptance
`cargo test -p ve-modeling --test g11_hulls`, `cargo test -p ve-modeling -p ve-graph -p ve-measure` (report any failing test by name), `cargo clippy -p ve-modeling --tests` shows nothing new from your files,
`cargo xtask rules`.

## Status (DeepSeek: fill in with real numbers for each of the nine tests, then stop)
- State: complete. `solid.hull@1` and `solid.collision_boxes@1` are specified, registered, estimated, dispatched
  and tested; 9/9 tests pass. Nothing was committed by me. (Commit `861043e`, made by another party, happens to
  contain this work along with `poly.spin`; one rustfmt-only edit to `src/hulls.rs` was made after it and is
  uncommitted.)
- Per test: pass/fail and the measured value:
  1. PASS `one_hull_of_a_box_is_that_box`: hull volume `6.0` (within `1e-6`); bounds equal the input box on all axes.
  2. PASS `two_hull_of_an_l_is_between_the_l_and_its_box`: L `3.0`, hull `3.5`, bounding box `4.0`
     (`3.0 < 3.5 <= 4.0`).
  3. PASS `three_hull_of_a_sphere_keeps_its_volume`: sphere `0.5119025125985383`, hull `0.5119024938664052`
     (relative error ≈ `3.65e-8`, well under 0.5%). Evaluated with `max_vertices=2000` because a Manifold sphere
     at `segments=32` has 258 points, above the default 256; this does not weaken the volume assertion.
  4. PASS `four_hull_of_a_hull_is_idempotent`: hull `6.0`, hull of hull `6.0` (difference `0`, under `1e-9`).
  5. PASS `five_box_shape_of_a_turned_box_is_axis_aligned_and_larger`: bounding volume
     `14.660253808141306`, box shape volume `14.660253808141306`; 36 indices (12 triangles); both above `6.0`.
  6. PASS `six_oriented_box_shape_recovers_the_turned_box`: oriented box volume `6.000000708005757`
     (relative error ≈ `1.2e-7`, under 1%).
  7. PASS `seven_best_shape_of_a_thin_cylinder_is_within_half_again`: capsule chosen; ratio
     `0.9503586570786882` (≤ 1.5); warning contains `capsule` and `volume ratio`.
  8. PASS `eight_a_solid_with_more_points_than_max_vertices_is_refused`: sphere(64) refuses with
     `solid.hull_too_many_points`, suggestion
     "raise max_vertices (up to 2000) or decimate the mesh (poly.decimate) before hulling it"; a box with
     `max_vertices=4` is refused the same way.
  9. PASS `nine_two_hulls_have_bit_identical_positions`: 66 hull vertices, bitwise identical between two
     evaluations.
- Commands run and results:
  - `cargo test -p ve-modeling --test g11_hulls -- --nocapture`: 9 passed, 0 failed.
  - `cargo test -p ve-modeling -p ve-graph -p ve-measure --no-fail-fast`: all suites pass; no `FAILED` lines.
  - `cargo clippy -p ve-modeling -p ve-graph --tests`: no diagnostic points at `src/hulls.rs`,
    `tests/g11_hulls.rs` or `builtin/hulls.rs`. Other warnings are pre-existing; the `operations.rs`
    "too many lines (85/80)" one predates the 3-line dispatch hook (the function was already 82 lines).
  - `cargo xtask rules`: pass.
  - `cargo xtask quick ve-modeling ve-graph`: not green, for reasons outside this change. Its workspace
    `cargo fmt --all --check` stage stops on pre-existing unformatted files
    `crates/ve-measure/tests/{query,symmetry,uv}.rs`; its `clippy -- -D warnings` stage stops on pre-existing
    warnings in `ve-graph` and `ve-kernel`. My three new/edited files pass
    `rustfmt --edition 2024 --config-path rustfmt.toml --check`. The strict gate is still pending.
- Open questions or things you chose not to do:
  - `max_vertices` is enforced as an input-point bound (`input.mesh.positions.len()`), matching test 8's
    "more points than max_vertices allows"; `ve-measure`'s own point budget remains the hard cap.
  - `solid.collision_boxes` builds the box/oriented-box shape as the hull of its 8 corners and the capsule by
    revolving a quarter-circle profile, then reports the fit as a warning (`solid.collision_boxes.fit`,
    "… volume ratio {:.1}").
  - No changes to `ve-measure`; no bug was found there.
  - Did not run `cargo fmt`/`cargo fmt --all` (per this handoff), and did not fix pre-existing warnings or
    formatting in files owned by other work.
  - Did not commit.
