# Handoff: `curve.hang`, a rope or chain hanging between two points

Owner: Claude. Helper: Codex. Plain Rust, no Blender, no C++.

## Why
The outposts and harbours need ropes, washing lines, chains, cables and bunting between two tie points, and a straight
tube between two points looks fake. `curve.tube` already turns a curve into a rope; this adds the curve.

## The test (fixed, do not weaken)
`crates/ve-modeling/tests/g22_curve_hang.rs` (7 tests, closed-form catenary values). Do not edit it.
Run: `CARGO_TARGET_DIR=target-codex-hang cargo test -p ve-modeling --test g22_curve_hang`.

## What to build
Operator `curve.hang@1`, no inputs, output Curve (a polyline like `curve.bezier` makes). Parameters:
- `from` Vec3 (Length, default [0, 3, 0]), `to` Vec3 (default [4, 3, 0]): the two tie points in metres. `-EXTENT..EXTENT` like other curves.
- `slack` Num (Ratio, default 0.05, min 0, max 2): extra rope length as a fraction of the straight distance, so length L = distance * (1 + slack).
- `points` Int (default 32, min 3, max 512): points along the rope, evenly spaced in horizontal distance.
Shape: a catenary under gravity along -Y between the two points with arc length L. With horizontal distance h and height difference v, find
`a` with `sqrt(L^2 - v^2) = 2 a sinh(h / (2a))` by deterministic bisection (200 iterations, no random, no f32 accumulation: compute in f64),
then place the points with the usual catenary formula shifted so both ends hit the tie points. Straight when slack = 0 or the ends
are vertically above each other (h below 1e-9): a straight line with the same `points`. Identical ends are refused with code
`geometry.invalid_curve` and a suggestion naming `from` and `to`. If slack is positive but tiny (L - distance below 1e-9) use the straight line.

## Where
- Spec: `crates/ve-graph/src/builtin/curves.rs` next to `curve.bezier` (follow its style), and add `"curve.hang@1"` to the ready list in
  `crates/ve-graph/src/builtin/ready.rs` next to `curve.bezier@1`.
- Evaluate: `crates/ve-modeling/src/curves.rs` (the `evaluate` function dispatches by family; add a sampler in `curves/sample.rs`).
- Cost estimate: `crates/ve-modeling/src/plan/estimate.rs` where `curve.bezier` is handled.
- Docs: one line in `docs/modeling/` where curve operators are listed (grep for `curve.helix`).
- Add unit tests of the solver for a sloped case (check the rope length by summing segment lengths with 512 points: within 0.2% of L).
- Add `curve.hang` to the guide only if you find the list of curve operators in `tools/mcp/server.mjs`; otherwise leave it.

## Rules
- You may edit only the files named above. I (Claude) am editing other files in `ve-graph` and `ve-modeling` at the same time:
  `ve-graph/src/builtin/world_pad.rs`, `ve-modeling/src/world_pad.rs`, `ve-modeling/src/operations.rs`, `ve-modeling/src/lib.rs`,
  `ve-modeling/src/plan/estimate.rs` (only the terrain lines): do not touch those lines. Never run `git checkout`, `git clean`,
  `git stash`, `git restore`. No commits.
- Do not run `cargo fmt`; use `rustfmt --edition 2024 --config-path rustfmt.toml <file>` on your own files.
- `CARGO_TARGET_DIR=target-codex-hang`, no `cargo xtask`. No `unwrap`/`expect`/`panic!` in library code, no `#[allow]`, files under 400 lines.
- When done also run `cargo test -p ve-graph` and `cargo test -p ve-modeling --test g5_surfaces`.
- Write the final Status below yourself (your final message is not saved). Say honestly what you did not do.

## Status
Implemented `curve.hang@1` with bounded parameters, deterministic f64 catenary solving and sampling, evaluator and cost-estimate dispatch, ready registration, modeling docs, and MCP guide entry. Added a sloped-case solver length test; the handoff integration test remains unchanged.

Passed:
- `CARGO_TARGET_DIR=target-codex-hang cargo test -p ve-modeling --test g22_curve_hang` (7 passed)
- `CARGO_TARGET_DIR=target-codex-hang cargo test -p ve-graph` (all passed)
- `CARGO_TARGET_DIR=target-codex-hang cargo test -p ve-modeling --test g5_surfaces` (11 passed)
- `CARGO_TARGET_DIR=target-codex-hang cargo test -p ve-modeling --lib curves::sample::hang_tests::sloped_catenary_samples_have_the_requested_rope_length` (1 passed)

Did not run `cargo xtask`, as instructed. Existing unrelated compiler warnings appeared during the test builds.
