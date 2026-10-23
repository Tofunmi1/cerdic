# Handoff to Codex: the HUD drawn live in the viewer (`ve-runtime`)

Read `engine/AGENTS.md` first and follow it (no `unwrap`/`expect`/`panic!`/`println!` in library code, no `#[allow]`: `#[expect(lint, reason)]` on the
smallest item only, files under 400 lines, docs on public items, no commits, no `Co-Authored-By` or tool attribution). **Never run `cargo fmt`; format
only your files with `rustfmt --edition 2024 --config-path rustfmt.toml <file>` from `engine`.** Build only into the shared `target/` with `CARGO_TARGET_DIR=target`
(disk is tight; a private wasm build dir is several GB).
Context: `ve_game::GameDef.huds`, `World::hud_lines()` and `ve_world::pixel_text::layout` exist (see `docs/handoff-codex-hud.md`). The live game is `crates/ve-runtime/src/game.rs`.

## Task
Draw the HUD of the loaded game as geometry in front of the viewer.
- For each `HudDef` build (or rebuild when `World::hud_lines()[i]` changes) one mesh from `pixel_text::layout(line, hud.size)`: one quad (two triangles, double-sided or
  facing the viewer) per rect, centred on the line's extent, drawn with an unlit emissive material (bright off-white, around `[1.0, 0.95, 0.8]`, no shadows, no AO) so it reads in any light.
  Use the same material machinery as other runtime meshes (see `projection/` and how `game.rs` draws parts); do not add a font or texture asset.
- Placement: a world-space panel at about 1.3 m in front of the eye. `gaze`: slightly below the view centre; `left`/`right`: to the side (about 0.45 m); `top`/`bottom`: above/below.
  It faces the viewer and follows the viewer's yaw with a slow lag (turn rate about 90 degrees per second) so it does not stick to the head; it does not tilt with head pitch.
  Use the same viewer pose source as the hero's feet in `game.rs` (`crate::viewer::pose()` or the rig).
- The panel hides when no game is loaded or when the game has no huds; it stays visible after the game is won or lost (the last values remain).
- Status: `game_status()` JSON gains `"hud": { "lines": [string...], "triangles": number }` (`lines` from `hud_lines()`, `triangles` the total drawn for the panel(s); `{"lines": [], "triangles": 0}` when none).
- Rebuild only when a line's text changed (cheap per frame otherwise).

## Contract (written first, do not weaken)
`engine/web/tests/game-live.mjs` (the `hud` assertions: the panel exists with its triangles, text follows the variable). Build and run as in `docs/handoff-codex-game-live.md`
(the rebuild script is `/private/tmp/claude-501/-Users-mac-Work-victoria-vr-hack/9d098cda-d740-48c5-ac1e-a0a723c575c1/scratchpad/rebuild-viewer-shared.sh`; it uses the shared `target/`),
then `cd engine/web && VE_DIST=dist-bevy-gpu VE_CHANNEL=chromium bun tests/game-live.mjs` and `bun tests/mcp-tools.mjs`. If the sandbox forbids the local server, say so in Status and give what you could verify
(the assistant that wrote this runs the browser tests itself). Also take a screenshot of the HUD if you can run the browser and describe how it looks.
`cargo clippy -p ve-runtime --target wasm32-unknown-unknown -- -D warnings` must add nothing new for your files.

## Status
Implemented live HUD panels in `ve-runtime`. Each line is laid out with `ve_world::pixel_text::layout` and drawn as one emissive mesh with two triangles per lit pixel. Panels follow viewer position, stay upright, and turn toward the viewer with a 90°/s yaw limit. Meshes are rebuilt when expanded text changes and removed when the game or its HUD is cleared. `game_status()` now includes expanded HUD lines and the total triangle count; `ve-game::World::huds()` exposes the definitions to the renderer.

Checks run:
- `CARGO_TARGET_DIR=target cargo check -p ve-runtime --target wasm32-unknown-unknown` — passed.
- The shared rebuild script, invoked with `bash` because it is not executable — passed; it rebuilt `web/dist-bevy-gpu`.
- `cargo xtask quick ve-runtime ve-game` — stopped at its repository-wide formatting stage; existing formatting diffs are present across the workspace and vendored Bevy. `cargo xtask rules` — stopped on pre-existing oversized files in `ve-graph` and `ve-modeling`; the changed runtime/game files are below 400 lines.
- `VE_DIST=dist-bevy-gpu VE_CHANNEL=chromium bun tests/game-live.mjs` — could not start the local server (`Failed to start server. Is port 0 in use?`), before test assertions ran. No browser screenshot was available.
- `bun tests/mcp-tools.mjs` — same local-server startup failure, before test assertions ran.
- `CARGO_TARGET_DIR=target cargo clippy -p ve-runtime --target wasm32-unknown-unknown -- -D warnings` — stopped in existing `ve-human/src/measure.rs` on `many_single_char_names`, before reaching `ve-runtime`. A `--no-deps` diagnostic run reports 35 existing strict-lint failures in runtime files outside this change and none in the HUD/game/world files changed here; the HUD `clone_from` lint found in the first diagnostic run was fixed.

The browser contract remains unverified because both scripts were unable to bind their local server. The full strict lint result is pending cleanup of the pre-existing Clippy failures.
