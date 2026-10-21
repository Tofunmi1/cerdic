# Handoff to Codex: the cloud layer of the cubemap sky in `ve-sky`, one job

Read `engine/AGENTS.md` first and follow it: no `unwrap`/`expect`/`panic!`/`println!` in library code, no `#[allow]` (use `#[expect(lint, reason = "...")]` on the
smallest item if a lint is truly wrong), Rust files under 400 lines, docs on public items, no commits, no `Co-Authored-By` or tool attribution.
**Never run `cargo fmt` on the workspace; format only your files with `rustfmt --edition 2024 --config-path rustfmt.toml <file>` from `engine`.**
**Never run `git checkout`, `git clean`, `git stash` or `git restore`. Edit only `engine/crates/ve-sky/src/clouds.rs` and this file's Status section.** Other
people are editing other crates at the same time. Do not build the browser bundle. No network.

## The job
`engine/crates/ve-sky/src/clouds.rs` has three functions whose bodies are stubs (`hash_cell`, `noise2`, `cloud_amount`; the signatures and doc comments are the contract).
They are the engine's sky shader's cloud layer (`engine/crates/ve-render/src/shaders/sky.wgsl`: `mul32_high`, `mul64`, `shr64`, `hash_cell`, `noise2` and the cloud lines at the end of
`fragment`) on the CPU, in `f32` and native `u64`. Read that shader, it is the spec. The tests in `engine/crates/ve-sky/tests/clouds.rs` are fixed: **do not edit them to make them
pass** (a premise you think is wrong goes in the Status). They contain a literal port of the shader's u32-pair hash as the oracle for `hash_cell`.

Notes:
- `hash_cell` returns the hash's high word shifted right by 8 (24 bits), as the shader does. With native `u64` the multiplies are `wrapping_mul` and the xorshifts are plain.
  The cell coordinates enter as `i32 as u32` in the low word and sign extension (`0xffffffff` for negative, 0 otherwise) in the high word.
- `noise2` differs from the shader on purpose, see the module doc: corners are `hash_cell(..) as f32 / 16_777_216.0`, with no extra shift. `floor`/`fract` as in the shader (`fract(p) = p - floor(p)`).
- `smoothstep(e0, e1, x)` is the usual clamp-then-cubic.

## Acceptance
`cargo test -p ve-sky` passes (the 9 new tests and the older ones), `cargo clippy -p ve-sky --lib -- -D warnings` is clean, and no file of yours is over 400 lines.

## Status (Codex: fill in, then stop)
Implemented `hash_cell`, `noise2`, and `cloud_amount` in `engine/crates/ve-sky/src/clouds.rs`, matching the shader’s wrapping 64-bit hash and cloud equations. `cargo test -p ve-sky` passed (32 integration tests); `cargo clippy -p ve-sky --lib -- -D warnings` passed. Formatted only `clouds.rs` with the required rustfmt command; it is 75 lines. No tests or shader files were changed.
