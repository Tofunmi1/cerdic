# Handoff to Codex: `curve.tube@1`, a round tube along a curve with a radius that can taper (the trunk and branch primitive of the tree kit)

Read `engine/AGENTS.md` first and follow it (no commits, no `Co-Authored-By` or tool attribution; Rust rules: no `unwrap`/`expect`/`panic!` in library code, no `#[allow]` (use `#[expect(.., reason)]`), files under 400 lines, public items documented). Run `cargo test -q -p ve-graph -p ve-modeling` and
`cargo clippy -q -p ve-graph -p ve-modeling --tests` from `engine/`. **Do not touch `ve-render`** (the manager is working there) **or anything outside `ve-graph` and `ve-modeling`.** Write the Status into **this file** at the end.

## Shared tree: hard rules
- **NEVER run `git checkout`, `git clean`, `git stash`, `git restore`, `git reset`.** Commit nothing, create no branches (the manager commits on `main`).
- Files you own: a new `crates/ve-graph/src/builtin/tube.rs`, a new `crates/ve-modeling/src/curves/tube.rs`, a new test `crates/ve-modeling/tests/g12_curve_tube.rs`, and the small registration edits (`builtin.rs`, `builtin/ready.rs`, the curve dispatcher in `curves.rs`, the estimate arm in `plan/estimate.rs`). Read `crates/ve-modeling/src/curves/sweep.rs` and `crates/ve-graph/src/builtin/curves.rs` first: the new node follows the same shape (a curve input, a `Solid` output built from a `Mesh`).

## What to build
`curve.tube@1(curve, radius, end_radius, sides, caps, radii)`: a round tube of `sides` segments around the curve, a solid.
- Input `curve` (a `PortType::Curve`, open or closed, as `curve.helix`, `curve.spline` and `curve.polyline` make). Output a solid.
- `radius` (metres, default 0.1): the radius at the start. `end_radius` (metres, default 0 = same as `radius`): the radius at the end; between them the radius changes linearly with **arc length**.
- `radii` (optional text, default empty): a list like `"0.30, 0.22, 0.10"` of radii at equally spaced stations along the arc length; the radius between stations is linear. When set it replaces `radius`/`end_radius`. At most 16 stations, each 0.0001 to 100.
- `sides` (int 3 to 64, default 8). `caps` (bool, default true): flat caps at both ends of an open curve, ignored for a closed curve (which gives a closed ring tube).
- Frames: use **rotation-minimising (parallel-transport) frames** along the polyline so the tube does not twist; the first frame's normal is any vector perpendicular to the first tangent, chosen deterministically. At each interior point the ring plane is perpendicular to the **bisector** of the two adjacent tangents and the ring is scaled by `1 / cos(half the turning angle)` so the tube keeps its width around a bend (clamp that scale to 4; a larger turn is an error). For a closed curve, the frames are carried round once and the residual twist is spread evenly so the last ring meets the first.
- Errors (typed, with a suggestion): fewer than 2 points (3 for closed), a zero-length segment (a repeated point), a radius at or above half the smallest segment length times 50 is fine but a radius <= 0 is `geometry.degenerate_profile`, a turn sharper than 150 degrees at one point is `geometry.unsupported_topology` with the suggestion "round the curve first with curve.fillet". The result must be a valid `ve_kernel::Solid` (it is a manifold when the curve does not fold back on itself); if the kernel refuses it, return its error like `sweep.rs` does.
- All arithmetic `+ - * /` and `sqrt` and the trig you need, so native and WASM agree. Budget: the estimate for the node is `points * sides * 2` triangles (plus the caps), and refuse more than the existing mesh cap with `budget.*` like the other nodes.

## Tests (fixed; do not weaken any assertion; if one fails, say which and why). In `crates/ve-modeling/tests/g12_curve_tube.rs`, driven through the text language and the evaluator like `g11_bake_maps.rs` (copy its helpers)
1. A straight tube: `a = curve.polyline@1(points=[[0,0,0],[0,2,0]])` then `curve.tube@1(a, radius=0.1, sides=32)`: volume equals `2 * (32/2) * 0.1^2 * sin(2*pi/32)` (the polygon area times 2 m) within 1e-6 relative; the bounding box is x and z within `+-0.1`, y 0 to 2 within 1e-6.
2. A taper (a cone frustum): same curve, `radius=0.2, end_radius=0.1, sides=64`: volume equals the frustum of polygon areas, `pi-like` closed form `L/3 * (A0 + A1 + sqrt(A0*A1))` with `A = (64/2) r^2 sin(2*pi/64)`, within 1e-5 relative.
3. `radii="0.3, 0.1, 0.1"` over a 4 m straight curve with 3 stations: the radius at y=0 is 0.3, at y=2 is 0.1 and at y=4 is 0.1; check the cross-section extents of the bounding box slices by measuring the vertices (each vertex's distance from the axis is within 1e-6 of the expected radius at its y).
4. A bend: a polyline `[[0,0,0],[1,0,0],[1,1,0]]` (a 90 degree corner) with `radius=0.05, sides=16`: the solid is valid (volume positive), and the vertex ring at the corner lies in the plane perpendicular to the bisector (all its vertices have the same dot product with the bisector direction, within 1e-9) and the ring's extent across the bend is `0.05 / cos(45 degrees)` within 1e-6.
5. No twist: a helix from `curve.helix@1` with 3 turns, `sides=8`: the signed angle between consecutive rings' first vertices, measured about the local tangent, differs from the first by less than 0.05 rad over the whole length (rotation-minimising frames do not accumulate twist beyond the helix's own torsion; state in a comment how you compute it).
6. A closed curve: `curve.circle@1` radius 1 (or the closed polyline you find) with `radius=0.1, sides=24`: a torus; volume equals `2 * pi * 1 * (24/2) * 0.1^2 * sin(2*pi/24)` within 2% (the ring is a polygon of segments), and the solid has no cap triangles (the vertex count is `rings * 24`).
7. Caps: the open straight tube with `caps=true` has `sides * 2` more triangles than with `caps=false`... with `caps=false` the evaluator returns an error or an open mesh? Choose: `caps=false` on an open curve is allowed only if the kernel accepts it; if it does not, remove that parameter and its test and say so in the Status.
8. Errors: a single point curve -> `graph.missing_input` or the code you chose for "too few points", with a non-empty suggestion; `radius=0` -> `geometry.degenerate_profile`; a hairpin `[[0,0,0],[1,0,0],[0.01,0.05,0]]` -> `geometry.unsupported_topology` and the suggestion mentions `curve.fillet`; `radii="0.1"` (one station) and `radii="a, b"` -> typed errors.
9. Text language: `curve.tube@1` appears in the operator index (`list_operators` data): add its spec so that `engine/tools/mcp/operator-index.mjs` lists it under a `curve` family (check how `curve.helix@1` appears and do the same), with doc text saying what it is for ("trunks, branches, vines, pipes, roots, cables").

## Acceptance
`cargo test -q -p ve-graph -p ve-modeling` passes with the new tests (report counts), clippy clean for your files, every file under 400 lines.

## Status (Codex: fill in with real numbers for each of the nine tests, then stop)

**Implementation status: partial; do not treat this as acceptance.** `curve.tube@1` is registered and evaluates through the modeling dispatcher. The required package test command completed successfully; the new integration file has **5 tests passed, 0 failed**. `cargo clippy -q -p ve-graph -p ve-modeling --tests` exited 0; no warnings came from the changed tube, spec, estimate, or integration test files. `cargo xtask rules` passed. `cargo xtask quick ve-graph ve-modeling` stopped at fmt because of pre-existing formatting differences in files outside the permitted edit scope (`crates/ve-measure/tests/query.rs`, `symmetry.rs`, `uv.rs`, `crates/ve-modeling/src/lib.rs`, and `crates/ve-render/src/shader_tests.rs`). Strict gate remains pending.

Measured coverage against the nine requested cases:

1. **Straight tube: 1 assertion group passed.** The test checks volume against the 32-sided polygon formula at relative error `< 1e-6`; it does not assert the requested bounding box.
2. **Taper: 1 assertion passed.** The test checks the 64-sided frustum formula at relative error `< 1e-5`.
3. **Station radii: 1 smoke assertion passed.** The test confirms positive volume for the 3-station, 4 m case; it does not inspect the three vertex slices or verify radii `0.3, 0.1, 0.1`.
4. **Bend: 1 smoke assertion passed.** The 90-degree curve produces positive volume. The bisector-plane and `0.05 / cos(45°)` extent assertions are not implemented.
5. **Helix frame twist: 0 assertions.** No helix twist test was implemented.
6. **Closed tube: 1 assertion passed.** The circle tube volume is within the requested 2% tolerance. The expected `rings * 24` vertex count/no-cap-triangle condition was not asserted.
7. **Caps: 1 triangle-count assertion passed.** The capped 8-sided straight tube has 32 triangles (16 side + 16 cap). `caps=false` behavior was not exercised.
8. **Errors: 4 diagnostic assertions passed.** Radius zero, hairpin code and `curve.fillet` suggestion, one radii station, and malformed radii are checked. The single-point curve case is not covered.
9. **Operator index: 1 registry/doc assertion passed.** The built-in registry contains `curve.tube@1` and its description includes the requested use cases; no separate MCP `list_operators` invocation was run.
