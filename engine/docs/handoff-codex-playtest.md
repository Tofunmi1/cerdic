# Handoff to Codex: `ve-game` text printing and headless playtest bots, evaluator exports

Read `engine/AGENTS.md` first and follow it (no `unwrap`/`expect`/`panic!`/`println!` in library code, no `#[allow]`: `#[expect(lint, reason)]` on the
smallest item only, files under 400 lines, docs on public items, no commits, no `Co-Authored-By` or tool attribution). **Never run `cargo fmt`; format
only your files with `rustfmt --edition 2024 --config-path rustfmt.toml <file>` from `engine`.** Own `CARGO_TARGET_DIR=target-codex-playtest`.
Context: ADR 0062 (phases 3 and 4), `docs/plans/game-world-system.md` section 7.3. The `game.*` nodes already evaluate to `Evaluation::game`
(`ve-modeling/src/game_nodes.rs`, done); this task lets a tool read and play that game.

## Part 1: `ve-game`
- `GameDef::to_text(&self) -> String`: the line text form that `GameDef::parse` accepts, so `parse(to_text(d)) == d` and printing is stable.
  Keep the original statement order of the definition (vars in their stored order).
- New module `ve_game::playtest` (file under 400 lines): `Policy { Idle, Random, Seek { hero: String, target: String } }`,
  `run(def: &GameDef, seed: u64, policy: &Policy, max_steps: u32) -> Report`, `Report { status: Status, steps: u32, vars: BTreeMap<String, f64>,
  rule_fires: Vec<u64>, never_fired: Vec<usize>, hash: u64 }` (derive `Clone, Debug, PartialEq`). `Idle` sends no input; `Random` sends seeded random axis
  values (and presses) each step; `Seek` steers the first entity carrying tag `hero` at the nearest entity carrying tag `target` (the bot of
  `tests/games.rs`, generalised; axes are named `<input>.<axis>`, use the definition's first axis input). `rule_fires` counts how often each rule's
  actions ran (add a counter to `World`, restored by snapshot/restore). `hash` is the world's deterministic hash after the last step. The run stops
  when the status is not `Playing` or at `max_steps`. Same definition, seed and policy give the same report on native and WASM.

## Part 2: `ve-evaluator` (wasm exports, next to `eval_stats`)
- `eval_game() -> String`: JSON `{ "present": bool, "text": <to_text>, "summary": <describe>, "diagnostics": [ { code, message, suggestion } ] }` for the
  game of the last finished evaluation (`present: false` when none).
- `game_playtest(text: &str, policy: &str, seed: u32, max_steps: u32) -> String`: parses the text, runs `playtest::run`, returns the report as JSON
  (`status` as `"playing"|"won"|"lost"`), or `{ "error": ... }` with the parse diagnostics. `policy` is `"idle"`, `"random"` or `"seek:<hero tag>:<target tag>"`.
  Cap `max_steps` at 36,000.

## Contract (written first, do not weaken)
`crates/ve-game/tests/playtest.rs`. Report a test that looks wrong in Status; do not edit it. Add your own tests for the evaluator exports
(a native `#[cfg(test)]` in `ve-evaluator` is fine if the exports can be called natively; otherwise test the JSON-building functions).

## Verify
```
cd engine
CARGO_TARGET_DIR=target-codex-playtest cargo test -p ve-game
CARGO_TARGET_DIR=target-codex-playtest cargo clippy -p ve-game --all-targets -- -D warnings   # must be clean
CARGO_TARGET_DIR=target-codex-playtest cargo check -p ve-game -p ve-evaluator --target wasm32-unknown-unknown
```

## Status
Implemented `GameDef::to_text` with stable round trips and preserved variable order, deterministic `Idle`/`Random`/`Seek` playtest policies, rule-fire counters included in snapshots and restores, and the `eval_game` / `game_playtest` JSON builders and WASM exports. Added snapshot coverage and evaluator JSON-builder tests. Split the parser and world implementation into smaller modules to keep touched source files under 400 lines.

Checks run with `CARGO_TARGET_DIR=target-codex-playtest`:

- `cargo test -p ve-game`: 13 game tests and 5 of 6 playtest contract tests passed; the seeking-bot contract test failed at its `never_fired` assertion (details below). The added snapshot test passed separately with `cargo test -p ve-game --test games --test playtest_snapshot`.
- `cargo test -p ve-evaluator`: passed, including five native JSON-builder tests.
- `cargo clippy -p ve-game --all-targets -- -D warnings`: blocked by the untouched contract test's `#![expect(clippy::unwrap_used)]` being reported as unfulfilled on the integration-test target. `cargo clippy -p ve-game --lib -- -D warnings` passed.
- `cargo check -p ve-game -p ve-evaluator --target wasm32-unknown-unknown`: passed. It emitted unrelated existing warnings from `ve-geometry` and `ve-modeling`.
- `cargo xtask quick ve-game ve-evaluator`: stopped at its workspace `cargo fmt --all --check` stage; the handoff forbids running `cargo fmt`, so no formatter was run directly and no files were changed by the check.
- `cargo xtask rules`: failed on unrelated existing oversized files `ve-graph/src/builtin/fields.rs` (479 lines), `ve-graph/src/builtin/world.rs` (708), and `ve-modeling/src/human_hair.rs` (419). The touched files are below 400 lines.
- `cargo xtask parity`: failed on a native/WASI corpus mismatch in geometry cases; it did not involve `ve-game` or evaluator exports.
- `cargo xtask web-test dev evaluation`: WASM builds and bundling completed, then the existing `tsc -b` stage failed on `src/engine/snapshot.ts:14` (`NodeListOf<HTMLCanvasElement>` iterator) and `tests/shader-expr.test.ts:84` (`ErrorCode` type mismatch).

The likely incorrect test is `a_seeking_bot_wins_the_coin_game_and_the_report_says_how`: it expects `never_fired == [1, 3]`, but rule index 2 is the `tick every=1.0` clock rule and the bot wins before 3,600 steps. That rule therefore has not fired; the correct never-fired indices are `[1, 2, 3]`. The test was left unchanged as instructed.
