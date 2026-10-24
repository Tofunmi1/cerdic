# Handoff: `material.preset@1`, a named layered surface

Owner: Claude manages; Codex implements. **Crates you may edit, in this order, running that crate's tests before moving on: `ve-graph` (operator spec and the ready list), `ve-modeling` (evaluation, a new `src/material_presets.rs`).** Do not touch `ve-render`, `ve-bevy`, `ve-runtime`, `ve-evaluator`, `src/engine/texture/*.ts`.
Rules: never run `git checkout`, `git clean`, `git stash` or `git restore`; never `cargo fmt` (use `rustfmt --edition 2024 --config-path rustfmt.toml <file>`); never `cargo xtask`; use exactly `CARGO_TARGET_DIR=/tmp/codex-target-preset`; do not commit or stage; edit with your edit tools, not scripts; leave `[lints]` alone.
`ve-modeling`'s lib test target may fail to compile for reasons that are not yours: test with `--test material_preset` and `--test materials`. You cannot run a browser here: **Claude compiles and looks at every preset program in the real viewer afterwards**, so write programs in the style of known-good ones and say in your Status which node names you were unsure of.

## Why

A model that wants "a mossy brick wall" today writes a raw texture program (a Material Maker-derived language, about 350 nodes) or picks one of 10 fixed `recipe` words. Programs written from scratch often fail or look flat. A library of vetted named presets, with sliders for wear, dirt, moss and wetness driven by the part's own mesh (curvature, cavity, up-facing), gives every model a good surface in one line. It sits on the existing path: it sets the same `Material.texture_program`, `texture_tile` and `texture_size` as `solid.texture` (`crates/ve-modeling/src/appearance.rs` `texture` and `textured_surface`; reuse them, including the regions and instance-set handling).

## What to build

`material.preset@1(in, name, tile=1.0, wear=0.3, dirt=0.2, moss=0.0, wet=0.0, size="512")`:
- Input `in`: the same kinds `solid.texture` takes (`COLOURABLE`); output the same as input.
- `name`: choice of 14: `oak`, `planks`, `red_brick`, `cobblestone`, `concrete`, `plaster`, `rusted_iron`, `brushed_steel`, `leather`, `linen`, `sand`, `mud`, `snow`, `ice`. Doc: say what each looks like in a few words.
- `tile`: length in metres of one repeat (0.1 to 10, default 1.0). `wear`, `dirt`, `moss`, `wet`: numbers 0 to 1 (refused outside). `size`: choice `256`/`512`/`1024`, default `512`.
- Evaluation: each preset is data in `material_presets.rs`: a base colour, metallic, roughness factor (the program paints the real roughness, so use 1.0 as `textured_surface` does when the program contains "roughness"), and a function building the program text from the four sliders. The programs follow the known-good one in `engine/web/dist-bevy-gpu/worlds/aim.txt` (`let t_steel = "s = fbm(scale_x=9, ...); ... albedo = ...; metallic = ...; roughness = ...; height = g"`): `name = node(param=..., gradient=[0:#hex, 1:#hex])` statements separated by `;`, the last statements named `albedo`, `roughness`, `metallic`, `height`, `emissive` (as many as the preset needs). Node names that are known to work are in `src/engine/texture/catalog.ts` `CURATED` (fbm, perlin, voronoi, bricks, colorize, blend, surface_curvature, surface_cavity, surface_ao, surface_up, surface_height, ...); read it and read how existing worlds in `engine/web/dist-bevy-gpu/worlds/*.txt` write `bricks`, `blend` and `colorize` calls. Use only curated names.
- **Layers by slider**, built on the base: `wear` paints edge wear from `surface_curvature` (lighter, rougher on edges), `dirt` darkens `surface_cavity` or `surface_ao` creases, `moss` blends a green layer where `surface_up` is high (only in the albedo, roughness and height), `wet` darkens the albedo a little and multiplies the roughness down (no mesh signal needed). **With wear, dirt and moss at 0 the program must not contain the text `surface_`** (a plain tiled program can be shared by many parts as one map; a mesh-reading one cannot): build the layer statements only for sliders above 0. Wet at any value must not add a mesh signal.
- Keep every program under 8,192 bytes and deterministic (no randomness outside seeded noise nodes, no time).
- In `ve-graph`: the operator spec in `builtin/modeling/appearance.rs` next to `solid.texture@1`, the ready list in `builtin/ready.rs`, and the estimate in `ve-modeling/src/plan/estimate.rs` (`Ok(0)` like `solid.texture`). Register the evaluation in `operations.rs` like `solid.texture`.

## Tests (written by Claude; never weaken; add more if you like): `crates/ve-modeling/tests/material_preset.rs`

Nine tests: all 14 presets set a textured surface and keep the shape (balanced brackets, under 8,192 bytes, albedo and roughness channels, metals metallic); unknown name refused; default tile 1.0 and size 512; each slider changes the program; determinism; no `surface_` at all zeros, with `surface_up` for moss and a cavity/ao signal for dirt; wet changes the program; sliders out of range refused; works under `solid.finish`. Also run `cargo test -q -p ve-graph` (update only entries for the new operator; never weaken a test).

## Check

`cargo test -q -p ve-graph`, `cargo test -q -p ve-modeling --test material_preset --test materials`, `cargo build -q -p ve-evaluator --target wasm32-unknown-unknown --release`.

## Status (Codex: write this section yourself when you stop; your final chat message is not saved)

- [x] done / [ ] blocked: implemented `material.preset@1` with 14 preset definitions, bounded sliders, conditional mesh-signal layers, tiled map sizing, and passthrough handling for regular materials, regions, and instance prototypes. Added graph registration and zero-cost estimation. Pre-existing edits were present in several touched files; preserved them.
- Tests run and their output:
  - `CARGO_TARGET_DIR=/tmp/codex-target-preset cargo test -q -p ve-graph` — passed (all test binaries).
  - `CARGO_TARGET_DIR=/tmp/codex-target-preset cargo test -q -p ve-modeling --test material_preset --test materials` — passed (9 material_preset tests, 7 materials tests).
  - `CARGO_TARGET_DIR=/tmp/codex-target-preset cargo build -q -p ve-evaluator --target wasm32-unknown-unknown --release` — passed.
  - Build/test output included unrelated existing warnings in `ve-geometry` and `ve-modeling` (`toward`, `Tile::origin`, and unreachable `Finish` re-export).
- Files changed for this work: `crates/ve-graph/src/builtin/modeling/appearance.rs`, `crates/ve-graph/src/builtin/ready.rs`, `crates/ve-modeling/src/appearance.rs`, `crates/ve-modeling/src/lib.rs`, `crates/ve-modeling/src/material_presets.rs`, `crates/ve-modeling/src/operations.rs`, `crates/ve-modeling/src/plan/estimate.rs` (several were already modified in the worktree), and this handoff Status.
- Node names or parameters you were unsure of in the programs: none; used curated `fbm`, `bricks`, `weave`, `colorize`, `blend`, `surface_convex`, `surface_cavity`, and `surface_up` calls with parameter forms present in the repository catalog/examples. Programs still need Claude's real-viewer inspection as noted above.
