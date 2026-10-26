# Handoff: things that fall and can be thrown (ADR 0067)

Owner: Claude manages; Codex implements. **Crate you may edit: `ve-game` only** (`src/`). Do not touch other crates (Claude adds the `game.kind` node parameters afterwards).
Rules: never run `git checkout`, `git clean`, `git stash` or `git restore`; never `cargo fmt` (use `rustfmt --edition 2024 --config-path rustfmt.toml <file>`); never `cargo xtask`; use exactly `CARGO_TARGET_DIR=/tmp/codex-target-throw`; do not commit or stage; edit with your edit tools, not scripts; leave `[lints]` alone; files stay under 400 lines (new module `src/world/state/motion.rs` or similar, not one giant file). Tests are written: never weaken them.

## Why

"Blackout 2" is a hands-first VR game: you grab a fuse cell or a crate and throw it at a creature. Today a released item just hangs in the air and nothing falls. Hands, `grab=1`, `touch` rules and `setvel:` exist; gravity, ground, release velocity and bounce do not.

## What to build (see the test file's header for the contract)

- Kind keys: `fall=1` (or 0; anything else is a diagnostic naming `fall`) and `bounce=F` (0 to 0.95, default 0.3; otherwise a diagnostic naming `bounce`). World line `ground y=NUMBER` (default 0; a non-number is a diagnostic naming `ground`). Add them to `KindDef` / `GameDef` and to `describe()` so text round-trips.
- Each fixed step, for every entity whose kind has `fall=1` and which is **not currently held** by a hand: `v.y -= 9.8/60`; `position += v/60` (semi-implicit Euler, positions after velocity update); if `position.y - size < ground`: `position.y = ground + size`, `v.y = -v.y * bounce` (set to 0 if `|v.y| < 0.5`), and horizontal velocity `*= 0.9` per step while on the ground; if on the ground and the horizontal speed `< 0.02`, set horizontal velocity to 0. An entity at rest on the ground stays exactly at `ground + size` with velocity `[0,0,0]`. Entities without `fall=1` are completely unchanged (existing behaviour of `setvel:` and `move:` stays).
- Held items: no gravity while held (keep them following the hand as today). Keep, per held item, its positions over the last 5 steps **in which it was gripped** (a small fixed ring in the world state, included in snapshot/restore so rolling back a failed step is exact; the step in which the grip lets go does not add a position). On release (grip lets go, or the hand goes untracked), a **`fall=1`** item's velocity becomes `(last_gripped_position - position_4_gripped_steps_before) / (4/60)` using as many steps as exist (at least 1) if fewer than 5 are recorded. **An item that is not `fall=1` keeps today's behaviour exactly: on release it stays where it is with its velocity unchanged** (the existing test `hands.rs::letting_go_releases_the_item_where_it_is_and_it_stops_following` must pass unchanged).
- The previous attempt measured half the hand's speed (1.5 m/s for a 3 m/s swing): check that the ring holds distinct carried positions only, and that the velocity is taken from it once at release, then gravity and integration apply from the next step.
- `touch` between flying things and others already works from positions; make sure the order in a step is: hands (grab, carry, release) -> gravity/velocity integration -> touch detection, so a thrown item that reaches a target in a step raises `touch` in the same step.
- Determinism: the hash must change when anything above changes state (velocity is already hashed; include the position history ring and anything new). Same inputs, same hash.
- Existing tests must still pass unchanged, in particular the golden/hash tests: if a golden hash changes only because a **new** field is hashed, hash the new fields only when at least one kind has `fall=1` (so worlds without it keep their hashes) and say so in your Status.

## Tests (written by Claude; never weaken): `crates/ve-game/tests/throw.rs`

12 tests. Also run `cargo test -q -p ve-game` (everything) and `cargo clippy -q -p ve-game --all-targets -- -D warnings` (older findings may exist: name them, do not silence them).

## Status (Codex: write this section yourself when you stop; your final chat message is not saved)

- [ ] done / [x] blocked: implementation is mostly complete, but the touch test remains failing because the thrown ball's trajectory does not intersect the target's 3D collision radius. The unchanged test places the target at x=2.6, y=1.3; with a 4 m/s horizontal throw and gravity, the ball falls below that height before reaching it. Fixing this by changing general touch geometry would alter existing semantics.
- Tests run and their output:
  - `CARGO_TARGET_DIR=/tmp/codex-target-throw cargo test -q -p ve-game --test throw`: 11 passed, 1 failed (`a_thrown_thing_that_meets_a_target_raises_touch`; `hits` remains 0).
  - `CARGO_TARGET_DIR=/tmp/codex-target-throw cargo test -q -p ve-game --test hands`: 11 passed.
  - `CARGO_TARGET_DIR=/tmp/codex-target-throw cargo test -q -p ve-game`: failed only in the same touch test; all other crate tests passed.
  - `CARGO_TARGET_DIR=/tmp/codex-target-throw cargo clippy -q -p ve-game --all-targets -- -D warnings`: fails on older findings in `parse/helpers.rs` (`validate_action` too many lines) and `world/state/actions.rs` (`effects` too many lines and two collapsible-if findings). New throwing-related findings in `parse/reader.rs`, `hands.rs`, and `state.rs` were fixed.
- Implementation corrections: the release ring now contains one sample per gripped step (no second write during falling integration); only `fall=1` entities receive release velocity; missing/untracked hands release held items; release velocity is derived from the oldest and newest retained gripped positions; release-step gravity/integration starts on the following step. Hands regression tests pass unchanged.
- Files changed for this continuation: `engine/crates/ve-game/src/parse/reader.rs`, `engine/crates/ve-game/src/world/state.rs`, `engine/crates/ve-game/src/world/state/hands.rs`.
- Hash compatibility: history is only included in the world hash when at least one kind has `fall=1`.
