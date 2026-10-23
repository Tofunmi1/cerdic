# Handoff to Codex: the player as the hero, and kinds bound to scene parts (`ve-game`, `game.*` nodes)

Read `engine/AGENTS.md` first and follow it (no `unwrap`/`expect`/`panic!`/`println!` in library code, no `#[allow]`: `#[expect(lint, reason)]` on the
smallest item only, files under 400 lines, docs on public items, no commits, no `Co-Authored-By` or tool attribution). **Never run `cargo fmt`; format
only your files with `rustfmt --edition 2024 --config-path rustfmt.toml <file>` from `engine`.** Own `CARGO_TARGET_DIR=target-codex-follow`.
Context: ADR 0062 (phase 3), `docs/plans/game-world-system.md` 7.3. In VR the player *is* the hero: the game must know where the viewer stands,
and each kind must name the scene part that draws it so the runtime can show entities. This task is the pure-logic half; the Bevy runtime system comes next.

## Part 1: `ve-game` (contract: `crates/ve-game/tests/follow.rs`, written first, do not weaken)
- `Inputs.viewer: Option<[f64; 3]>` (derive stays `Default`, so `None`).
- Text: `input <name> follow=viewer target=tag:<tag>` (new key `follow`, the only accepted value is `viewer`; anything else is a located
  diagnostic on that line mentioning `follow`). During a step, before touches and rules are evaluated, every entity matching `target` is placed at the
  viewer position (`InputDef` gains a public `follow` field as appropriate; keep existing public API compatible, adding fields is fine).
- `KindDef.part: Option<String>` (public), text key `part=<name>` on `kind` lines, printed back by `to_text` and kept by `describe` if sensible.
- Snapshot and restore cover everything new (nothing new is stored except positions, which snapshots already hold).
- Existing `ve-game` tests stay green and clippy `--all-targets` stays clean.

## Part 2: `game.*` nodes (`ve-graph/src/builtin/game.rs`, `ve-modeling/src/game_nodes.rs`)
- `game.kind` gets optional `part` (str, the name of a scene output that draws this kind; default empty = invisible).
- `game.input` gets optional `follow` (`"viewer"` or empty).
- The collected definition carries both (through the text form of each item). Add tests to `crates/ve-modeling/tests/g14_game_nodes.rs`
  **only by adding new test functions at the end** (a coin game where the player kind has `part="body"` and an input follows the viewer, checked
  through `Evaluation::game`); the existing tests there are mine and stay unchanged.
- A kind whose `part` names no scene output of the document is not an error at collect time (parts are checked by `check`, later); note it in Status.

## Verify
```
cd engine
CARGO_TARGET_DIR=target-codex-follow cargo test -p ve-game -p ve-graph -p ve-modeling --no-fail-fast   # only g12_field_warp may fail (known debt)
CARGO_TARGET_DIR=target-codex-follow cargo clippy -p ve-game --all-targets -- -D warnings
CARGO_TARGET_DIR=target-codex-follow cargo check -p ve-game -p ve-modeling -p ve-evaluator --target wasm32-unknown-unknown
```

## Status
Implemented both parts: `ve-game` accepts `Inputs.viewer`, supports `follow=viewer` on targeted inputs before touch detection and rules, and preserves `kind part=...` in canonical text and descriptions. The `game.kind` and `game.input` nodes carry `part` and `follow` through collection into `Evaluation::game`. Scene part names are not checked at collection time; that validation remains for `check`.

Tests added to `crates/ve-modeling/tests/g14_game_nodes.rs` cover a player kind bound to `body`, viewer following, and collecting a coin through `Evaluation::game`. The existing `crates/ve-game/tests/follow.rs` contract passes, including located rejection of unsupported follow sources and snapshot restore.

Verification with `CARGO_TARGET_DIR=target-codex-follow`:

- `cargo test -p ve-game -p ve-graph -p ve-modeling --no-fail-fast`: all tests passed except the known unrelated `g12_field_warp` test target (8 failures; the `field.taper`/bend/twist operators are not registered or implemented in this workspace). All 5 follow tests and all 11 `g14_game_nodes` tests passed.
- `cargo clippy -p ve-game --all-targets -- -D warnings`: passed.
- `cargo check -p ve-game -p ve-modeling -p ve-evaluator --target wasm32-unknown-unknown`: passed. It reported existing warnings in `ve-geometry` and `ve-modeling` outside this change.
- Re-ran `cargo test -p ve-game --test follow` and `cargo test -p ve-modeling --test g14_game_nodes` after the clippy cleanup: both passed.

The `g12_field_warp` failures are unrelated to this task and match the handoff's stated known debt. Runtime wiring that consumes the viewer and draws entities by `KindDef.part` remains the next phase.
