# Handoff: which way a game entity faces

Owner: Claude manages; Codex implements. **Crate you may edit: `ve-game` only** (`src/`), and `ve-graph` only for the one node spec line named below. Do not touch `ve-runtime` (Claude wires the drawing to it).
Rules: never run `git checkout`, `git clean`, `git stash` or `git restore`; never `cargo fmt` (use `rustfmt --edition 2024 --config-path rustfmt.toml <file>`); never `cargo xtask`; use exactly `CARGO_TARGET_DIR=/tmp/codex-target-heading`; do not commit or stage; edit with your edit tools, not scripts; leave `[lints]` alone. Tests are written; never weaken them.

## Why (ADR 0066)

A game entity is drawn as a copy of the part its kind names. To draw an animated model (a guard with legs and a state machine) the runtime has to turn it to face where it walks. `ve-game` knows where an entity moved; it must say which way it faces.

## What to build

- `Entity` gains `pub heading: f64`: yaw in radians about +Y, 0 = facing +Z, `atan2(dx, dz)` of the displacement of the last step in which the entity moved (by any means: follow, velocity, a rule). It is unchanged on a step where the entity moved less than 1e-9 in the horizontal plane. Initial value 0, or the spawn's.
- `spawn KIND at=x,y,z heading=DEGREES` (optional; also on `count`/`spread` spawns, all copies get it): degrees converted to radians; a value that is not a finite number is a diagnostic naming `heading` (with a suggestion) on that line. Update the spawn line's help/format text wherever the other spawn keys are documented in `ve-game`.
- `heading` is **derived data: do NOT include it in `World::hash()` or the snapshot used by determinism tests** (existing golden hashes must not change; run all `ve-game` tests to prove it). It must still be restored sensibly when a snapshot is restored (any value that keeps later steps deterministic: restore leaves it as is).
- `ve-graph`: the `game.spawn` node (see `crates/ve-graph/src/builtin/game.rs`) gets an optional `heading` number parameter (degrees, default 0, range -360..360) that is written to the spawn line; a test in ve-graph's existing game node tests that a spawn with `heading=90` produces `heading=90` in the game text (find where `game.spawn` is tested and follow it).

## Tests (written by Claude; never weaken): `crates/ve-game/tests/heading.rs`

6 tests. Also run: `cargo test -q -p ve-game` (all, including the older golden/hash tests), `cargo test -q -p ve-graph` (a few unrelated older failures may exist; name any you see and show they fail without your change by saying which test, not by reverting).

## Status (Codex: write this section yourself when you stop; your final chat message is not saved)

- [ ] done / [x] blocked: the allowed crate list excludes `ve-modeling`, but `ve-modeling/src/game_nodes.rs::make_spawn` is where `game.spawn` actually emits the game `spawn` line. The heading parameter is registered in `ve-graph`, but cannot reach the game text without editing that excluded emitter. The stated `ve-graph` game node test location also does not exist in this checkout; existing game node integration tests are under `ve-modeling/tests/g14_game_nodes.rs`.
- Tests run and their output: `CARGO_TARGET_DIR=/tmp/codex-target-heading cargo test -q -p ve-game` passed (all suites, including 6 heading tests and existing golden/hash tests); `CARGO_TARGET_DIR=/tmp/codex-target-heading cargo test -q -p ve-graph` passed (all suites).
- Files changed: `crates/ve-game/src/parse.rs`, `crates/ve-game/src/parse/reader.rs`, `crates/ve-game/src/world/state.rs`, `crates/ve-game/src/world/state/actions.rs`, `crates/ve-graph/src/builtin/game.rs`.
