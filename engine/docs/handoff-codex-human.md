# Handoff to Codex: `ve-human`, the human body model from the CC0 pack

Read `engine/AGENTS.md` and `engine/docs/handoff-common.md` section 2 first and follow them (no `unwrap`/`expect`/`panic!`/`println!` in library code, no `#[allow]`:
use `#[expect(lint, reason)]` on the smallest item only, files under 400 lines, docs on public items, tests, **no commits, no `git checkout`/`clean`/`stash`/`restore`**,
no `Co-Authored-By` or tool attribution). **Never run `cargo fmt` or `cargo fmt --all`; format only your files with `rustfmt --edition 2024 --config-path rustfmt.toml <file>`
from `engine`.** Create the new crate `crates/ve-human` (the workspace takes `crates/*`) and touch nothing else except adding its dependencies; other engineers are editing
`ve-runtime`, `ve-render`, `ve-bevy`, `ve-modeling`, `ve-graph`. Read `docs/adr/0055-human-body-from-cc0-data.md` for why this exists.

## What this is
`assets/human/human.pack` is the CC0 MPFB2/Anny human, converted by `tools/human-pack/convert.py` (read it: it is the format's definition). The model is linear in its blend
shapes: `positions = template + sum_k coefficient_k * shape_k`. `ve-human` reads the pack and evaluates a body from named parameters. It is pure Rust, no `unsafe`, works on
`wasm32-unknown-unknown`, has no file access (it takes bytes), and is deterministic.

## The pack
One zstd frame (decode with the `ruzstd` crate, already in the lock file at 0.8.3: add it as a dependency with the streaming decoder; if a feature is needed say which) holding
`b"VEHUMAN\x01"`, `u32` LE JSON length, the JSON header, then the payload. The header has `vertex_count` (13,718), `render_vertex_count`, `rank`, `phenotype_labels` (11 names),
`phenotype_variations` (feature to the names of its prototype shapes, in order), `anchors` (feature to its increasing anchor values), `local_change_labels` (256), `facial_action_labels`
(52), `bone_labels` (104) and `sections`: `{name: {offset, dtype, shape}}` with offsets from the start of the payload, little-endian, 4-byte aligned. Sections:
- `template` f32 (V,3): rest positions. Our axes, metres.
- `phenotype_basis` i16 (rank,V,3) and `phenotype_basis_scale` f32 (rank): basis vector `r` is `value * scale[r]`. `phenotype_coefficients` f32 (624,rank).
  The 624 phenotype shapes are `shape_k = sum_r coefficients[k][r] * basis[r]`. Do not rebuild them: with weights `w_k`, the delta is `sum_r (sum_k w_k C[k][r]) * basis[r]`.
- `phenotype_mask` u8 (624,26): the weights `w_k = prod over the 26 columns j of (p_j * mask[k][j] + (1 - mask[k][j]))`, where `p` is the 26 prototype weights in the order of
  `phenotype_variations` iterated in the header's key order and each feature's names in order (race: african, asian, caucasian; gender: male, female; age: newborn, baby, child, young,
  old; muscle, weight, height, proportions, cupsize, firmness...).
- Prototype weights: for each of age, gender, muscle, weight, height, proportions, cupsize, firmness the value `x` (default 0.5) is clamped to the feature's anchors and turned
  into two adjacent hat weights `1 - alpha` and `alpha` between its two surrounding anchors (the other prototypes get 0): `idx = clamp(searchsorted_left(anchors, x), 1, n-1)`,
  `alpha = (x - anchors[idx-1]) / (anchors[idx] - anchors[idx-1])`. Race: `african`, `asian`, `caucasian` default 0.5 each, normalised to sum 1 (all zero gives thirds).
  `convert.py` has `hat_weights` and `phenotype_weights` as a working reference. Note gender 0 is the male prototype (the taller end).
- `face_*` and `local_*` sparse shapes: `*_offsets` u32 (N+1), `*_indices` u32 (nnz), `*_deltas` i16 (nnz,3), `*_scales` f32 (N): shape `i` has vertices `indices[offsets[i]..offsets[i+1]]`
  with deltas `value * scales[i]`. Face: 52 shapes, one per facial action, weight = the action value (0..1). Local: 512 shapes in pairs per local change `i`:
  shape `2i` for a value >= 0 with weight `value`, shape `2i+1` for a value < 0 with weight `-value` (so the parameter range is -1..1).
- `render_source` u32 (R), `render_uv` f32 (R,2), `triangles` u32 (T,3): the drawn mesh, vertices split at UV seams; render vertex `i` copies body vertex `render_source[i]`.
  `quads` u32 (Q,4) is the original quad faces on body vertices.
- `skin_bones` u8 (V,9) and `skin_weights` u16 (V,9) (divide by 65535): up to nine influences per vertex (unused slots have weight 0).
- `bone_heads` f32 (104,3), `bone_head_shapes` f32 (1188,104,3) (the same 1188 coefficients as the surface, in the order 624 phenotype, 52 face, 512 local; use the real weights
  `w_k` for the first 624), `bone_parents` i32 (104; the root has -1).

## API the tests use (crates/ve-human/tests/human.rs, already written, do not weaken)
`Pack::from_zstd(&[u8]) -> Result<Pack, Error>`; `Human::new(Pack)`; `Human::{vertex_count, bone_names, bone_parents, phenotype_labels, local_change_labels,
face_action_labels, skin, render_mesh, evaluate}`; `Params { phenotype, local, face }` (three `BTreeMap<String, f32>`, `Default`); `evaluate(&Params) -> Result<Body, Error>` with
`Body { positions: Vec<[f32;3]>, render_positions: Vec<[f32;3]>, bone_heads: Vec<[f32;3]> }`; `skin() -> &[[(u8, f32); 9]]`: the test only needs `skin().len()`
and each element iterating `(bone, weight)` pairs with `bone as usize`, so an array or a `Vec` of pairs both work; `render_mesh() -> &RenderMesh { source:
Vec<u32>, uv: Vec<[f32;2]>, triangles: Vec<[u32;3]> }`. Errors are a `thiserror` enum: an unknown parameter names itself and lists the valid names; values out of range (phenotype 0..=1,
face 0..=1, local -1..=1) are refused with the name and the range; a corrupt, truncated or wrong-version pack is an error, never a panic or an out-of-bounds index (validate every
section's offset, length and indices at load time). Evaluation should use f32 and be fast: skip shapes with zero weight, and accumulate the phenotype delta through the basis
(`rank` vectors), not through 624 dense shapes. Aim for under 10 ms for one evaluation natively.
Normals are not part of the body: the renderer computes them.

## Acceptance
`cargo test -p ve-human` passes, `cargo clippy -p ve-human --all-targets -- -D warnings` clean, `cargo check -p ve-human --target wasm32-unknown-unknown` passes, no file over 400 lines
(split `pack.rs`, `eval.rs`, `error.rs`...), docs on public items, a unit test of your own for the hat weights (anchor values give exact prototypes; values between give two weights summing
to 1; out-of-range is not extrapolated because it is refused earlier). If a test premise is wrong (for example `height` direction, the exact error wording, or a label name), say so in the
Status and explain; do not change my tests quietly.

## Status (Codex: fill in when you finish, then stop)
- State: Implementation and runtime acceptance pass. The exact all-target clippy command is blocked by a stale `#[expect(clippy::panic)]` in the protected `tests/human.rs`; do not modify that file without owner direction.
- What was built: The reference golden mismatch was fixed in the regenerated asset as described by the owner; no golden or integration test files were changed. Added strict validation for every section's dtype, shape, alignment, byte range and overlap, phenotype metadata and anchors, sparse offsets and vertices, render sources, triangle/quad indices, skin bone indices, and bone parents. Added corruption and hat-weight unit tests. Split evaluator work into helpers, added public error docs, and fixed library clippy findings. All Rust source files remain at or below 400 lines.
- Commands run and results: `cargo test -p ve-human` (2 unit tests and all 12 integration tests pass); `cargo clippy -p ve-human --lib -- -D warnings` (pass); `cargo check -p ve-human --target wasm32-unknown-unknown` (pass). `cargo clippy -p ve-human --all-targets -- -D warnings` fails only because `tests/human.rs:5` expects `clippy::panic`, but that test crate contains no panic lint occurrence, so `-D warnings` rejects the unfulfilled expectation. No tests were changed to work around it.
- Could not verify / open questions: All requested runtime acceptance is met, including load-time index safety and byte-corruption panic resistance. All-target clippy can become clean only by correcting/removing the stale expectation in the explicitly protected integration test, or by changing the lint invocation; neither was done. `cargo xtask quick` and `cargo xtask rules` were not run.
