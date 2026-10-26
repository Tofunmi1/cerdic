# Handoff: general names for rolling things and hand-held tools, and their nodes (ADR 0068)

Owner: Claude manages; Codex implements. **Crates you may edit: `ve-game`, `ve-graph` (`src/builtin/game.rs`, `src/builtin/game/*.rs`, `ready.rs`), `ve-modeling` (`src/game_nodes.rs`, `src/game_nodes/*.rs`).** Not `ve-runtime` (Claude updates it).
Rules: never run `git checkout`, `git clean`, `git stash` or `git restore`; never `cargo fmt` (use `rustfmt --edition 2024 --config-path rustfmt.toml <file>`); never `cargo xtask`; use exactly `CARGO_TARGET_DIR=/tmp/codex-target-rolling`; do not commit or stage; edit with your edit tools, not scripts (a move of a file you created is fine); leave `[lints]` alone; files stay under 400 lines. Tests are written: never weaken them.

## Why

The owner's point: golf is an example game, not a part of the engine. Names in the engine must be general (billiards, bowling, hockey, tennis, mallets, pinball all use the same things). The physics and text format from the last round (ground field, rolling things on surfaces, swept hand-held swings, pockets) stay exactly as they are; only the names change, then the nodes get built under the general names.

## Part 1: rename (no behaviour change)

| Old | New |
|---|---|
| kind key `strike=1` | `swing=1` |
| event `strike` (a = tool, b = ball) | `swing` |
| kind key `cup=1` | `pocket=1` |
| event `sink` (a = pocket, b = ball) | `pocket` |
| any "golf", "club", "cup" in identifiers, module and file names, comments, diagnostics, suggestions, docs | neutral words: "rolling", "swing", "tool", "pocket" (examples may still say "a club" or "a cup" as examples of a tool or pocket) |
| `src/parse/golf.rs`, `src/parse/golf/`, `src/golf_tests.rs` | `src/parse/rolling.rs`, `src/parse/rolling/`, `src/rolling_tests.rs` (and the `mod` lines) |
| `hash.rs` / snapshot wording that says golf | neutral |

Keys that already read well stay: `roll`, `reach`, `loft`, `power`, `terrain`, `surface`, the event `out`, `GroundField`, `HandRotation`, `Entity::orientation`. `describe()` writes the new keys; the old keys are no longer accepted (nothing in the repo uses them but the tests, which are already updated). Event names accepted in `on=` must list `swing` and `pocket` instead of `strike` and `sink` (see `parse/helpers.rs`). Also update any rustdoc, and `engine/docs/handoff-codex-golf.md` is history: leave it.

## Part 2: nodes (`ve-graph` and `ve-modeling`)

- `game.kind@1` gets optional parameters `roll` (bool), `swing` (bool), `reach` (number >= 0), `loft` (number 0 to 80), `power` (number > 0, at most 5), `pocket` (bool), written as `roll=1`, `swing=1`, `reach=M`, `loft=DEG`, `power=F`, `pocket=1` only when given. Look at how `fall` and `bounce` were added.
- `game.terrain@1(parts=[names], cell=M)` writes `terrain parts=a,b cell=M` (default cell 0.25, > 0; a second terrain node is a problem naming it).
- `game.surface@1(name, parts=[names], decel, bounce)` writes `surface NAME parts=a,b decel=D bounce=B` (parts non-empty; decel > 0; bounce 0 to 1, default 0.5); surfaces come out in the order `game.collect@1` received them (that order is their ids).
- All new nodes in `ready()` with help text (one working example each; say `parts` are the output names of authored parts; phrase examples with general games such as a rolling ball on a table or a field, not only one sport), `game.collect@1` accepts them; problems name the node as the other `game.*` nodes do.

## Tests (written by Claude; never weaken)

`crates/ve-game/tests/rolling.rs` (21), `crates/ve-modeling/tests/game_rolling_nodes.rs` (5). Also run `cargo test -q -p ve-game` (all must pass: 152 existed before the rename), `cargo test -q -p ve-graph`, `cargo test -q -p ve-modeling --test game_rolling_nodes --test game_agent_nodes --test g14_game_nodes --test game_spawn_heading`, `cargo clippy -q -p ve-game --all-targets -- -D warnings` (the four older findings in `parse/helpers.rs` and `world/state/actions.rs` exist: do not silence them, add no new ones). Finally `grep -rni "golf" crates/ve-game/src crates/ve-graph/src crates/ve-modeling/src` must print nothing.

## Status (Codex: write this section yourself when you stop)

- [ ] done / [x] blocked: Both implementation parts are complete, in order. Part 1 was tested before the node changes: the physics calculations, thresholds, integration order, and legacy digest are unchanged. Part 2 supplies the general kind capabilities, terrain/surface fragments, validation, help/examples, and collection order. Terrain and surface were already in `ready()` and remain there. Completion is blocked by the one test/formatter mismatch below; no existing test was weakened or changed except the required neutral renames of the old source unit-test module and fixtures. `ve-runtime` and the historical handoff were left alone.
- Tests run and their output (all Cargo commands used `CARGO_TARGET_DIR=/tmp/codex-target-rolling`):
  - After part 1: `cargo test -q -p ve-game`: **153 passed, 0 failed** (152 existing tests plus one regression rejecting the obsolete kind keys/events). Includes the unchanged **21/21** rolling integration tests. Also ran `cargo test -q -p ve-game --test rolling` separately: **21 passed, 0 failed**.
  - `cargo test -q -p ve-graph`: **138 passed, 0 failed**, both after the rename and after the graph file split/help changes.
  - `cargo test -q -p ve-modeling --test game_rolling_nodes --test game_agent_nodes --test g14_game_nodes --test game_spawn_heading`: `g14_game_nodes` **15/15**, `game_agent_nodes` **4/4**, `game_rolling_nodes` **4/5**; Cargo stopped at that failed target. Ran `game_spawn_heading` separately: **1/1**. Repeated the full requested selection with `--no-fail-fast` after the final code changes: **24 passed, 1 failed** across all four targets.
  - **Test looks wrong: `crates/ve-modeling/tests/game_rolling_nodes.rs:38`, `the_nodes_write_the_rolling_lines_and_keys`.** It requires literal `bounce=0.3` on the ball line. The fixture supplies **0.3**, which is already the kind's default restitution; the existing `parse/format.rs::kind_text` omits it when `fall` is false and bounce equals **0.3**. Actual canonical line: `kind ball size=0.03 speed=0 roll=1`. The parsed bounce remains **0.3**, and canonical round-trip passes. Changing the default, enabling falling, or changing the formatter merely to satisfy this assertion would change existing behavior. Left the test and formatter behavior unchanged as instructed.
  - `cargo clippy -q -p ve-game --all-targets -- -D warnings`: exit **101**, exactly the **4** known findings, no new ones: `parse/helpers.rs:109` `validate_action` has **84/80** lines; `world/state/actions.rs:187` `effects` has **104/80** lines; collapsible `if` statements at `actions.rs:216` and `:217`. None silenced.
  - Added four node regressions for defaults/omitted capabilities, reversed nested surface collection order, the second terrain naming its node, and invalid cell/power/empty part names naming their nodes. `cargo test -q -p ve-modeling --lib game_nodes::rolling` is blocked by existing **E0428**: duplicate `tests` modules in `src/anim/part_2.rs:154` and `src/anim/drop.rs:286` (outside this task's allowed files). Compiled those same four regression tests with `rustc --test` through `/tmp/codex-rolling-regressions.rs`, linking the built `ve-modeling`, `ve-graph`, and `ve-game` libraries under the required target directory: **4 passed, 0 failed**.
  - `grep -rni "golf" crates/ve-game/src crates/ve-graph/src crates/ve-modeling/src`: **no output** (exit 1, no matches). Scoped `git diff --check`: clean. Every changed Rust file is below **400** lines (largest: `world/state.rs`, **392**).
  - Modeling builds also print existing warnings in `ve-geometry/src/field/tape/eval.rs` (`toward` unused), `ve-modeling/src/world_terrain.rs` (`origin` unused), and `ve-modeling/src/result.rs` (unreachable `pub`). Left those files alone. Strict gate pending; no `cargo xtask`, `cargo fmt`, staging, or commits were run.
- Files changed:
  - `ve-game/src/ground.rs`, `lib.rs`, `parse.rs`; `parse/format.rs`, `parse/helpers.rs`, `parse/reader.rs`; moved `parse/golf.rs` to `parse/rolling.rs`, `parse/golf/fuzz.rs` to `parse/rolling/fuzz.rs`, and `golf_tests.rs` to `rolling_tests.rs` with neutral names/fixtures and unchanged existing assertions; added obsolete-key/event rejection coverage there.
  - `ve-game/src/world/state.rs`, `world/state/hash.rs`, `world/state/rolling.rs`; moved `world/state/club.rs` to `world/state/swing.rs`, renaming tool state and swing/pocket events without changing physics.
  - `ve-graph/src/builtin/game.rs`; added `builtin/game/kind.rs` to keep files below 400 lines. General names and working examples; existing terrain/surface readiness retained.
  - `ve-modeling/src/game_nodes.rs`; added `game_nodes/rolling.rs` and `game_nodes/rolling/tests.rs`. Kind fields, terrain/surface validation, and the four regressions. Existing collection logic already preserves surface order and rejects a second terrain with its node label.
  - This handoff's Status section.
