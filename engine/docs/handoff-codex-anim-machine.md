# Handoff: `anim.machine@1`, the animation state machine as a graph node

Owner: Claude manages; Codex implements. **Crates you may edit, in this order, running that crate's tests before moving on: `ve-graph` (operator spec, ready list), `ve-modeling` (evaluation, `Part` field, `Cargo.toml` dependency on `ve-animstate`).** Do not touch `ve-animstate` (finished: `StateMachine::parse`/`to_text`/`validate`), `ve-runtime`, `ve-render`, `ve-bevy`, `ve-evaluator`, `src/material_presets.rs`.
Rules: never run `git checkout`, `git clean`, `git stash` or `git restore`; never `cargo fmt` (use `rustfmt --edition 2024 --config-path rustfmt.toml <file>`); never `cargo xtask`; use exactly `CARGO_TARGET_DIR=/tmp/codex-target-machine`; do not commit or stage; edit with your edit tools, not scripts; leave `[lints]` alone and fix clippy findings in your own code. In `ve-modeling` run integration tests with `--test <name>` (the lib test target may not compile for reasons that are not yours). A new dependency on `ve-animstate` is a path dependency like the other `ve-*` ones; check it builds for wasm (`ve-animstate` is pure arithmetic).

## Why

The owner's goal (2026-10-25): an LLM makes characters, creatures and animation itself from general primitives. Clips already exist in the language (`anim.track`, `anim.wave` with a `clip` name) but nothing chooses between them in play. `ve-animstate` has the logic (states, 1D blend spaces, transitions with conditions, fades, exit times) and, as of this job's predecessor, a text form a model can write. This job makes that text a node, so a model attaches a machine to a part in the language, and gets told when the machine names a clip nothing defines. Driving the machine at runtime (parameters from the game, blending the clips' poses) is a later job; here the machine is carried as data.

## What to build

`anim.machine@1(in, program)`:
- Input `in`: any solid, placement set or polygon mesh (the family `anim.track` takes: look at its spec and at how it passes the value through). Output: the same shape, unchanged.
- `program`: string up to 4,096 bytes, in `StateMachine::parse`'s text. **`;` also counts as a line break** (the language's strings cannot be written over several lines by a model easily; `\n` also works), so `"entry a; state a clip=x"` is two lines. Do the replacement of `;` by `\n` before calling `parse`, and count lines on the result so `ParseError.line` matches the statement order the model wrote.
- Evaluation (`ve-modeling`, a new `src/anim_machine.rs`, registered in `operations.rs` like `anim.track`; estimate `Ok(0)` in `plan/estimate.rs`): parse the program; a `ParseError` fails the node with code `anim.machine` and the error's `Display` as the message (it starts with `line N`) and a suggestion that points at `state NAME clip=CLIP`/`transition A -> B when ...`. On success set `Part.state_machine: Option<ve_animstate::StateMachine>` (a new public field on `Part`, `#[serde(default)]`, `None` for every other part) on the part(s) of this node's value (for a plain solid its part; for placements the prototype's part), and for an input that has several regions put it on all of them.
- **Missing clips:** after the whole evaluation, collect the clip names used by the machine (`Motion::Clip.clip` and every `BlendPoint.clip`) and the clip names defined by tracks anywhere in the evaluation's parts (`Track.clip`; the empty string is the main clip). For each used clip no track defines, push one `EvaluationWarning` with code `anim.machine.missing_clip`, node = the machine's node, message such as `"the machine plays clip \"dance\", which no animation track defines; add anim.track or anim.wave nodes with clip=\"dance\""` (one warning per missing clip name, naming only that clip). Find where other cross-part warnings are pushed (`check.attached` in `evaluate.rs` or `hulls.rs` is the model) and follow it.
- Operator doc (one paragraph the model reads): what a state machine is for, the text format with a two-state example, that `;` separates statements, that the machine reads parameters named in conditions (`speed`, `grounded` and triggers are set by the game; naming is free), and that clips come from `anim.track`/`anim.wave` `clip=` names.
- Parameters read with `Param::as_f64`/`Param::Str` as elsewhere (whole numbers arrive as integers in saved documents).

## Tests (written by Claude; never weaken): `crates/ve-modeling/tests/anim_machine.rs`

6 tests: the machine rides on the part and the shape is untouched; no node, no machine; a bad program is refused naming the line (`;` as a line break) and the unknown state; a missing clip gets exactly one warning naming it while defined clips get none; the default empty-named clip matches tracks with no clip; integer values work. Also run `cargo test -q -p ve-graph` (update only entries for the new operator) and the other `ve-modeling` integration targets you touch (`--test materials`, `--test check_attached`).

## Check

`cargo test -q -p ve-graph`, `cargo test -q -p ve-modeling --test anim_machine --test materials --test check_attached`, `cargo build -q -p ve-evaluator --target wasm32-unknown-unknown --release`.

## Status (Codex: write this section yourself when you stop; your final chat message is not saved)

- [ ] done / [x] blocked: implementation is in place, but one supplied integration test conflicts with the required `ve_animstate` validation. `whole_number_values_written_as_integers_work` uses `exit=1` on a looping source state; `StateMachine::parse` rejects it because exit times require a one-shot source. The handoff forbids changing `ve-animstate` and requires passing the text to its parser, so I did not mask that validation or alter the test.
- Tests run and their output:
  - `CARGO_TARGET_DIR=/tmp/codex-target-machine cargo test -q -p ve-graph` — passed (all test targets).
  - `CARGO_TARGET_DIR=/tmp/codex-target-machine cargo test -q -p ve-modeling --test anim_machine --test materials --test check_attached` — animation machine: 5 passed, 1 failed for the conflict above; Cargo stopped at that failing target.
  - `CARGO_TARGET_DIR=/tmp/codex-target-machine cargo test -q -p ve-modeling --test materials` — passed (7 tests).
  - `CARGO_TARGET_DIR=/tmp/codex-target-machine cargo test -q -p ve-modeling --test check_attached` — passed (7 tests).
  - `CARGO_TARGET_DIR=/tmp/codex-target-machine cargo build -q -p ve-evaluator --target wasm32-unknown-unknown --release` — passed.
- Files changed: `engine/crates/ve-graph/src/builtin/modeling/animation.rs`, `engine/crates/ve-graph/src/builtin/ready.rs`, `engine/crates/ve-modeling/Cargo.toml`, `engine/crates/ve-modeling/src/anim_machine.rs` (new), `engine/crates/ve-modeling/src/appearance.rs`, `engine/crates/ve-modeling/src/ao.rs`, `engine/crates/ve-modeling/src/budget.rs`, `engine/crates/ve-modeling/src/cache.rs`, `engine/crates/ve-modeling/src/emitters.rs`, `engine/crates/ve-modeling/src/evaluate.rs`, `engine/crates/ve-modeling/src/instances/output.rs`, `engine/crates/ve-modeling/src/lib.rs`, `engine/crates/ve-modeling/src/operations.rs`, `engine/crates/ve-modeling/src/plan/estimate.rs`, `engine/crates/ve-modeling/src/result.rs`, and this handoff file.
