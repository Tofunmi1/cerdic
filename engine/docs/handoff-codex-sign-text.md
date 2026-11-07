# Handoff: `sign.text@1`, lit text as geometry (the neon of a cyberpunk street)

Owner: Claude manages; Codex implements. **Do this AFTER `handoff-codex-detail-panels.md` is finished and its tests pass, in the same agent run, in the same crates: `ve-graph` (spec, ready list) then `ve-modeling` (a new `src/sign_text.rs`, registration in `operations.rs`, estimate in `plan/estimate.rs`).** Do not touch `ve-world` (its `pixel_text::layout` is finished: use it), `src/material_presets.rs` (Claude edits it), other crates.
Rules: as in the panels handoff (never `git checkout/clean/stash/restore`, never `cargo fmt` or `cargo xtask`, `CARGO_TARGET_DIR=/tmp/codex-target-panels`, no commit or stage, edit tools only, leave `[lints]` alone). Test with `--test sign_text`.

## Why

The owner chose a cyberpunk set (2026-10-25): a dense, narrow, rainy night street lit by neon and LED signs. Research (secondary sources) names the vocabulary: neon and LED signs and holograms in cyan and magenta, doubled by wet pavement, through haze; layered clutter (cables, pipes, AC units). A sign must be real geometry in VR (the player reads it from a metre away, bloom blooms it, and its emissive area can drive a point light), so text becomes boxes. `ve_world::pixel_text::layout(text, pixel)` already turns text into lit-pixel rectangles (`[x, y, width, height]`, glyphs 5 by 7 pixels, advance 6 pixels, 7 pixels tall; it backs the game HUD).

## What to build

`sign.text@1(text, height=0.5, depth=0.04, color=[1.0, 0.1, 0.6], glow=3)`, no input, output a solid:
- `text`: string 1 to 48 characters with at least one lit pixel (empty or only spaces refused with a typed diagnostic). `height`: metres 0.05 to 10: the height of the 7-pixel glyph rows, so the pixel size is `height / 7`. `depth`: metres 0.005 to 1: how far the letters stand out. `color`: a colour (read each component with `Param::as_f64`; whole numbers arrive as integers in saved documents). `glow`: 0 to 20.
- Geometry: one closed box per lit pixel rectangle (or merge horizontal runs into longer boxes if simple; the volume must still equal lit pixels x pixel area x depth), centred on x = 0 (the text width is `layout.width`), standing on y = 0 (the bottom row at y = 0, text rows going up: note `layout` has row 0 at the top of the glyph, so flip y), from z = 0 to z = depth, facing +z. Under 40,000 triangles for 48 `W`s at height 1.
- Material (set by the node, like `solid.material` does): `color` is the albedo scaled to 0.15 (a dark housing), `emissive` is `color * glow`, so glow 0 is unlit, roughness 0.4, metallic 0. A later `solid.material` or `material.preset` may override it.
- Operator doc: what it does, the orientation (centred, standing on y=0, facing +z), that the sign has no backing plate (make one with `solid.box`), and an example `sign.text@1(text="OPEN", height=0.6, color=[0.1, 1, 1], glow=4)`.
- Deterministic; no randomness. Typed `param.range` diagnostics for bad values.

## Tests (written by Claude; never weaken): `crates/ve-modeling/tests/sign_text.rs`

Nine tests: centred, on the floor, facing forward (bounds of `OPEN` at height 0.7: x within 1.15 m, z within the depth); the volume equals the lit pixels times depth; text differs and repeats; the sign glows in its colour (emissive peak above 1 at glow 3, hue kept, glow 0 unlit); integers accepted; empty or space-only or over-long text refused; bad sizes refused; a 48-character sign stays under 40,000 triangles; it moves and passes `check.attached` against a backing plate. Run `cargo test -q -p ve-graph` too (update only entries for the new operator).

## Check

`cargo test -q -p ve-graph`, `cargo test -q -p ve-modeling --test sign_text --test detail_panels --test material_preset --test check_attached`, `cargo build -q -p ve-evaluator --target wasm32-unknown-unknown --release`.

## Status (Codex: write this section yourself when you stop; your final chat message is not saved)

- [x] done (2026-11-07, implemented by Claude)
- Algorithm: `ve_world::pixel_text::layout` rects, y flipped so the bottom row is at y = 0, horizontal runs of lit pixels merged into one box, centred on x, each box from z = 0 to `depth`, joined with the kernel union; the node sets the material (colour x 0.15, emissive colour x glow, roughness 0.4) in `operations.rs` after the value is made.
- Tests run: `cargo test --release -p ve-modeling --test sign_text` 9 passed.
- Files changed: `ve-graph/src/builtin/detail.rs`, `ve-modeling/src/sign_text.rs` (new), `operations.rs`, `plan/estimate.rs`, `lib.rs`.
