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

## Status (Codex: fill in when you finish, then stop)
- State: partial implementation; acceptance is not met.
- What was built: New `crates/ve-game` package with an initial line parser, expression evaluator, basic fixed-step world, snapshot/hash API, effect values, example, and two behavior tests. All created source files are below 400 lines. Only this crate and this Status section were edited.
- Commands run and results: `cargo test -p ve-game` passed (2 tests); `cargo check -p ve-game --target wasm32-unknown-unknown` passed; `cargo xtask quick ve-game` failed in clippy (28 lint errors); `cargo xtask rules` failed because the pre-existing `crates/ve-runtime/src/lib.rs` has 411 lines. `rustfmt --edition 2024 --config-path rustfmt.toml` was run only on ve-game files. Strict gate pending.
- Could not verify / open questions: This is not yet the requested complete rules core. Touch/input/timer/custom event dispatch and actions/selectors are incomplete, static validation is incomplete, and the requested coin/pong/shooter/parser/budget tests are missing. Clippy must be cleaned before acceptance. No commit made.
