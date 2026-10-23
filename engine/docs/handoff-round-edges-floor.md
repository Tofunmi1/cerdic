# Handoff: `poly.round_edges` fails when a cut reaches the bottom face

Owner: Claude. Helper: Codex. Crate you may edit: `crates/ve-geometry` only (the test file is mine, do not edit it).

## Problem
`poly.round_edges@1(p, width=0.03, segments=N)` on a wall with a doorway cut down to the floor
(a boolean difference whose cutter crosses the bottom face) fails:
- segments=3: `poly.triangulation_unsupported ... cannot triangulate a polygon: self-intersecting polygon`
- segments=2 on larger doorways (e.g. `adobe_hut` without its 0.1 m sill): "folded curved quad"
Where the doorway meets the floor, the bevel strips of the two jamb edges and the floor edge meet at a corner
of three bevelled edges whose profile caps overlap or fold. Round must produce a valid closed mesh there.

## The test (fixed, do not weaken)
`crates/ve-modeling/tests/g20_cut_wall_rounding.rs`, test `a_doorway_down_to_the_floor_rounds_with_several_segments`
(segments 1, 2, 3 must all evaluate and give more triangles than the unrounded wall).
Also keep `crates/ve-geometry/tests/g11_round_edges.rs` and `crates/ve-modeling/tests/g11_round_edges.rs` passing.

Add your own numeric tests in `crates/ve-geometry/tests/` (new file `g21_round_floor_corner.rs`): the rounded doorway wall must be
closed (every edge shared by exactly two faces), its volume must be a little less than the sharp one and more than sharp minus
(edge length x width^2), and no triangle may have zero area.

## Where to look
`crates/ve-geometry/src/poly_ops/round.rs` (and `crates/ve-geometry/src/triangulate.rs` for the error text).
Find where a vertex with 3 or more bevelled edges builds its corner patch; find why it folds when two of the edges are
collinear-neighbouring boundary faces at the cut. Prefer a fix in the corner patch construction, not loosening the fold check.

## Rules
- Never run `git checkout`, `git clean`, `git stash`, `git restore`. Do not touch other crates or files. No commits.
- Do not run `cargo fmt`; format a file with `rustfmt --edition 2024 --config-path rustfmt.toml <file>`.
- Use your own `CARGO_TARGET_DIR=target-codex-round`. No `cargo xtask`.
- No `unwrap`/`expect`/`panic!` in library code, no `#[allow]`, files under 400 lines.
- Run: `CARGO_TARGET_DIR=target-codex-round cargo test -p ve-geometry` and `cargo test -p ve-modeling --test g20_cut_wall_rounding --test g11_round_edges --test g11_kits`.
- Write the final Status below yourself (your final message is not saved).

## Status
Blocked by the handoff's crate restriction and stale paths. In this checkout, `round_edges` is in
`crates/ve-modeling/src/poly_ops/round.rs`, and the actual bevel operation is Blender's BMesh bevel
behind `crates/ve-kernel`/C++; `ve-geometry` has no corner patch implementation to fix. The handoff
allows edits only to `crates/ve-geometry`, and explicitly forbids editing its test in `ve-modeling`.
I made no code or test changes because the necessary fix is outside the allowed crate.

Checks run:
- `CARGO_TARGET_DIR=target-codex-round cargo test -p ve-geometry` — still running when the command
  time limit elapsed; the tests printed before termination passed.
- `CARGO_TARGET_DIR=target-codex-round cargo test -p ve-modeling --test g20_cut_wall_rounding --test g11_round_edges --test g11_kits`
  — `g11_round_edges` (4) and `g11_kits` (8) passed; `g20_cut_wall_rounding` failed on the fixed
  doorway test at segments=3 with `self-intersecting polygon`.

To complete this task, update the permitted crate scope to include the BMesh bevel implementation
(`crates/ve-kernel`/`engine/cpp`) and the geometry/modeling integration-test changes as needed.
