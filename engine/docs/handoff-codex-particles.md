# Handoff to Codex: `ve-particles`, particle effects

Read `engine/AGENTS.md` first and follow it (no `unwrap`/`expect`/`panic!`/`println!` in library code, no `#[allow]`, files under 400 lines,
docs on public items, every behaviour has a test, no commits, no `Co-Authored-By` or tool attribution anywhere).

## Why
The engine has no particles: no fire, smoke, sparks, dust, rain, snow or splashes. This crate is the **pure simulation core**: it steps
particles on the CPU, deterministically, and hands back per-particle data for a billboard renderer. Rendering and the graph-language node
are later tasks, not this one.

## Scope: you may create and edit only
- `engine/crates/ve-particles/**` (new crate; `crates/*` is already a workspace glob, so no other file needs editing)
- the Status section at the bottom of this file

Do NOT touch any other crate, the workspace `Cargo.toml`, the web folder, or the C++ code.

## What to build
1. **`EmitterSpec`** (serde, validated by a `validate()` that returns typed errors that say what to change): spawn rate per second and
   one-shot bursts, lifetime range, initial speed range and a cone angle around a direction, shape (point, sphere, box, disc, line),
   gravity vector, linear drag, turbulence strength and scale (deterministic noise), spin range, size over life and colour over life
   (small keyframe curves with linear interpolation), `max_particles` (hard cap 20000), `seed`, optional ground plane height with
   bounce and kill-on-hit.
2. **`ParticleSystem`:** `new(spec)`, `step(dt, emitter_to_world)` (a fixed internal timestep, accumulating the leftover, so a frame rate
   change does not change the result), `burst(count)`, `particles() -> &[Particle]`, and `instances() -> Vec<ParticleInstance>` with
   world position, size, rotation, RGBA colour and age 0..1, sorted back to front for a given camera position via
   `sorted_instances(camera_pos)`.
3. **Presets:** `Preset::{Fire, Smoke, Sparks, Dust, Rain, Snow, Magic, Splash, Explosion, Steam, Embers, Fountain}` giving a ready
   `EmitterSpec`, each scalable by a `scale` and tintable by a colour.
4. **Determinism:** same spec, seed and step sequence give bit-identical particles on native and `wasm32`. No libm `sin`/`cos`/`exp`/`pow`
   in the path; use your own approximations from `+ - * /` and `sqrt` and an integer PRNG (splitmix64). Document it.
5. **Bounds:** never allocate beyond `max_particles`; dead particles are recycled; a very large `dt` is clamped (say so in the docs).

## Acceptance (all as tests; run `cargo test -p ve-particles` and `cargo clippy -p ve-particles`, both clean)
- Same inputs give identical output (compare a hash); a different seed differs.
- Stepping 1 x 1.0 s gives the same result as 60 x (1/60) s (fixed timestep).
- The live count never exceeds `max_particles`; burst plus rate respects the cap.
- Particles die at the end of their lifetime and their age reaches 1.0; colour and size follow the curves at age 0, 0.5 and 1.
- Gravity makes a particle fall by about `g t^2 / 2`; drag slows it; the ground plane bounces or kills as configured.
- `sorted_instances` is farthest first.
- Every `Preset` validates, produces particles within one second of stepping, and stays finite for ten seconds.
- Invalid specs (negative lifetime, zero cap, cap above 20000, NaN) are refused with a message naming the field.
- Do not run `cargo xtask check --strict`. Do run `cargo xtask rules` at the end and fix any complaint about your crate.

## Status (Codex: fill in when you finish, then stop)
- State: complete; `ve-particles` implementation and crate checks are done.
- What was built: Added the serializable and validated `EmitterSpec`, bounded fixed-step `ParticleSystem`, deterministic splitmix64 stream and arithmetic-only noise, shape/cone sampling, curves, ground collisions, render instances and sorting, plus all twelve tintable/scalable presets. Frames above 10 seconds are clamped. Eight acceptance tests cover determinism, frame-step equivalence, cap, curves/lifetime, gravity/drag/ground, sorting, presets, and invalid fields.
- Tests run and results: `cargo test -p ve-particles` passed (8/8); `cargo clippy -p ve-particles` passed; `cargo xtask quick ve-particles` passed the crate clippy and focused nextest stages (8/8), but its final rules stage failed on existing over-400-line files in other crates (`ve-render`, `ve-audio`, `ve-graph`, `ve-modeling`, and `ve-runtime`). `cargo xtask rules` likewise reports only those out-of-scope files. The strict gate is pending.
- Open questions or things you chose not to do: The workspace rules gate is blocked by unrelated files; they were left untouched per scope. No renderer or graph-language integration was added, as directed.
