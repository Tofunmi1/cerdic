# Handoff to Codex: `haze` and `light_shafts` on `scene.environment` and `scene.sky`

Read `engine/AGENTS.md` and `engine/docs/handoff-common.md` section 2 and follow them (no `unwrap`/`expect`/`panic!`/`println!` in library code, no `#[allow]`: use
`#[expect(lint, reason)]` on the smallest item only, files under 400 lines, docs on public items, tests, **no commits, no `git checkout`/`clean`/`stash`/`restore`**, no
`Co-Authored-By` or tool attribution). **Never run `cargo fmt` or `cargo fmt --all`; format only your files with `rustfmt --edition 2024 --config-path rustfmt.toml <file>`
from `engine`.** Edit only `crates/ve-graph` (the two operator specs in `src/builtin/environment.rs`) and `crates/ve-modeling` (`src/environment.rs`, `src/result.rs`,
`src/scene_settings.rs`). Another engineer is editing `ve-runtime`, `ve-bevy`, `ve-render` and `ve-modeling/src/rig`, `src/finish.rs`; do not touch those.

## Why
A forest, a swamp, a mountain valley or a morning street needs distance haze and shafts of light through it. Bevy's renderer has distance fog and volumetric light, but the
language has no way to ask for them: everything must be reachable from the text language. This job adds the parameters, the evaluated data and the validation. The mapping
onto Bevy's `DistanceFog`, `VolumetricFog` and `VolumetricLight` is done by the other engineer afterwards.

## What to build
Four new parameters on **both** `scene.environment@1` and `scene.sky@1` (each with units, range, default and one line of help written for a model):
- `haze` number 0..=1, default 0: how thick the distance haze is (0 is clear air, 0.2 a light morning mist, 1 a thick fog).
- `haze_color` vec3 (sRGB 0..1), default `[0.72, 0.78, 0.86]`: the colour of the haze.
- `haze_height` number metres 0..=10000, default 0: 0 is a uniform haze, above 0 the haze is thickest at the ground and thins out over this height (ground fog).
- `light_shafts` number 0..=1, default 0: how visible the sun's light shafts are through the haze (needs haze above 0 to show).

Result data: a `pub struct Haze { pub density: f32, pub color: [f32; 3], pub height: f32, pub shafts: f32 }` (serde, `Clone, Copy, Debug, PartialEq`, with `Default` = clear air:
density 0, color the default above, height 0, shafts 0) in `scene_settings.rs` next to `Wind`, exported from `ve-modeling` like `Wind` is, and a field
`#[serde(default)] pub haze: Haze` on `SceneEnvironment` (`result.rs`). The environment evaluator (`environment.rs`) fills it from the node's parameters like it fills `wind`.
When a `scene.sky` node supplies the environment, its haze parameters fill the same field. Out-of-range values are refused through the operator specs' normal range check, which
produces the `param.range` diagnostic (the same way other bounded parameters are refused); do not clamp silently.

## Tests, already written
`crates/ve-modeling/tests/g12_haze.rs`. Do not weaken them. If a test premise is wrong, say so in the Status and explain; do not change it quietly. Add a unit test of your own
for the default `Haze` if it helps.

## Acceptance
`cargo test -p ve-graph -p ve-modeling` passes (the existing `g9_environment` tests must still pass, and the serialized form of an environment with no haze must still load),
`cargo clippy -p ve-modeling --all-targets -- -D warnings` clean for your files, `cargo check -p ve-modeling --target wasm32-unknown-unknown` passes, no file you touch over 400 lines
(`result.rs` is near the limit: put new types in `scene_settings.rs`).

## Status (Codex: fill in when you finish, then stop)
- State: Implemented. The haze evaluator inherits sky haze only when the environment haze is its clear-air default; an authored environment haze takes precedence.
- What was built: Added `haze`, `haze_color`, `haze_height`, and `light_shafts` to both operator specs with documented ranges/defaults; added/exported serializable `Haze`, default clear-air haze, backward-compatible `SceneEnvironment.haze`, and evaluation for both scene nodes. Existing sky/environment merging now propagates haze.
- Commands run and results: Formatted only the touched Rust files with the required `rustfmt` invocation. `cargo xtask rules` passed. `cargo test -p ve-graph -p ve-modeling` compiled and ran; the haze integration tests had 6 passes and 1 failure described below. `cargo xtask quick ve-graph ve-modeling` stopped at workspace `fmt --check` due formatting diffs in unrelated concurrent work, including `ve-bevy` and vendored Bevy. `cargo check -p ve-modeling --target wasm32-unknown-unknown` failed in unrelated `crates/ve-graph/src/builtin/modeling/rig.rs` because it refers to nonexistent `Unit::Metres`. Clippy did not complete because its build was interrupted during the same long native build. Strict gate pending.
- Could not verify / open questions: The written `out_of_range_haze_is_refused_with_the_parameter_named` premise is incorrect for the current API: `parse_text` rejects the out-of-range literal immediately with `param.range`, before the test's `apply` helper can inspect an `Outcome`; it panics at its parse unwrap-or-panic. The spec performs the intended normal range rejection, and the test was left unchanged per handoff. The full requested suite therefore exits failing on this test. Wasm and clippy remain unverified due concurrent unrelated `rig.rs` compile errors. No open implementation decisions remain.
