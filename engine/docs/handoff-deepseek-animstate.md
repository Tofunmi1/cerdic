# Handoff to DeepSeek: `ve-animstate`, animation state machine

Read `engine/AGENTS.md` first and follow it (no `unwrap`/`expect`/`panic!`/`println!` in library code, no `#[allow]`: if a lint is truly wrong
use `#[expect(lint, reason = "...")]` on the smallest item, files under 400 lines, docs on public items, every behaviour has a test, no
commits, no `Co-Authored-By` or tool attribution anywhere). Other engineers are editing other crates at the same time: touch nothing else.

## Why
Generated characters have named clips (`""` is the walk, `run`, `idle`, `fly`) but nothing chooses between them. A walker that speeds up
should blend from idle to walk to run; a jump should play once and return. This crate is the **pure logic core**: it decides which clips play
and with what weights. Wiring it to the runtime and the graph language is a later task, not this one.

## Scope: you may create and edit only
- `engine/crates/ve-animstate/**` (new crate; `crates/*` is already a workspace glob, so no other file needs editing)
- the Status section at the bottom of this file

## What to build
Dependencies: workspace `serde`, `thiserror` only. Deterministic: same inputs and step sequence give identical output (no libm `sin`/`cos`/
`exp`/`pow` in the path; use `+ - * /` and `sqrt` or small polynomials).

1. **Parameters:** a `Params` map of named values: `Float` (speed), `Bool` (grounded), `Trigger` (one-shot, cleared when consumed).
2. **Graph (serde, with `validate()` returning errors that name what to change):** `StateMachine { states, transitions, entry }`.
   A `State` is either a `Clip { clip: String, speed: f32, looping: bool }` or a `Blend1D { parameter: String, points: Vec<(f32, clip)> }`
   (1D blend space: idle at speed 0, walk at 1.4, run at 4, interpolating weights between the two nearest points and extrapolating by
   clamping). Transitions have a source (or any state), a target, conditions (`Greater`, `Less`, `Equals`, `Trigger`, `And` of several), a
   crossfade duration in seconds, and `exit_time` (only when the source clip has played at least this fraction; for one-shot clips).
3. **Runtime:** `Machine::new(graph, clip_lengths: &HashMap<String, f32>)`, `set_float/set_bool/trigger`, `step(dt) -> Pose` where
   `Pose { layers: Vec<ClipWeight { clip, time, weight }> }` with weights summing to 1, clip times wrapped for looping and clamped for
   one-shots, and crossfades interpolating the weights linearly (smoothstep optional) over the transition duration. A new transition may
   interrupt a running crossfade without a pop (continue from the current weights).
4. **Presets:** `Preset::Locomotion` (idle / walk / run blend on `speed`, plus a `jump` trigger into a one-shot that returns by exit time,
   and a `grounded` falling state), `Preset::Flyer` (idle / flap / glide), `Preset::Door` (closed / opening / open / closing from triggers).
   Each uses the clip names the character maker emits: `""` (the default walk clip, write it as the constant `WALK = ""`), `"run"`, `"idle"`,
   `"fly"`.
5. **Bounds:** at most 64 states, 256 transitions, 16 blend points, 8 simultaneous layers; refuse beyond that in `validate`. A huge `dt` is
   clamped (document the clamp).

## Acceptance (tests in the crate; run `cargo test -p ve-animstate` and `cargo clippy -p ve-animstate -- -D warnings`, both clean)
- Blend1D: weights at speed 0, 0.7, 1.4, 2.7, 4, 9 are as expected and always sum to 1.
- A transition fires when its condition holds, the crossfade runs for its duration, weights are continuous across the step it starts, and
  the source's weight reaches 0 exactly at the end.
- A trigger fires one transition once and is then cleared; a one-shot clip stops at its last time and the exit-time transition fires after it.
- An interrupting transition mid-crossfade has no discontinuity in the total weight or in any clip's weight.
- Same inputs give identical `Pose` sequences (compare a hash); `dt` of 1.0 equals sixty steps of 1/60 within a small tolerance.
- Every preset validates, runs for 10 simulated seconds under a scripted speed ramp (0 up to 5 and back) with finite output and weights summing to 1.
- Invalid graphs are refused with messages naming the state, transition or parameter (unknown target, missing entry, empty blend, duplicate state
  name, a blend point list not sorted by value, a cycle of zero-duration transitions that would never settle).
- Run `cargo xtask rules` at the end and fix any complaint about your crate (complaints about other files are not yours).

## Status (DeepSeek: fill in when you finish, then stop)
- State: done. `engine/crates/ve-animstate` exists, tests and strict clippy are clean. Not committed.
- What was built:
  - New crate `ve-animstate` (workspace `serde` + `thiserror` only, engine-agnostic, arithmetic-only
    so it is deterministic on native and wasm). Modules: `params.rs`, `graph.rs`, `validate.rs`,
    `blend.rs`, `machine.rs`, `preset.rs` (all under 400 lines; tests in `tests.rs` + `tests/{pose,runtime,errors}.rs`).
  - `Params`: named `Float`, `Bool` and one-shot `Trigger` values; an unset parameter counts as absent
    (conditions on it are false) and a non-finite float is ignored. `Machine::set_float/set_bool/trigger`.
  - `StateMachine { states, transitions, entry }` with serde. `Motion::Clip { clip, speed, looping }`
    and `Motion::Blend1D { parameter, points }`; `Condition::{Greater, Less, Equals, Trigger, And}`;
    `Transition { from (None = any state), to, conditions, duration, exit_time }`.
    `validate()` returns `Error::{State, Transition, Machine, Parameter}` naming the state, transition
    or parameter: unknown target/source, missing entry, empty/over-long blend, duplicate state name,
    blend points not strictly increasing by value, `exit_time` without a one-shot clip source, a
    parameter used as two kinds, and an unconditional zero-duration cycle (`a -> b -> a`) that would
    never settle. Bounds: 64 states, 256 transitions, 16 blend points.
  - `Machine::new(graph, clip_lengths)`, `step(dt) -> Pose { layers: Vec<ClipWeight { clip, time, weight }> }`.
    Weights sum to 1 (heaviest first, merged by clip). Looping times wrap, one-shot times clamp; a clip
    with no known length is left unwrapped. Crossfades use a smoothstep polynomial; at most one
    transition fires per step and a new one continues from the current weights, so an interruption never
    pops. `dt` is clamped to `MAX_STEP = 10.0` seconds (`1.0` is inside the clamp).
  - `Preset::{Locomotion, Flyer, Door}` built from the maker's clips `WALK = ""`, `RUN`, `IDLE`, `FLY`,
    plus `Preset::clip_lengths()`.
- Commands run and results:
  - `cargo test -p ve-animstate` — 14 unit tests + 1 doctest, all pass.
  - `cargo clippy -p ve-animstate -- -D warnings` and `cargo clippy -p ve-animstate --all-targets -- -D warnings` — clean.
  - `cargo fmt -p ve-animstate -- --check` — clean.
  - `cargo xtask rules` — no complaint about `ve-animstate`; it still reports
    `crates/ve-runtime/src/scene.rs` at 408 lines, which is another engineer's file, not mine.
  - `cargo xtask quick ve-animstate` — stops at workspace `cargo fmt --all --check` because of
    unformatted files in other crates (`crates/ve-render/src/shadow_math.rs` and others), so it never
    reached clippy/tests/rules; the scoped commands above cover this crate. The full strict gate
    (`cargo xtask check --strict`) is pending, as `AGENTS.md` says the owner runs it.
- Open questions or things you chose not to do:
  - "8 simultaneous layers": enforced at runtime in `pose()` (merge by clip, then keep the heaviest
    `MAX_LAYERS` and renormalise), not as a static graph rule, because interruption can in principle
    keep many states partly weighted. `validate` refuses the state/transition/blend-point caps the
    handoff lists. Flagging in case a stricter static refusal was meant.
  - The presets use only the maker's clip names. There is no `"jump"` or `"fall"` clip in
    `ve-modeling::creature`, so `Locomotion`'s jump one-shot plays `WALK` and its falling state plays
    `IDLE` as placeholders, and `Door` opens/closes on `WALK` with `IDLE` for the resting states.
  - `exit_time` is required (by `validate`) to name a source state that is a one-shot `Clip`; a
    `from: None` or `Blend1D` source with `exit_time` is refused rather than guessed at.
  - Wiring to the runtime and the graph language (authoring/tool schema and a corpus line) is the later
    task this handoff explicitly leaves out.
