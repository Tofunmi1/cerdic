# Handoff to Codex: the game running live in the viewer (`ve-runtime`)

Read `engine/AGENTS.md` first and follow it (no `unwrap`/`expect`/`panic!`/`println!` in library code, no `#[allow]`: `#[expect(lint, reason)]` on the
smallest item only, files under 400 lines, docs on public items, no commits, no `Co-Authored-By` or tool attribution). **Never run `cargo fmt`; format
only your files with `rustfmt --edition 2024 --config-path rustfmt.toml <file>` from `engine`.** Own `CARGO_TARGET_DIR=target-codex-live`
(a wasm build of `ve-runtime` is large; disk is tight, so build `--profile web-dev` into it once and delete it when you finish).
Context: ADR 0062 phase 3. `ve-game` (pure rules core, `Inputs.viewer`, `KindDef.part`) and the `game.*` nodes are done; `Evaluation::game` is
printed by the evaluator worker as `GameInfo { present, text, summary, diagnostics }` (see `src/engine/worker-types.ts`, `evaluation.ts`).

## Task
Run the game in the runtime and draw it.
1. **Delivery.** In `src/engine/evaluation.ts`, when a result is accepted, call a new runtime export `game_load(text: &str) -> String` with the game's text
   (empty string when there is none). It returns `""` on success or the parse error text. Do the TypeScript edit with the Edit tool; keep it small
   (the `Runtime` type in `src/engine/bridge.ts` gains the method). Loading a game resets it (new `World`, seed 1).
2. **Stepping.** New module `crates/ve-runtime/src/game.rs` (split if over 400 lines): a resource holding the `ve_game::World` and the definition; a
   fixed 1/60 s accumulator system (cap catch-up at 4 steps per frame) that builds `Inputs { viewer: Some(feet), .. }` where `feet` is the viewer's eye
   position minus the rig's standing eye height (1.6 m default), x and z as is, and steps the world. Stops stepping when the status is not `Playing`.
   Effects returned by steps are kept in a small list (last 32) for `game_status`; executing them (audio, particles) is a later task.
3. **Drawing.** For each game entity whose kind names a `part`, draw that part at the entity's position: reuse the mesh and material of the authored
   scene part with that output name (see `projection/part_1.rs`, `Authored`; it has the output name) as a separate Bevy entity per game entity;
   hide the authored original while a game binds its part (it is the prototype). Despawn the drawing when the game entity dies or the game is
   unloaded, and restore the original's visibility. Handle the scene being re-projected (new revision): the drawings are rebuilt from the new parts.
   An unknown `part` draws nothing (no panic, no error spam).
4. **Status.** Export `game_status() -> String`, JSON: `{ "present": bool, "status": "playing"|"won"|"lost", "steps": u64, "vars": {name: number},
   "entities": [{ "id": u32, "kind": str, "part": str|null, "position": [x,y,z] }], "drawn": number, "effects": [..] }`
   (`present: false` and empty fields when no game is loaded; `drawn` is the number of Bevy entities currently drawing game entities).

## Contract (written first, do not weaken)
`engine/web/tests/game-live.mjs`: a coin game whose hero follows the viewer; the viewer walks into two coins and wins; replacing the document
with one without a game unloads it and removes the drawings. Run it against a viewer build:
```
cd engine
CARGO_TARGET_DIR=target-codex-live cargo build -p ve-runtime --target wasm32-unknown-unknown --profile web-dev --features bevy-webgpu
# then wasm-bindgen and copy as in the rebuild script (target/tools/bin/wasm-bindgen --target web, replace "from \"wasi_snapshot_preview1\"" with "from \"./wasi_snapshot_preview1.js\"",
# copy ve_runtime* into web/dist-bevy-gpu/; bundle the page code: cd .. && bun build src/engine/viewer-bridge.ts --outfile engine/web/dist-bevy-gpu/viewer-bridge.js --target browser --format esm)
cd web && VE_DIST=dist-bevy-gpu VE_CHANNEL=chromium bun tests/game-live.mjs
```
A ready-made script that does the build steps into the *shared* `target/` is `/private/tmp/claude-501/-Users-mac-Work-victoria-vr-hack/9d098cda-d740-48c5-ac1e-a0a723c575c1/scratchpad/rebuild-viewer-shared.sh`
(copy and change its `CARGO_TARGET_DIR` to yours only if you must; the shared `target/` already has most of the dependencies built, so prefer it:
use `CARGO_TARGET_DIR=target` and only your changes recompile). Also run `cd engine/web && VE_DIST=dist-bevy-gpu VE_CHANNEL=chromium bun tests/mcp-tools.mjs` (must still pass).
`cargo clippy -p ve-runtime --target wasm32-unknown-unknown -- -D warnings` must add nothing new for your files.

## Status

Implemented live game delivery, fixed-step viewer input, status/effect reporting, and authored-part drawings. `World` now owns its parsed definition so the runtime can retain both safely. The drawing sync hides bound prototypes, restores their previous visibility when unbound, and replaces drawings when projected parts change.

Checks run:

- `cargo test -p ve-game` — passed (28 integration tests).
- `cargo check -p ve-runtime --target wasm32-unknown-unknown --profile web-dev --features bevy-webgpu` — passed.
- `cargo build -p ve-runtime --target wasm32-unknown-unknown --profile web-dev --features bevy-webgpu` — passed; wasm-bindgen and the viewer bundle completed.
- `cargo clippy -p ve-runtime --target wasm32-unknown-unknown -- -D warnings` — blocked by existing workspace lint failures in other files; the new `game.rs` findings were fixed. Existing runtime lint failures remain in unrelated files.
- `cargo xtask quick ve-game ve-runtime` — stopped at its workspace `cargo fmt --all --check`, which reports extensive pre-existing formatting differences, including vendored Bevy files.
- `cargo xtask rules` — failed on pre-existing oversized files: `crates/ve-graph/src/builtin/fields.rs`, `crates/ve-graph/src/builtin/world.rs`, and `crates/ve-modeling/src/human_hair.rs`.
- `VE_DIST=dist-bevy-gpu VE_CHANNEL=chromium bun tests/game-live.mjs` and `bun tests/mcp-tools.mjs` — could not start their local static server: sandbox denied loopback `listen` with `EPERM` (the runner reports this as “port 0 in use”). No browser assertions ran.

The handoff’s game-live contract remains the right test; it was not weakened. The strict milestone gate is pending per `engine/AGENTS.md`.
