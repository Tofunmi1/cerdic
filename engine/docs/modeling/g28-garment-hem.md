# G28: clean garment hems

`body.garment@1` now has an opt-in `hem` Bool, defaulting to false. When enabled, the evaluator finds
open boundary loops, fits least-squares planes through each loop, places each plane at the median loop
projection, and clips triangles on the exterior side. Clipped UVs are interpolated per corner and
new-vertex skinning uses the nearer source vertex's joints and weights. With `hem=false`, the existing
garment construction path is preserved.

The fixed acceptance test is `crates/ve-modeling/tests/g28_garment_hem.rs`. It runs, but currently 3 of
4 tests fail: the waist and cuffs are still ragged, and the skin weight count assertion reports 3,463
against 13,855 positions. The optional/default behavior test passes. The required compatibility tests
`g26_garment` (4 tests), `g25_human_rig` (5 tests), and `cargo test -p ve-graph` (all tests) passed with
`CARGO_TARGET_DIR=target-codex-hem`.

`cargo xtask quick ve-modeling ve-graph` stopped at its workspace `fmt --check` stage because many
unrelated changed workspace files have rustfmt diffs; no repository-wide formatter was run. The focused
`cargo xtask rules` check reports unrelated overlong files (`ve-game/parse.rs`, `ve-graph/builtin/{fields,
world}.rs`, `ve-modeling/human_hair.rs`, `ve-modeling/evaluate.rs`, `ve-runtime/game.rs`), and passes the
line limits for the files in this change. The hem unit test command was blocked by a duplicate `tests`
module in concurrently edited animation files. Those files were not changed here. Browser checks were
not run. Strict gate pending.
