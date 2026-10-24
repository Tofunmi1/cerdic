# Handoff: `MaterialTextures::report`, what the scene's maps take

Owner: Claude manages; Codex implements. **Crate you may edit: `ve-render` only** (`src/textures.rs`, the export list in `src/lib.rs`, and `src/textures_tests.rs` if you add unit tests). Do not touch other crates.
Rules: never run `git checkout`, `git clean`, `git stash` or `git restore`; never `cargo fmt` (use `rustfmt --edition 2024 --config-path rustfmt.toml <file>`); never `cargo xtask`; use exactly `CARGO_TARGET_DIR=/tmp/codex-target-render` (shared with the quest-tier agent to save disk; cargo locks it, so a build may wait a moment); do not commit or stage; edit with your edit tools, not scripts; leave `[lints]` in every Cargo.toml alone. Test with `cargo test -q -p ve-render --test texture_report` (other test files in `ve-render` may be mid-edit by another agent: do not run the whole crate's tests, and do not fix files that are not yours).

## Why

The scene's baked maps are RGBA8 in three sizes under a 192 MiB budget (ADR 0052). Nothing reports how much a scene really uses, so we cannot tell whether KTX2/ASTC compression is worth building or what a Quest budget should be. A report per size, plus what ASTC 6x6 would take, is the measurement.

## What to build

In `ve-render/src/textures.rs`, a `pub struct TextureReport` (derive `Clone, Copy, Debug, PartialEq`) and `MaterialTextures::report(&self) -> TextureReport`, exported from `lib.rs` next to `MaterialTextures`:
- `maps: [usize; 3]` maps per size (1024, 512, 256);
- `bytes: [usize; 3]` GPU bytes per size, mip chains counted (use `map_cost`);
- `total_bytes: usize` (equals `MaterialTextures::bytes()`);
- `budget_bytes: usize` (`BUDGET_BYTES`);
- `used_fraction: f32` (total over budget);
- `astc_6x6_estimate_bytes: usize`: what the same maps would take compressed with ASTC 6x6 (128 bits per 6x6 texels), mip chain included, rounding each mip level up to whole blocks. State in the doc comment that it is an estimate.
Pure and cheap: no allocation beyond the struct, no pixel reads.

## Tests (written by Claude; never weaken): `crates/ve-render/tests/texture_report.rs`

Five tests: empty scene; counts and costs by size; the compressed estimate is 0.10 to 0.125 of the total; clear resets; a full budget reads above 0.9. Add unit tests in `textures_tests.rs` for the block rounding of the estimate if you like.

## Check

`cargo test -q -p ve-render --test texture_report` passes; `cargo clippy -q -p ve-render --lib -- -D warnings` reports nothing from your files (a `too_many_lines` error in `src/lib.rs` `add_render_systems` is old and not yours); `cargo check -q -p ve-render --target wasm32-unknown-unknown`.

## Status (Codex: write this section yourself when you stop; your final chat message is not saved)

- [x] done / [ ] blocked:
- Tests run and their output: `CARGO_TARGET_DIR=/tmp/codex-target-render cargo test -q -p ve-render --test texture_report` — passed (5 tests); `CARGO_TARGET_DIR=/tmp/codex-target-render cargo check -q -p ve-render --target wasm32-unknown-unknown` — passed; `CARGO_TARGET_DIR=/tmp/codex-target-render cargo clippy -q -p ve-render --lib -- -D warnings` — reports only the pre-existing `too_many_lines` error in `src/lib.rs:add_render_systems` noted above, with no diagnostics from the texture report changes.
- Files changed: `crates/ve-render/src/textures.rs`, `crates/ve-render/src/lib.rs` (export), `docs/handoff-codex-texture-report.md` (this status).
