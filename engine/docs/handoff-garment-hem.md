# Handoff: `body.garment` `hem`, a clean cut edge

Owner: Claude. Helper: Codex. Plain Rust.

## Why
`body.garment` (shirt, long sleeves, trousers, shoes) takes the part of the body surface that a garment covers. The cut follows the body's own skin regions, so the waist hem
and the cuffs are ragged saw-teeth, which shows in every animation of a dressed person. A real hem is a clean line.

## The test (fixed, do not weaken)
`crates/ve-modeling/tests/g28_garment_hem.rs` (4 tests). Do not edit it. Run: `CARGO_TARGET_DIR=target-codex-hem cargo test -p ve-modeling --test g28_garment_hem`.
Existing tests must keep passing: `g26_garment` and `g25_human_rig`.

## What to build
A new optional Bool parameter `hem` (default false) on `body.garment@1`. Default false must leave the output bit-identical to today.
When true, every open edge of the garment surface becomes a clean cut:
- Find the boundary loops of the garment (edges used by one triangle, chained into loops).
- For each loop fit a plane (least squares through its vertices; for a loop around a limb or the waist the normal is the loop's mean direction of travel across the cut, i.e.
  the smallest principal axis of the loop points) and cut the garment along that plane: remove the cloth beyond the plane on the far side of the garment's interior
  and clip triangles that cross it so the new boundary lies exactly in the plane (interpolate positions, normals, UVs, skin joints and weights of the new vertices; for weights
  take the weights of the nearer original vertex rather than blending different joint sets).
- Choose the cut plane position so little cloth is lost: put it at the median of the loop points along the normal, so about half of the saw-teeth are cut away and the
  rest of the notches are closed by clipping. Triangle count of the result stays within 15% of the plain garment (the test checks this).
- Positions stay finite, indices valid, one weight set per vertex, still skinned with the same bones.
Skinning layout, UVs and tangents of the existing garment code must be kept (read `crates/ve-modeling/src/human_rig.rs` first, the garment part is near its end).

## Where
- `crates/ve-modeling/src/human_rig.rs` (or a new `human_rig/hem.rs` module if the file would exceed 400 lines), the spec in `crates/ve-graph/src/builtin/human.rs` (parameter `hem`
  with a doc sentence that also updates the "a hem is a little ragged" text), `tools/mcp/guide-creatures.mjs` (the "ANIMATING A PERSON" topic: mention `hem=true`), and
  `docs/modeling/` where `body.garment` is listed.
- Add unit tests of the plane fit and the clip in the new code.

## Rules
- You may edit only the files named above. Another helper is editing `ve-modeling/src/anim/*`, `ve-graph/src/builtin/modeling/animation.rs` and `ready.rs`: do not touch them. Never run `git checkout`, `git clean`, `git stash`, `git restore`. No commits.
- Do not run `cargo fmt`; use `rustfmt --edition 2024 --config-path rustfmt.toml <file>` on your own files.
- `CARGO_TARGET_DIR=target-codex-hem`, no `cargo xtask`. No `unwrap`/`expect`/`panic!` in library code, no `#[allow]`, files under 400 lines.
- When done also run `cargo test -p ve-graph`.
- Write the final Status below yourself (your final message is not saved). Say honestly what you did not do.

## Status
Implemented the optional parameter, clipping module, operator docs, and creature guide update. This is
not complete: the fixed G28 test runs but 3 of 4 tests fail (the cuff and waist are still ragged, and the
test's skin-weight count assertion fails). `g26_garment`, `g25_human_rig`, and `cargo test -p ve-graph`
pass. `cargo xtask quick` stops at workspace fmt diffs in unrelated pre-existing files; `cargo xtask
rules` fails on unrelated overlong files and no longer reports the files from this change. I did not edit
the concurrent animation files. Details are in `docs/modeling/g28-garment-hem.md`; strict gate pending.
