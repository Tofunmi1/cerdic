# Handoff: game rules set animation parameters (`effect:anim_set` and `effect:anim_flag`)

Owner: Claude manages; Codex implements. **Crate you may edit: `ve-game` only** (`src/effect.rs`, `src/parse/helpers.rs` action validation, `src/world/state/actions.rs`, wherever else `effect:` actions are parsed or emitted). Do not touch other crates: `ve-runtime/src/game.rs` has an exhaustive `match` on `EffectKind` and constructs nothing else, but it will not compile after this job: list what breaks in your Status and Claude fixes it.
Rules: never run `git checkout`, `git clean`, `git stash` or `git restore`; never `cargo fmt` (use `rustfmt --edition 2024 --config-path rustfmt.toml <file>`); never `cargo xtask`; use exactly `CARGO_TARGET_DIR=/tmp/codex-target-animparams`; do not commit or stage; edit with your edit tools, not scripts; leave `[lints]` alone and fix clippy findings in your own code (`cargo clippy -p ve-game --lib` has six older findings in `parse/reader.rs` and `world/state/hands.rs`: not yours). Deterministic arithmetic only.

## Why

The owner's goal (2026-10-25): an LLM builds playable games and the characters in them. A state machine (`anim.machine`) chooses clips from named parameters (`speed`, `grounded`, `armed` ...) and triggers. A game rule can already raise a trigger (`effect:anim:jump@tag:hero`). It cannot set a parameter. The real-model test of 2026-10-25 (a six-legged robot) ended with the model's own words: "animation state controls work, but I couldn't provide working keyboard movement controls": nothing connects the game (input, movement, state) to the machine. This job adds the two missing effects; the runtime (Claude) applies them to the machine.

## What to build

Two new effect kinds, `EffectKind::AnimSet` (a number parameter) and `EffectKind::AnimFlag` (a true/false parameter), and a new field on `Effect`: `pub value: Option<f64>` (`None` for every existing kind; add it wherever an `Effect` is built: the existing effects emit `value: None`). Update the doc comments of `Effect` and `EffectKind`.
- Action syntax: `effect:anim_set:NAME:EXPR[@SELECTOR]` and `effect:anim_flag:NAME:EXPR[@SELECTOR]`. `NAME` is the parameter name (non-empty, no `:` or `@`). `EXPR` is an expression in the game's existing expression language (`expr.rs`: numbers, names, `+ - * /`, comparisons, `and`, `or`, `!`): it is parsed and checked at parse time (a bad one is a parse error with the usual message), evaluated when the action runs. **Split the action on the last `@` first** (a selector such as `tag:hero` has a colon of its own), then split the rest into kind, name and expression at the first two colons only (the expression is everything after the name's colon).
- Evaluation: the same name lookup a rule's `when` uses (game variables and whatever else resolves there), **plus two names for the entity the effect is aimed at**: `speed` (the horizontal speed of that entity, `sqrt(vx*vx + vz*vz)`, in units per second, from its velocity at the moment the action runs) and `vy` (its vertical velocity). With no selector there is no entity: `speed` and `vy` resolve as the lookup would for an unknown name (read how it does that; do not invent a new rule). `EffectKind::AnimSet`: `value = Some(result)`. `EffectKind::AnimFlag`: `value = Some(1.0)` for a non-zero result, `Some(0.0)` for zero.
- Targets: as for `effect:anim`: a selector matching several entities emits one effect per entity (each evaluated with that entity's own `speed` and `vy`), `target: Some(id)`; no selector emits one effect with `target: None`; `name` is the parameter name. Validate the selector as the other effect actions do.
- Actions run in order, so `setvel:a:3,0,4;effect:anim_set:speed:speed@a` sees the new velocity (the test relies on it).

## Tests (written by Claude; never weaken): `crates/ve-game/tests/anim_params.rs`

9 tests: a literal; an expression over a variable; `speed` and `vy` of the target (5 and 2 for a hero moving at (3, 2, 4)); flags from non-zero values and a comparison; a selector matching several entities; one request per tick; eight bad forms refused at parse time; other effects carry no value while `anim` still works; determinism. Plus every existing `ve-game` test (`tests/anim_effect.rs` included).

## Check

`cargo test -q -p ve-game`, `cargo check -q -p ve-game --target wasm32-unknown-unknown`, `cargo clippy -q -p ve-game --lib -- -D warnings` (only the six older findings may remain).

## Status (Codex: write this section yourself when you stop; your final chat message is not saved)

- [x] done / [ ] blocked: `ve-game` implementation complete; host integration is still needed.
- Tests run and their output:
  - `CARGO_TARGET_DIR=/tmp/codex-target-animparams cargo test -q -p ve-game` — passed (all tests, including all 9 `anim_params` tests).
  - `CARGO_TARGET_DIR=/tmp/codex-target-animparams cargo check -q -p ve-game --target wasm32-unknown-unknown` — passed.
  - `CARGO_TARGET_DIR=/tmp/codex-target-animparams cargo clippy -q -p ve-game --lib -- -D warnings` — blocked by an existing `clippy::unnested_or_patterns` finding at `crates/ve-game/src/parse/reader.rs:281`; clippy did not report findings in the changed code before stopping.
  - `CARGO_TARGET_DIR=/tmp/codex-target-animparams cargo check -q -p ve-runtime` — passed for the native target. A wasm `ve-runtime` check was started but stopped before it finished.
- Files changed: `crates/ve-game/src/effect.rs`, `crates/ve-game/src/expr.rs`, `crates/ve-game/src/parse/helpers.rs`, `crates/ve-game/src/world/state.rs`, `crates/ve-game/src/world/state/actions.rs`.
- Crates outside your scope that need follow-up: `ve-runtime`'s wasm `crates/ve-runtime/src/game.rs` has an exhaustive `EffectKind` match in `effect_json` that needs arms for `AnimSet` and `AnimFlag`. Its effect forwarding also currently passes only kind, name and target, so host integration must forward/apply `Effect.value` to the targeted animation machine. The native `ve-runtime` check passed; the wasm check was not completed.
