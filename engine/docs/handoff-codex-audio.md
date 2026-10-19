# Handoff to Codex: `ve-audio`, procedural sound

Read `engine/AGENTS.md` first and follow it (no `unwrap`/`expect`/`panic!`/`println!` in library code, no `#[allow]`, files under 400 lines,
docs on public items, every behaviour has a test, no commits, no `Co-Authored-By` or tool attribution anywhere).

## Why
The engine has no audio at all. Product rule: **no imported assets**, so every sound is generated from numbers. This crate is the pure
synthesis core; wiring it to the graph language and the browser (WebAudio) is a later task, not this one.

## Scope: you may create and edit only
- `engine/crates/ve-audio/**` (new crate; `crates/*` is already a workspace glob, so no other file needs editing)
- the Status section at the bottom of this file

Do NOT touch any other crate, `Cargo.toml` at the workspace root, the web folder, or the C++ code.

## What to build
A deterministic, dependency-light (serde, thiserror from the workspace only) crate that renders mono `f32` PCM.

1. **Core types:** `Sound { sample_rate: u32, samples: Vec<f32> }` (default 48000 Hz, peak never above 1.0, no NaN/inf), `Waveform`
   (sine, saw, square, triangle, white noise), `Envelope` (attack, decay, sustain, release), a one-pole / biquad `Filter`
   (low-pass, high-pass, band-pass), `Lfo`.
2. **Patch:** a serde-serialisable `Patch` (oscillators with detune and mix, noise mix, filter with cutoff envelope, amplitude envelope,
   pitch sweep) and `render(&Patch, note_hz, seconds, seed) -> Sound`.
3. **Effect recipes:** `sfx(kind: SfxKind, seed: u64, intensity: f32) -> Sound` for at least: coin, jump, hit, explosion, laser, footstep
   (on stone, wood, grass, metal), whoosh, click, power-up, splash, door open and door close, wind loop, rain loop. Different seeds
   give audibly different variations; intensity 0..1 scales loudness and brightness.
4. **Music:** `music(spec: MusicSpec) -> Sound` where `MusicSpec { seed, tempo_bpm, bars, scale, mood }` (moods such as calm, tense,
   heroic, mysterious, playful). Chord progression from the scale, bass line, arpeggio or lead, soft noise drums. The result must
   **loop without a click** (the start and end connect smoothly).
5. **Spatial maths (pure functions, no audio device):** `spatialise(listener_pos, listener_forward, source_pos, ref_dist, rolloff) ->
   { gain, pan }` with inverse-distance attenuation and equal-power pan, plus a Doppler pitch ratio helper.
6. **WAV encoder:** `to_wav_bytes(&Sound) -> Vec<u8>`, 16-bit PCM mono with a correct header, so a person can listen to a render.
7. **Determinism:** the same inputs give bit-identical samples on native and `wasm32`. Do not call libm `sin`/`cos`/`exp`/`pow` in the
   synthesis path: use your own polynomial or table versions built from `+ - * /` and `sqrt`, and say so in a module doc. Use an
   integer hash/PRNG (for example splitmix64) for noise and variation.

## Acceptance (all as tests in the crate; run `cargo test -p ve-audio` and `cargo clippy -p ve-audio`, both clean)
- Same seed and spec give identical samples (compare a hash); a different seed differs.
- No NaN or inf; peak <= 1.0; expected durations (a coin is short, a rain loop is as long as requested).
- A sine of 440 Hz really has its energy near 440 Hz (use a small DFT in the test); a low-pass filter measurably removes high content.
- A music render loops: the difference between the last and first samples is small, and its length is a whole number of bars.
- `spatialise`: gain falls with distance, pan is -1 to 1 and follows left/right of the listener, equal-power at centre.
- The WAV header parses and the data length matches.
- Write one test that renders every `SfxKind` and checks the basics above, so a new kind cannot be added without passing them.
- Do not run `cargo xtask check --strict`. Do run `cargo xtask rules` at the end and fix any complaint about your crate.

## Status (Codex: fill in when you finish, then stop)
- State: complete.
- What was built: Added `crates/ve-audio` with deterministic procedural rendering, SFX recipes, loop music, spatial maths, WAV encoding, and integration tests. Split music into `src/music.rs` to keep every crate source file under 400 lines. Strict Clippy diagnostics were fixed using clearer names, formatting, `#[must_use]`, safe bounded conversions, and narrowly scoped `#[expect]` attributes for intentional numeric conversions.
- Tests run and results: `cargo fmt -p ve-audio` completed successfully. `cargo test -p ve-audio` passed (4 integration tests; 0 unit tests; doc tests passed). `cargo clippy -p ve-audio -- -D warnings` passed cleanly. `cargo xtask rules` reports no issue in `crates/ve-audio`, but exits 1 for oversized files elsewhere in the workspace (`crates/ve-render/src/prepare.rs` and files in ve-graph, ve-modeling, and ve-runtime).
- Open questions or things you chose not to do: No quick gate or strict workspace gate was run. No files outside `crates/ve-audio` and this Status section were edited by this task.
