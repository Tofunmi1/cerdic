# Handoff: `body.hair` `fine` and `cover`, hair that reads as hair

Owner: Claude. Helper: Codex. Plain Rust.

## Why
The owner looked at the person and said the hair looks like trash. Today `body.hair` grows 200..2000 wide flat ribbons (0.016 m) from the scalp; the result is a few dark flat
sheets with gaps between them, bare scalp showing through on top, no strand detail, and no UVs so no texture can give it root-to-tip colour or streaks.

## The test (fixed, do not weaken)
`crates/ve-modeling/tests/g30_hair_look.rs` (5 tests). Do not edit it.
Run: `CARGO_TARGET_DIR=target-codex-hair cargo test -p ve-modeling --test g30_hair_look`. Keep `cargo test -p ve-graph` and the other `body.hair` tests green.

## What to build
Two new optional parameters on `body.hair@1`; defaults keep today's output bit-identical (the test checks `fine=0, cover=false` equals the plain output):
- `fine` Num (Ratio 0..1, default 0): 0 today's wide ribbons. Towards 1 every wisp is split into several fine strands: strand width falls to about 2.5 mm at 1, each strand has
  its own small random offset across the wisp, its own length (80..105% of `length`), a slightly different direction and waviness, so the hair has individual strands and a
  ragged tip. More strands per wisp (up to 8). Keep the total under 400,000 triangles.
- `cover` Bool (default false): adds an under-layer so no scalp shows: a hair cap, a thin smooth shell over the whole scalp region inside the hairline (raised about 3 mm
  off the skin, following the head shape the way the current `add_cap` function does; look at it and improve it, it clearly leaves gaps), plus a dense inner layer of short
  strands near the roots. The test casts rays from the middle of the skull through the crown, back and sides and requires at least 99% to hit hair.
- UVs for the hair (when `fine > 0` or `cover`; leave the default output as it is): two floats per triangle corner; u across the strand 0..1, v from 0 at the root to 1 at the tip.
  The cap shell gets v from 0 at the hairline to 1 at the crown, u around.
- Hair never sinks into the skin (more than 1.5 mm off the skin everywhere) and stays out of the face.
Draw both sides as the current ribbons do (check how they handle double-sided drawing).

## Where
`crates/ve-modeling/src/human_hair.rs` (split into `human_hair/` modules if it would exceed 400 lines), the `hair()` function in `crates/ve-graph/src/builtin/human.rs` (the two params),
the cost estimate where `human_hair::triangles` is used, the guide topic "ANIMATING A PERSON" in `tools/mcp/guide-creatures.mjs` (one sentence), and `docs/modeling/` where `body.hair` is listed.

## Rules
- You may edit only the files named above. Another helper is editing `crates/ve-modeling/src/human_rig/*`, the `body.garment` spec in `ve-graph/src/builtin/human.rs` (only its garment function: do not touch it) and `docs/handoff-garment-tailor.md`. Never run `git checkout`, `git clean`, `git stash`, `git restore`. No commits.
- Do not run `cargo fmt`; use `rustfmt --edition 2024 --config-path rustfmt.toml <file>` on your own files.
- `CARGO_TARGET_DIR=target-codex-hair`, no `cargo xtask`. No `unwrap`/`expect`/`panic!` in library code, no `#[allow]`, files under 400 lines. Delete `target-codex-hair` when you finish (the disk is nearly full).
- Write the final Status below yourself (your final message is not saved). Say honestly what you did not do.

## Status
(not started)
