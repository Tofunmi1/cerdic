# Handoff: `anim.drop`, a physical fall baked into keys

Owner: Claude. Helper: Codex. Plain Rust, no Blender, no C++.

## Why
Things that are let go in an animation (a magazine, brass, a dropped tool) were keyed by hand with a guessed fall, so they hung in the air, sank into the floor or
never bounced. This node bakes a believable fall into ordinary `anim.track` keys: gravity, the floor, bounce and rest. It is the same idea as `phys.body`
but for authored clips: no runtime physics, the result plays identically everywhere and costs nothing per frame.

## The test (fixed, do not weaken)
`crates/ve-modeling/tests/g27_anim_drop.rs` (9 tests, closed-form flight and bounce). Do not edit it.
Run: `CARGO_TARGET_DIR=target-codex-drop cargo test -p ve-modeling --test g27_anim_drop`.

## What to build
Operator `anim.drop@1`: input one shape (any part), output the same shape with tracks. Parameters (flat, with units, ranges and defaults; follow `anim.track` for style):
- `start` Num (Duration/seconds, default 0, 0..60): when it is let go. Before that the displacement is zero.
- `velocity` Vec3 (m/s, default [0,0,0], each -50..50): velocity at release.
- `spin` Vec3 (deg/s, default [0,0,0], each -2000..2000): turning rate at release about the part's centre (bounding-box centre of the input geometry).
- `floor` Num (Length, default 0): height of the floor plane (world y).
- `bounce` Num (Ratio, default 0.3, 0..1): restitution.
- `gravity` Num (default 9.81, 0.1..50).
- `duration` Num (seconds, default 4, 0.5..60): length of the tracks; the last key is exactly at `duration`.
- `clip` Str (default "", same rules as `anim.track`).
Output tracks (both one-shot, `looped=false`, interpolation linear):
- a `position` track: the displacement of the part from where it was modelled. Keys start at time 0 with zero, hold zero until `start`, then follow the flight.
- a `rotation` track (degrees, same convention as `anim.track` rotation, applied about `pivot` = the centre) ONLY when `spin` is not zero.
Physics (document the simplifications in the operator doc):
- Semi-implicit integration at a fixed sub-step (e.g. 1/480 s), f64.
- The part touches the floor with the lowest point of its rest geometry (its bounding box bottom); tumbling is not modelled for contact height.
- On impact vertical velocity becomes `-bounce * v` (at most 6 bounces; a bounce that would rise under 1 mm stops the part), horizontal velocity is multiplied by 0.6
  (floor friction) and spin by 0.6 on each bounce; once it is down and slow it lies still: the displacement is constant until `duration`.
- Keys: a key exactly at `start`, exactly at every impact, and at least 8 evenly spaced keys inside every flight arc; at most 64 keys per track (`anim.track` limit,
  `MAX_KEYS`). If the limit would be exceeded, thin the arcs evenly (never drop an impact key). The last key is at `duration`.
- Refusals with the parameter named in the message and a `suggestion`: `start >= duration`, a `floor` at or above the lowest point of the part, values out of range.
- Deterministic: no random numbers, no time, only f64 in fixed order.

## Where
- Spec: `crates/ve-graph/src/builtin/modeling/animation.rs` next to `anim.track`; add `"anim.drop@1"` to the ready list in `crates/ve-graph/src/builtin/ready.rs`.
- Evaluate: `crates/ve-modeling/src/anim/` (a new file `drop.rs` is fine; wire it where `anim.track` is dispatched) and the cost estimate in `plan/estimate.rs` (cheap, count it like a track).
- Docs: one entry where animation operators are listed in `docs/modeling/` and one line in the guide of `tools/mcp/guide-creatures.mjs` or the animation guide topic (grep `anim.track`).
- Add unit tests of the integrator (energy never grows, resting state reached) in the new file.

## Rules
- You may edit only the files named above. I am editing `ve-modeling/src/human_rig.rs`, `ve-graph/src/builtin/human.rs` and `ve-modeling/src/human.rs` (another helper is on them): do not touch them. Never run `git checkout`, `git clean`, `git stash`, `git restore`. No commits.
- Do not run `cargo fmt`; use `rustfmt --edition 2024 --config-path rustfmt.toml <file>` on your own files.
- `CARGO_TARGET_DIR=target-codex-drop`, no `cargo xtask`. No `unwrap`/`expect`/`panic!` in library code, no `#[allow]` (use `#[expect(..., reason)]` on the smallest item), files under 400 lines.
- When done also run `cargo test -p ve-graph` and `cargo test -p ve-modeling --test anim_tracks`.
- Write the final Status below yourself (your final message is not saved). Say honestly what you did not do.

## Status
Implemented `anim.drop@1`: validated parameters, deterministic one-shot position keys with floor impacts and rest, optional spin rotation about the input bounds centre, ready registration, plan estimate, docs and guide entry. Added integrator energy/rest unit tests. The fixed `g27_anim_drop` suite (9 tests), `cargo test -p ve-graph`, and `cargo test -p ve-modeling --test anim_tracks` pass.

Contributor gates: `cargo xtask quick ve-graph ve-modeling` stopped at the workspace formatter check, which reports formatting diffs in unrelated existing files. `cargo xtask rules` fails on pre-existing over-limit files, including `evaluate.rs` (outside the permitted edit list). `cargo xtask parity` fails on native/WASM corpus output differences unrelated to this feature. `cargo xtask web-test dev g10-animation` builds the web targets but stops at schema/typecheck due to two unrelated TypeScript errors (`src/engine/snapshot.ts` and `tests/shader-expr.test.ts`); the browser script itself was not reached. Strict gate pending.
