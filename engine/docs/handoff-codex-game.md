# Handoff to Codex: `ve-game`, a data-driven game rules core

Read `engine/AGENTS.md` first and follow it (no `unwrap`/`expect`/`panic!`/`println!` in library code, no `#[allow]`: use `#[expect(lint, reason)]` on the
smallest item only, files under 400 lines, docs on public items, tests, no commits, no `Co-Authored-By` or tool attribution). **Never run `cargo fmt` or
`cargo fmt --all`; format only your files with `rustfmt --edition 2024 --config-path rustfmt.toml <file>` from `engine`.** Create a new crate
`crates/ve-game` (pure Rust, no Bevy, no I/O, compiles to wasm32). Do not edit other crates. Another engineer is editing `ve-graph`, `ve-modeling`,
`ve-runtime`, `ve-render` and `web/`.

## Why (research, short)
- Ludii and VGDL/GVGAI show that most games are expressible as **data**: Ludii = players + equipment + rules; VGDL = sprite set, interaction set (what
  happens when two kinds touch), termination set (win/lose), level mapping. No general scripting needed.
- Spec2Game (2026) finds LLMs write entities fine but are weak on **rules/mechanisms (46/100) and goal/termination (44/100)**; detailed, sectioned
  specs help most (51 to 69). So the language must make rules, goals and feedback explicit, small and checkable, and give actionable errors.
- ECS engines (Bevy observers, Flecs, Godot signals) use one uniform event mechanism. We do the same: everything is an event with a name and payload.

## What to build
A deterministic fixed-step (1/60 s) simulation driven by a **line-based text definition** in the same style as our node language (one statement per line,
`key=value`, `#` comments). No scripting, no loops, no recursion: only these statements.

```
game name=coins
var score=0 lives=3 time=0
kind player  size=0.5 speed=4 tags=hero
kind coin    size=0.4 tags=pickup
kind spike   size=0.5 tags=hazard
spawn player at=0,0,0
spawn coin   at=3,0,2 count=8 spread=6     # seeded scatter inside a square
input move   axis=x,z target=tag:hero      # moves entities with that tag by speed*axis
input jump   key=press
rule on=touch a=tag:hero b=tag:pickup  do=add:score:1;destroy:b;effect:sfx:pickup@b
rule on=touch a=tag:hero b=tag:hazard  do=add:lives:-1;destroy:b;effect:particles:explosion@b
rule on=tick every=1.0                 do=add:time:1
rule on=var:lives when=lives<=0        do=lose
goal win  when=score>=8
goal lose when=time>=60
```

Requirements:
1. **Kinds and entities:** kind has `size` (radius), `speed`, `tags` (up to 8, name ≤16 chars) and numeric `props` (up to 16). Entity: id, kind, position
   (x,y,z), velocity, props. Movement is simple (position += velocity*dt, speed*input axis); no physics engine here.
2. **Events:** `start`, `tick`, `touch` (sphere overlap, once per overlap start, with `a` and `b` entity references), `input`, `timer`, `var:<name>`,
   `destroyed`, and custom `emit:<name>`. Selectors `tag:x`, `kind:x`, `a`, `b`, `other`.
3. **Conditions:** `when=` tiny expression grammar: numbers, var and prop names, `< <= > >= == !=`, `and`, `or`, `not`, parentheses, plus `+ - * /`. Parse once
   into a tree; errors name the column and suggest a fix.
4. **Actions** (separated by `;`): `set:var:v`, `add:var:v`, `spawn:kind@pos`, `destroy:sel`, `move:sel:dx,dy,dz`, `setprop:sel:name:v`, `emit:name`,
   `timer:name:seconds`, `win`, `lose`, `effect:<sfx|particles|camera_shake|message>:<name>@<target>`. **Effects are never executed here**; they are
   returned in the step outcome so a host (the viewer) renders particles and plays sound (ve-particles `Preset`, ve-audio `SfxKind` names).
5. **Budgets and determinism:** max 2048 entities, 256 rules, 64 kinds, 4096 actions per step, event chain depth 8; exceeding returns a typed
   `GameError` or a truncated outcome with a diagnostic, never a panic. Seeded RNG (own small PCG, no external dependency). Same definition, same
   inputs, same seed give bit-identical state on every platform: `World::hash()` (FNV over canonical state), `snapshot()`/`restore()` for save and rollback.
6. **API:** `GameDef::parse(text) -> Result<GameDef, Vec<Diagnostic>>` (diagnostics carry line, column, message, suggestion), `World::new(&def, seed)`,
   `world.step(&Inputs) -> Outcome { status: Playing|Won|Lost, effects: Vec<Effect>, events_count }`, `world.entities()`, `world.vars()`. Plus
   `GameDef::describe()` (a compact, model-friendly summary of kinds, vars, rules and goals) and `validate()` that flags rules that can never fire (no entity
   of that tag) and goals that can never be reached.
7. **Tests (each a real behaviour):** the coin example above parses, can be won by a scripted input sequence, and loses by timeout; touch fires once per
   overlap; destroy during iteration is safe; budget overflow reports; determinism (two runs, same hash; snapshot, run, restore, run again gives the same
   hash); parser error messages for ten malformed lines (unknown kind, bad expression, missing `=`, etc.); a second and third game (a pong-like
   paddle and ball using `move` and `emit`, and a shooter with timers and spawning) written as text and played by scripted input.
8. Keep files small: split into `parse/`, `expr.rs`, `world/`, `event.rs`, `effect.rs`, `error.rs`.

## Acceptance
`cargo test -p ve-game`, `cargo clippy -p ve-game -- -D warnings` clean, `cargo check -p ve-game --target wasm32-unknown-unknown`, `cargo xtask rules`
shows none of your files over 400 lines. Add `examples/play.rs` that runs the coin game with a scripted input and prints the final state and hash.

## Round 2 (2026-10-22): finish it; the contract is `crates/ve-game/tests/games.rs` (do not edit it)
The owner-side session wrote the tests first (ADR 0062 makes `ve-game` the rules core of the document's `game.*` nodes). Make every test in `tests/games.rs` pass, plus the clippy and wasm acceptance below. Read the tests; they are the exact contract. Interface decisions the original text left open (these override anything above that disagrees):
- **Input names.** `input <name> axis=x,z target=tag:hero` creates the axes `<name>.x` and `<name>.z` in `Inputs.axes` (so `move.x`, `move.z`); each is clamped to -1..1 and moves every entity matching the target selector by `speed * axis * dt` (a kind's `speed`). `input <name> key=press` is raised by naming it in `Inputs.pressed`; it raises the event `input:<name>` for that step. A non-finite axis value is `GameError::InvalidInput(name)`.
- **Host events.** `Inputs` gains `pub host: Vec<HostEvent>` with `pub struct HostEvent { pub name: String, pub a: Option<u32>, pub b: Option<u32> }` (`Clone, Debug, PartialEq`; `Inputs` stays `Default`). A host event raises the rule event `host:<name>` for exactly that step, with `a` and `b` bound to the named entity ids so selectors `a`, `b` and `tag:x` filters work (the engine uses this for grab, release, socket and sensor events from `ve-interact` and `ve-physics`).
- **New action `setvel:<selector>:vx,vy,vz`** sets the velocity of the selected entities; positions integrate `velocity * dt` every step in addition to input motion. `move` stays an instant displacement.
- **Touch** is sphere overlap (distance between positions below the sum of the two kinds' `size` radii), evaluated each step in ascending entity id order; it fires once when a pair starts overlapping (an overlap present at the first step counts as starting then) and again after the pair separated and re-entered. Rules with `a=` and `b=` selectors match in either order but bind `a` to the entity matching the `a=` selector.
- **Chain depth.** An event raised by an action (`emit`, `var:` change events, `timer:` expiry) is handled in the same step up to depth 8; beyond that `step` returns `GameError::ChainDepth(8)` (new variant; state is left as it was at the start of the step). Action budget stays `GameError::ActionBudget(n)`, entity budget `GameError::EntityBudget(n)` (also returned when `World::new` spawned more than 2048, at the next `step`; `validate()` reports it as a diagnostic whose message contains `2048`).
- **Timers.** `timer:<name>:<seconds>` fires `timer:<name>` after `round(seconds * 60)` steps; `on=tick every=1.0` fires every 60 steps starting at step 60.
- **Diagnostics.** Every parse error is a `Diagnostic { line, column, message, suggestion }` with a non-empty message and suggestion and the one-based line and column of the offending token (the ten cases in the test name what each should say). `validate()` returns warnings for rules that can never fire (a selector names a kind or tag no kind has, an event nothing raises) and goals that cannot be reached (a variable in a goal that no rule ever changes), each naming the thing.
- **`describe()`** returns a compact summary (game name, kinds with tags, variables, rules, goals) under 2,000 bytes for the coin game.
- **Determinism.** `World::hash()` must not depend on `format!("{:?}")` of floats or maps (use a canonical byte encoding of the state); `snapshot()`/`restore()` capture entities, variables, tick, rng, status, pending timers and the touching-pair set so a restore replays identically; `restore` of a snapshot from another definition returns `SnapshotMismatch` (compare a hash of the definition's canonical text, not just its name).
- **Selectors** `tag:x`, `kind:x`, `a`, `b`, `other` in actions and rule filters. Unknown selectors are parse errors (`bad selector`).
Add your own tests in `tests/` (a new file) for anything the contract does not cover (selectors, timers across restore, rule order). Run: `cargo test -p ve-game`, `cargo clippy -p ve-game --all-targets -- -D warnings`, `cargo check -p ve-game --target wasm32-unknown-unknown`. If a listed lint in the test file's `#![expect(...)]` is wrong, say so in your Status; do not edit the file. Use `CARGO_TARGET_DIR=target-codex-game`. Files stay under 400 lines (split further under `parse/`, `world/`).

## Status (Codex: fill in when you finish, then stop)
- Round 2 implementation completed; all 13 supplied `games.rs` tests and 3 added `round2.rs` tests pass.
- Earlier status (round 1), kept for the record: partial implementation; acceptance is not met.
- What was built: Completed line-based input and rule parsing, selectors, host events, deterministic movement and touch dispatch, rules/actions, timers, event-chain and entity/action budgets, validation and summaries, canonical state hashing, definition-bound snapshots, and rollback on failed steps. Added selector, timer restore, and rule-order tests in `crates/ve-game/tests/round2.rs`. The test contract `tests/games.rs` was not edited. All crate Rust files are below 400 lines. Only `crates/ve-game` and this Status section were edited.
- Commands run and results (from `engine/`, with `CARGO_TARGET_DIR=target-codex-game`): `cargo test -p ve-game` passed (13 supplied tests and 3 added tests); `cargo clippy -p ve-game --all-targets -- -D warnings` failed on five `clippy::float_cmp` diagnostics in the unedited `crates/ve-game/tests/games.rs`; `cargo clippy -p ve-game --lib -- -D warnings` passed; `cargo check -p ve-game --target wasm32-unknown-unknown` passed. No `cargo fmt`, `cargo xtask quick`, or commit was run.
- Could not meet: The requested all-target clippy command cannot pass while preserving the instruction not to edit `tests/games.rs`. Its `#![expect(clippy::unwrap_used)]` does not account for the five exact float comparisons that clippy reports (`clippy::float_cmp`). The tests themselves pass. No other test is believed wrong. No commit made.
