# Handoff: the animation state machine as text (`StateMachine::parse` and `to_text`)

Owner: Claude manages; Codex implements. **Crate you may edit: `ve-animstate` only** (a new `src/text.rs`, the `mod`/`pub use` lines in `src/lib.rs`, and an error type exported from there). Do not touch other crates (the graph node and the runtime come in later jobs, after this passes).
Rules: never run `git checkout`, `git clean`, `git stash` or `git restore`; never `cargo fmt` (use `rustfmt --edition 2024 --config-path rustfmt.toml <file>`); never `cargo xtask`; use exactly `CARGO_TARGET_DIR=/tmp/codex-target-animtext`; do not commit or stage; edit with your edit tools, not scripts; leave `[lints]` alone and fix clippy findings in your own code. The crate is arithmetic only and deterministic: no platform maths functions, no randomness, `BTreeMap` where order matters (read `src/lib.rs`'s header).

## Why

The owner's goal (2026-10-25): an LLM must be able to make characters, creatures and animation itself, from general primitives, not from hard-coded presets. `ve-animstate` already holds the logic that picks which clips play and blends them (`StateMachine`, `Machine`, `Pose`), but it is only reachable from Rust, no other crate uses it, and its only authored examples are three hard-coded `Preset`s. A model writes text. This job gives the state machine a small text form a model can write and read, with errors that name the line, so a later graph node can carry it and the runtime can drive it. Presets stay as worked examples; each must be expressible in the text and round-trip.

## The text format (the tests fix exactly this)

One statement per line; `#` starts a comment (to the end of the line); blank lines and extra spaces are ignored; names are case sensitive and contain no spaces.
```
entry ground
state ground blend=speed idle@0 walk@1.5 run@4
state jump clip=jump once
state fall clip=fall loop
transition any -> jump when trigger:jump fade=0.1
transition any -> fall when grounded=false fade=0.2
transition jump -> ground exit=0.9 fade=0.15 when grounded=true
transition fall -> ground when grounded=true & speed>0.1 fade=0.25
```
- `entry NAME`: exactly one, naming a state.
- `state NAME clip=CLIP [speed=F] [loop|once]`: plays one clip; speed default 1.0; looping by default (`once` makes it a one-shot). `CLIP` is a name, or `""` (two double quotes) for the default, empty-named clip (the character maker's walk).
- `state NAME blend=PARAM POINT...` where each point is `CLIP@VALUE`: a 1D blend space on a float parameter, at least one point; points keep the order written.
- `transition FROM -> TO [when COND (& COND)*] [fade=F] [exit=F]`: `FROM` is a state name or `any`. Conditions: `p>F`, `p<F` (float), `p=true`/`p=false` (bool), `trigger:NAME`. `fade` is seconds, default 0.15, not negative. `exit` is the fraction of a one-shot's length, 0 to 1, default none. The `when`, `fade` and `exit` parts may come in any order after `TO`, but `when` runs to the end of what is not `fade=` or `exit=`.
- `to_text(&self) -> String` writes the same format deterministically (states in order, then transitions in order, `entry` first), such that `parse(to_text(m)) == m` and `to_text(parse(to_text(m))) == to_text(m)`. Numbers are written so they re-read to the identical `f32` (use Rust's shortest round-trip `Display` for `f32`).

## API

- `StateMachine::parse(text: &str) -> Result<StateMachine, ParseError>` and `StateMachine::to_text(&self) -> String`.
- `ParseError { pub line: usize, pub message: String }` (derive `Clone, Debug, PartialEq, Eq`; `Display` is `"line N: message"`; implement `std::error::Error`; `thiserror` is already a dependency). `line` is 1-based; a problem of the whole machine (no states, no entry) uses the line of the last statement or 1 for empty input.
- The parser checks names itself, so errors carry line numbers: an unknown or repeated state, a transition to or from a state that does not exist, an unknown keyword or token, a bad number, a bad condition (`speed>>1`), a blend with no points or a point without a value, a state that plays nothing, a negative fade, an exit time over 1, no states, no `entry`, an `entry` that is not a state, more than `MAX_STATES` states or `MAX_TRANSITIONS` transitions. The message names the offender (`"unknown state \"zzz\""`) and, where it helps, what to write instead.
- After a successful parse `validate()` (existing) must pass for anything the parser accepts, or the parser returns its error as a `ParseError` (so a model gets one error source). Do not weaken `validate`.
- Never panic on any input, including NUL bytes, a 100,000-character line, unterminated quotes, `1e999`.

## Tests (written by Claude; never weaken; add more in `src/text.rs` unit tests if you like): `crates/ve-animstate/tests/text_format.rs`

11 tests: a full locomotion text read into the exact graph; defaults; the empty clip name; `<` and chained conditions; comments and spacing; round trip for all three presets; stable text; line numbers for 11 kinds of error; whole-machine errors; no panics.

## Check

`CARGO_TARGET_DIR=/tmp/codex-target-animtext cargo test -q -p ve-animstate` (the existing unit tests must still pass), `cargo clippy -q -p ve-animstate --all-targets -- -D warnings`, `cargo check -q -p ve-animstate --target wasm32-unknown-unknown`.

## Status (Codex: write this section yourself when you stop; your final chat message is not saved)

- [x] done / [ ] blocked: implemented `StateMachine::parse`, `to_text`, and exported `ParseError`.
- Tests run and their output:
  - `CARGO_TARGET_DIR=/tmp/codex-target-animtext cargo test -q -p ve-animstate` — passed (15 unit, 10 integration, 1 doctest).
  - `CARGO_TARGET_DIR=/tmp/codex-target-animtext cargo clippy -q -p ve-animstate --all-targets -- -D warnings` — blocked by `unwrap_used` on `unwrap_err()` in the supplied `tests/text_format.rs`; left test unchanged.
  - `CARGO_TARGET_DIR=/tmp/codex-target-animtext cargo clippy -q -p ve-animstate --lib -- -D warnings` — passed.
  - `CARGO_TARGET_DIR=/tmp/codex-target-animtext cargo check -q -p ve-animstate --target wasm32-unknown-unknown` — passed.
- Files changed: `crates/ve-animstate/src/text.rs` (new), `crates/ve-animstate/src/lib.rs`, `docs/handoff-codex-animstate-text.md`.
