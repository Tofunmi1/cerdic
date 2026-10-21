# Handoff to Codex: `field.noise`, bounded displacement of a field's surface

Read `engine/AGENTS.md` and `engine/docs/handoff-common.md` section 2 first and follow them (no `unwrap`/`expect`/`panic!`/`println!` in library code, no `#[allow]`:
use `#[expect(lint, reason)]` on the smallest item only, files under 400 lines, docs on public items, tests, **no commits, no `git checkout`/`clean`/`stash`/`restore`**,
no `Co-Authored-By` or tool attribution). **Never run `cargo fmt` or `cargo fmt --all`; format only your files with `rustfmt --edition 2024 --config-path rustfmt.toml <file>`
from `engine`.** Edit only these crates: `ve-geometry`, `ve-graph` (the builtin operator list), `ve-modeling` (`field_ops.rs`, `plan/fields.rs`). Another
engineer is editing `ve-runtime`, `ve-render` and `ve-bevy`; do not touch them.

## Why
Detail finer than the mesh has to live in the surface before meshing: wrinkles, scales and veins on a creature. We have `xform.noise`, but it runs after meshing, so it
can push vertices around and cannot make undercuts, and it cannot add detail finer than the mesh. A noise term inside the field makes the mesher put triangles where the
detail is. This is the first step of a plan to make generated bodies much more detailed (the next steps are adaptive meshing and GPU skinning).

## What to build
A new field node, reachable from the text language like every other node:

    rough = field.noise@1(ball, amplitude=0.05, scale=0.15, octaves=3, persistence=0.5, seed=7, mode="smooth")

It returns the input field with its distance value displaced: `d'(p) = d(p) - amplitude * n(p)`, where `n` is a 3D noise normalised to the range [-1, 1].
- `amplitude`: metres, 0 to 1e6, default 0.02. The surface moves by AT MOST this far (assuming the input's gradient is about 1). 0 changes nothing, bit for bit.
- `scale`: metres, 0.001 to 1e6, default 0.25. The wavelength of the first octave.
- `octaves`: integer 1 to 6, default 1. Each octave halves the scale. `persistence`: 0 to 1, default 0.5, the weight ratio between octaves. Normalise by the sum of the
  weights so the total stays within [-1, 1] for any octave count.
- `seed`: integer, default 0. Same seed gives the same shape on every platform (integer hashing only, no float tricks, no thread or time dependence).
- `mode`: `smooth` or `ridged` (ridged uses `1 - 2*abs(n)`, which makes creases and veins).
- The result has no distance guarantee (`DistanceGuarantee::None`), like `field.smooth_union`.
- Reuse the existing integer hashed value noise in `ve-modeling/src/noise.rs` if it fits; otherwise put a small one in `ve-geometry` and say which.
- Where the field is compiled to a tape (`ve-geometry/src/field/tape`), add the instruction there so meshing stays as fast as for the other nodes, and so wasm and native
  agree bit for bit.

Touch points (find them with `grep -rn "field.shell"`): `ve-geometry/src/field/{expr,model,tape}.rs` (the expression and its evaluation), `ve-graph/src/builtin/fields.rs` and
`ready.rs` (the operator definition with units, ranges, defaults and one line of help written for a model), `ve-modeling/src/field_ops.rs` (execution),
`ve-modeling/src/plan/fields.rs` (the work estimate: a noise node costs about `octaves` extra evaluations per sample; the planner's field-work budget must see it).

## Tests, already written
`crates/ve-modeling/tests/g12_field_noise.rs`. Do not weaken them. They check the baseline sphere volume, zero amplitude is identical, the surface stays within the amplitude
(volume between the spheres of radius 0.45 and 0.55), determinism and seed sensitivity, the `field.clipped` diagnostic when the amplitude exceeds the bounds, and that the
operator and its parameters are in the registry. Add unit tests of your own in `ve-geometry` for the noise function (range within [-1, 1] over a million samples, same seed
same value, different seed different value, octave normalisation). If a test of mine has a wrong premise, say so in the Status and explain, do not change it quietly.

## Acceptance
`cargo test -p ve-geometry -p ve-graph -p ve-modeling` passes, `cargo clippy -p ve-geometry -p ve-graph -p ve-modeling -- -D warnings` is clean,
`cargo check -p ve-geometry -p ve-modeling --target wasm32-unknown-unknown` passes, and no file of yours is over 400 lines.

## Decision on the blocker you raised (owner side, answered)
You found that meshing runs through a `ve-kernel` tape opcode and a C++ evaluator. **Yes: you may edit `ve-kernel`, `ve-kernel-sys` and `engine/cpp/` for this job** (add the
noise opcode and its C++ evaluation; keep the C ABI and the "no exceptions across it" rules from AGENTS.md; the Rust reference noise in `ve-geometry` must match the C++ one
exactly, so use integer hashing and the same float operation order in both, and add a test that evaluates the same points through both paths and compares bits or a 1e-6
tolerance). Do not touch `ve-runtime`, `ve-render`, `ve-bevy` or `web/`. Build and test the C++ the way the repo does (`cargo test -p ve-kernel` builds it natively); if the wasm
build needs the wasi-sdk and it is not found, say so in the Status instead of skipping silently.

## Status (Codex: fill in when you finish, then stop)
- State: Implemented `field.noise@1` through the Rust expression graph, compiled register tape, native C++ evaluator, graph registry, modeling evaluator, and planner/kernel work estimates. Added geometry noise unit tests and cross-checked Rust and C++ tape samples.
- What was built: Deterministic integer-hashed smooth value noise in `ve-geometry` (separate implementation from `ve-modeling/src/noise.rs` so the field evaluator has a pure geometry implementation that can match C++), octave normalization, smooth/ridged modes, bounded amplitude and parameter validation, zero-amplitude short-circuit, and `DistanceGuarantee::None` for nonzero displacement. Added `VE_FIELD_NOISE` to the tape and C++ evaluator. The C and Rust tape layouts remain unchanged.
- Commands run and results: `cargo test -p ve-geometry -p ve-graph -p ve-modeling` passed, including the six supplied `g12_field_noise` tests. `cargo test -p ve-kernel` passed, including native C++ field tests. Final focused reruns `cargo test -p ve-modeling --test g12_field_noise`, `cargo test -p ve-geometry field::noise`, and `cargo check -p ve-geometry -p ve-modeling --target wasm32-unknown-unknown` passed. The geometry unit test covers one million samples, range and deterministic hashing, octave normalization bounds, seed sensitivity, and Rust/C++ evaluator parity. `cargo xtask rules` and `git diff --check` passed. The all-target Clippy command did not pass: `ve-graph` reports existing `clippy::too_many_lines` errors in unrelated `curve_tools.rs`, `instances.rs`, `modeling/{animation,appearance,behavior,rig}.rs`, `poly_network.rs`, `poly_network_whole.rs`, `terrain.rs`, and `text/build.rs`; a geometry-only Clippy run also reaches an existing missing `# Errors` section on `ve-kernel/src/uv.rs::pack_uvs`. `cargo xtask cpp` passed native C++ format, sanitizer tests, and clang-tidy, then failed its wasm unresolved-symbol audit because `cpp/blender/unresolved.txt` contains `ZN7blender20BLI_system_backtraceEP7__sFILE`, which the audit reports as resolved or unused. Strict gate pending.
- Could not verify / open questions: No acceptance premise needed correction. The wasm Rust check passed with the installed wasi-sdk. `cargo xtask cpp` remains red only at the unresolved Blender symbol audit; this task did not alter that shared audit file. Clippy is blocked by the unrelated existing graph line-count lints and kernel UV docs lint listed above.
