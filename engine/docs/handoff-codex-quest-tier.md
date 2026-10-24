# Handoff: a `quest` quality tier, and shadow cascades that follow the quality settings

Owner: Claude manages; Codex implements. **Crates you may edit, in this order, running that crate's tests before moving on: `ve-render` (`src/quality.rs` only, plus the tier list wherever `ve-render` matches on tiers), then `ve-bevy` (`src/map.rs`, `src/lib.rs`, `src/camera.rs`).** Do not touch `ve-runtime`, `ve-evaluator`, `ve-modeling` (Claude updates the page and the runtime afterwards).
Rules: never run `git checkout`, `git clean`, `git stash` or `git restore`; never `cargo fmt` (use `rustfmt --edition 2024 --config-path rustfmt.toml <file>`); never `cargo xtask`; use exactly `CARGO_TARGET_DIR=/tmp/codex-target-render` (shared with the texture-report agent to save disk; cargo locks it, so a build may wait a moment); do not commit or stage; edit with your edit tools, not scripts; leave `[lints]` alone. Another agent adds `src/textures.rs` pieces to `ve-render` at the same time: run only `--test quest_tier` there and do not touch `textures*.rs`, `tests/texture_report.rs`.

## Why

A Quest has about 200 draw calls and 1.5 million triangles to spend (Meta device page). Measured 2026-10-24 on the forest: 1,100 to 2,200 draws per frame, of which the four sun-shadow cascades redraw every caster each (about 150 draws each) and about 30 post passes. `RenderQuality` already has a `shadow_cascades` field, but **it never reaches Bevy**: `ve-bevy/src/map.rs` `sun_cascades()` hard-codes 4 cascades on WebGPU and 1 on WebGL2 whatever the tier says. A standalone headset needs a tier of its own that is honest about that.

## What to build

1. `ve-render`: `QualityTier::Quest` (name `quest`, parsed by `FromStr`, listed in `ALL` between `Low` and `Medium`), with `RenderQuality::for_tier(Quest)`: `msaa_samples` 4 (a tile-based GPU resolves it on chip), `shadow_resolution` 1024, `shadow_cascades` 1, `shadow_pcf_radius` 0, `max_point_lights` 2, `max_spot_lights` 2, `resolution_scale` 1.0, bloom, ssao, auto exposure and specular AA off. Update every exhaustive `match` on `QualityTier` in `ve-render` and `ve-bevy` (compile errors tell you; grep `QualityTier::`). Doc comment: Quest tier = the settings of a standalone headset.
2. `ve-bevy`: make `sun_cascades` `pub` taking `&RenderQuality`, exported from `lib.rs`, returning a `CascadeShadowConfig` with `shadow_cascades` cascades, capped at 1 when the `webgpu` feature is off (WebGL2's shaders hold one cascade: keep the existing comment) and at 4 when on; `minimum_distance` 0.1, `maximum_distance` 60.0 (30.0 when the tier has one cascade, since a Quest has little reach and shadow draws scale with it), first cascade far bound 6.0 or the maximum when only one cascade, overlap 0.2. `map_lights` must read `Res<RenderQuality>` and rebuild the sun's cascades when the quality changes (not only when the light changes): see how `camera.rs` `map_cameras` reacts to `quality.is_changed()`.

## Tests (written by Claude; never weaken)

`crates/ve-render/tests/quest_tier.rs` (5 tests) and `crates/ve-bevy/tests/cascades.rs` (3 tests, default features and, if you can build it, `--features webgpu`). Also keep `cargo test -q -p ve-bevy` (27 tests) green.

## Check

`cargo test -q -p ve-render --test quest_tier`, `cargo test -q -p ve-bevy`, `cargo clippy -q -p ve-bevy --all-targets -- -D warnings` (a `too_many_lines` error in `ve-render/src/lib.rs` `add_render_systems` is old and not yours), `cargo check -q -p ve-bevy --target wasm32-unknown-unknown`. List any other crate that fails to compile because of the new enum variant in your Status; do not fix it.

## Status (Codex: write this section yourself when you stop; your final chat message is not saved)

- [x] done / [ ] blocked:
- Tests run and their output:
  - `CARGO_TARGET_DIR=/tmp/codex-target-render cargo test -q -p ve-render --test quest_tier` — passed, 5 tests.
  - `CARGO_TARGET_DIR=/tmp/codex-target-render cargo test -q -p ve-bevy` — passed, 27 unit tests and 3 cascade tests.
  - `CARGO_TARGET_DIR=/tmp/codex-target-render cargo test -q -p ve-bevy --features webgpu --test cascades` — passed, 3 tests.
  - `CARGO_TARGET_DIR=/tmp/codex-target-render cargo check -q -p ve-bevy --target wasm32-unknown-unknown` — passed (existing warnings in `toon.rs`).
  - `CARGO_TARGET_DIR=/tmp/codex-target-render cargo clippy -q -p ve-bevy --all-targets -- -D warnings` — blocked by the noted pre-existing `too_many_lines` error in `ve-render/src/lib.rs::add_render_systems`.
  - `CARGO_TARGET_DIR=/tmp/codex-target-render cargo check -q -p ve-runtime --target wasm32-unknown-unknown` — fails because `ve-runtime/src/quality.rs` does not match `QualityTier::Quest`; that crate is outside this handoff's scope.
- Files changed: `crates/ve-render/src/quality.rs`, `crates/ve-bevy/src/map.rs`, `crates/ve-bevy/src/lib.rs`, `docs/handoff-codex-quest-tier.md` (Status).
- Crates outside your scope that match on `QualityTier`: `ve-runtime` (`src/quality.rs`, wasm32 compile fails on the missing `Quest` arm).
