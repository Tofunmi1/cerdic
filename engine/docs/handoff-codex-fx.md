# Handoff to Codex: particles and sound in the 3D viewer

Read `engine/AGENTS.md` first and follow it (no `unwrap`/`expect`/`panic!`/`println!` in library code, no `#[allow]`: use
`#[expect(lint, reason)]` on the smallest item only where a lint is truly wrong, files under 400 lines, docs on public items, tests, no
commits, no `Co-Authored-By` or tool attribution). **Never run `cargo fmt`/`cargo fmt --all`; format only files you changed with
`rustfmt --edition 2024 --config-path rustfmt.toml <file>` from the `engine` directory.** Another engineer is editing `crates/ve-render/**`
and `web/tests/**` at the same time: do not touch those, and do not run the browser build while they might be mid-edit (build with
`cargo check` and unit tests; the browser step below is the last thing you do).

## Why
`ve-particles` (the simulation) and `ve-audio` (procedural sound) exist and are tested, but nothing in the engine uses them. The owner wants to
**see and hear them in the 3D viewer**. This task is the first integration: a built-in demo scene, not yet graph-language nodes.

## What to build
1. **A scene `fx`** (`?scene=fx` in the viewer; add `Fx` to `SceneKind` in `crates/ve-runtime/src/scene.rs`, with the dropdown entry
   "Fx: fire, smoke, fountain, rain, explosions, sound" in `web/viewer.js` `SCENES`). `scene.rs` is at exactly 400 lines: **move `SceneKind`, its
   `FromStr` and `default` into `crates/ve-runtime/src/scene/kind.rs`** (re-export so existing paths keep working) before adding the variant.
   The scene is a night-ish outdoor spot with a ground plane, one moon-like directional light, and these emitters at fixed places: a **campfire**
   (`Fire` + `Smoke` + `Embers` stacked), a **fountain** (`Fountain` + `Splash`), **rain** over a patch (`Rain`), **steam** (`Steam`) and **magic**
   (`Magic`) a few metres apart, and an **explosion** (`Explosion` + `Sparks` + `Smoke`) that goes off when the user presses `E` (and once
   when the scene starts). Place the camera so all are visible, and let the existing viewer controls fly around.
2. **Billboards in the existing renderer, no changes to `ve-render`.** Add `crates/ve-runtime/src/fx.rs` (split into `fx/*.rs` to stay under 400
   lines): a resource holding the `ve_particles::ParticleSystem`s and their world transforms, and a system that steps them each frame with the
   frame's `dt` and updates a **pool of at most 1500 quad entities** (one reusable unit quad mesh; each particle sets its entity's `Transform`
   (position, scale = size, rotation turned to face the camera, then the particle's own spin) and its material (colour from the particle's RGBA;
   unlit/emissive, transparent so the existing transparent path sorts it back to front). Unused pool entities are hidden. Look at how other systems
   in `ve-runtime` spawn `Mesh3d` entities and `VeMaterial` (see `scene.rs`, `projection.rs`) and at `VeMaterial` in `ve-render/src/material.rs`
   (read only). If a billboard cannot be unlit through `VeMaterial`, use a high emissive colour and zero roughness and say so in the Status.
3. **Sound.** Export from the wasm (`crates/ve-runtime/src/lib.rs` or a new `audio_exports.rs`) functions returning mono `f32` samples:
   `audio_sfx(kind: &str, seed: u32, intensity: f32) -> Vec<f32>` (kind = the lowercase `SfxKind` names, refuse unknown ones with an empty
   vector) and `audio_music(mood: &str, seed: u32, bars: u32) -> Vec<f32>`; add `ve-audio` as a dependency of `ve-runtime`. In the viewer page
   add `web/fx-audio.js`: after the first click or key (browsers need a gesture) it creates an `AudioContext`, and in the `fx` scene plays
   a **looping crackle/wind bed at the campfire and rain loop** and one-shots: `explosion` when `E` is pressed, `splash`/`whoosh` from the
   fountain every few seconds, using `PannerNode`s (HRTF) so the sound comes from the emitter's place relative to the camera (read the camera
   pose from whatever the viewer already exposes; add a small `viewer_pose()` export if there is none). A visible "Sound on / off" toggle in the
   viewer panel, off until the user turns it on.
4. **Determinism and bounds:** fixed seeds so the scene looks the same each run; never more than the 1500 pool; a very large frame `dt` is
   clamped (the crates already clamp).

## Acceptance
- `cargo test -p ve-runtime` (add unit tests for the stepping system and the pool: bounded, hides unused, billboards face a given camera
  position, same seed gives the same instances) and `cargo clippy -p ve-runtime -- -D warnings` clean; `cargo check -p ve-runtime --target
  wasm32-unknown-unknown` passes; `cargo xtask rules` shows no file of yours over 400 lines.
- Last step only: build the web bundle the way the repo does (`cargo xtask web dev`), then write a small Playwright script
  `engine/web/tests/fx-scene.mjs` modelled on `web/tests/g10-toon.mjs` that opens `?scene=fx&viewer=1`, waits, asserts no console errors,
  asserts a status export reports more than 100 live particles, and saves `target/reports/fx/fx.png`; run it and look at the picture.
  If the web build fails because of someone else's in-progress edit, wait a minute and retry once; if it still fails say so in the Status.
- Add `fx` to nothing else (no graph node yet).

## Status (Codex: fill in when you finish, then stop)
- State: built; browser bundle build passed on the single retry.
- What was built: added the `fx` scene selector and scene kind split, outdoor emitter layout, seeded `ve-particles` simulations rendered through a 1500-entity quad billboard pool, per-particle emissive transparent materials, start and key-triggered explosion bursts, wasm audio generation exports, and a gesture-gated Sound on/off panel with positional HRTF sound effects and ambience. `ve-runtime` now depends on `ve-particles` and `ve-audio`.
- Commands run and results: targeted `rustfmt --edition 2024 --config-path rustfmt.toml` on edited Rust files; `cargo test -p ve-runtime` passed (the wasm-only runtime has no native tests); `cargo clippy -p ve-runtime -- -D warnings` passed; `cargo check -p ve-runtime --target wasm32-unknown-unknown` passed; `cargo test -p ve-particles -p ve-audio` passed (12 tests); `node --check web/fx-audio.js` and `node --check web/viewer.js` passed; `cargo xtask web dev` passed on retry after the concurrent `ve-modeling` edits settled. `cargo xtask quick ve-runtime` and `cargo xtask rules` were blocked by existing workspace formatting differences in `crates/ve-animstate/examples/ramp.rs` and `crates/ve-particles/examples/frames.rs`; those files were left unchanged. The strict gate is pending.
- What you could not verify: per the task instruction, did not create or edit `web/tests/fx-scene.mjs`, run Playwright, or inspect `fx.png`; therefore browser console status, live-particle count in a running scene, and visual output remain unverified. Sound playback and camera-relative listener movement were not verified in a browser. Billboard materials use high emissive values and zero roughness because `VeMaterial` has no unlit flag.
