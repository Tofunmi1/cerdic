# Handoff: `VeMaterial` reachable by reflection

Owner: Claude manages; Codex implements. **Crate you may edit: `ve-render` only** (`src/material.rs`, `src/surface_terrain.rs`, `src/surface_water.rs`, `Cargo.toml` for dependencies, and the export list in `src/lib.rs` if a type must be exported). Do not touch other crates (Claude registers the type in the runtime and allows writing it).
Rules: never run `git checkout`, `git clean`, `git stash` or `git restore`; never `cargo fmt` (use `rustfmt --edition 2024 --config-path rustfmt.toml <file>`); never `cargo xtask`; use exactly `CARGO_TARGET_DIR=/tmp/codex-target-matreflect`; do not commit or stage; edit with your edit tools, not scripts; leave `[lints]` alone and fix clippy findings in your own code. ve-render compiles for wasm (`cargo check -q -p ve-render --target wasm32-unknown-unknown` must still pass).

## Why

The runtime world API (ADR 0065) reaches components by name through `bevy_reflect`. `VeMaterial` (a part's surface: colour, metallic, roughness, glow, finish, toon, foliage) is not `Reflect`, so a script cannot recolour a part or make it glow when clicked. Making it reflectable lets a model do that with `world.insert_components` and a script do it every frame.

## What to build

- Derive `bevy_reflect::Reflect` and add `#[reflect(Component)]` (and `Default` where it has one: `#[reflect(Component, Default)]`) on `VeMaterial`, and derive `Reflect` on every type it contains: `ProceduralRecipe`, `ToonStyle`, `FoliageStyle`, `SurfaceFinish`, `SurfaceTerrain`, `SurfaceWater` and anything those hold. Add `bevy_reflect = { path = "../../third_party/bevy/crates/bevy_reflect" }` to `[dependencies]` (and `serde`, `serde_json`, `bevy_reflect` to `[dev-dependencies]` as the test needs: `serde = { version = "1" }`, `serde_json = "1"`, use the workspace entries if there are any).
- **Serialisation must round-trip through `TypedReflectSerializer` and `TypedReflectDeserializer`** (the test does exactly that): that needs `Color`, `u64`, arrays, `Option<...>` and the enums to have reflected serde support. If a field type cannot be serialised that way (a type with no `ReflectSerialize`/`ReflectDeserialize`), either give it the data (`#[reflect(Serialize, Deserialize)]` with serde derives on our own types) or, if it is not something a script should touch, `#[reflect(ignore)]` it **and then the type must implement `Default` for the ignored field** (say which in your Status). Keep every parameter a model would want to change visible: `base_color`, `opacity`, `metallic`, `roughness`, `emissive`, `recipe`, `texture_scale`, `toon`, `foliage`, `finish` (with `clearcoat`, `wetness` and the rest).
- Enums are written by variant name (`"Marble"`). Keep `VeMaterial: Copy` and its existing derives and behaviour; this is additive. Do not change any field name or type; the test and the JSON shape depend on them.
- The existing `ve-render` tests, and `ve-bevy` (`cargo test -q -p ve-bevy`, which uses `VeMaterial`), must still pass.

## Tests (written by Claude; never weaken): `crates/ve-render/tests/material_reflect.rs`

6 tests: registered with its parts; JSON names; round trip of a painted material and the default; colours changed through JSON; wrong types and unknown variants refused; the optional terrain and water layers survive.

## Check

`cargo test -q -p ve-render --test material_reflect`, `cargo test -q -p ve-render` (the whole crate), `cargo test -q -p ve-bevy`, `cargo check -q -p ve-render --target wasm32-unknown-unknown`, `cargo clippy -q -p ve-render --lib -- -D warnings` (a `too_many_lines` error in `src/lib.rs` `add_render_systems` is older and not yours).

## Status (Codex: write this section yourself when you stop; your final chat message is not saved)

- [x] done / [ ] blocked: `VeMaterial` and its nested material types are reflected and serde-round-trippable; `finish` (including terrain and water options) is part of the material.
- Tests run and their output:
  - `CARGO_TARGET_DIR=/tmp/codex-target-matreflect cargo test -q -p ve-render --test material_reflect` — passed, 6 tests.
  - `CARGO_TARGET_DIR=/tmp/codex-target-matreflect cargo test -q -p ve-render` — passed, 123 unit tests and all integration tests.
  - `CARGO_TARGET_DIR=/tmp/codex-target-matreflect cargo test -q -p ve-bevy` — passed, 27 + 3 tests.
  - `CARGO_TARGET_DIR=/tmp/codex-target-matreflect cargo check -q -p ve-render --target wasm32-unknown-unknown` — passed.
  - `CARGO_TARGET_DIR=/tmp/codex-target-matreflect cargo clippy -q -p ve-render --lib -- -D warnings` — blocked only by the pre-existing `clippy::too_many_lines` in `src/lib.rs::add_render_systems` (108/80).
- Files changed: `crates/ve-render/Cargo.toml`, `crates/ve-render/src/material.rs`, `crates/ve-render/src/surface_terrain.rs`, `crates/ve-render/src/surface_water.rs`.
- Fields marked `reflect(ignore)`, if any, and why: none.
