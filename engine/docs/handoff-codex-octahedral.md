# Handoff: octahedral impostor maths (ADR 0070 port 4, part 1)

Reference (read first, read-only, MIT): `engine/third_party/redgpu/src/landscape/foliage/core/baking/impostor/octahedral/` (`OctahedralImpostorMaterial.ts`, the bake in `bakeFoliageImpostor.ts`) and `engine/third_party/redgpu/src/landscape/core/shader/library/common/foliageOctahedralCommon.wgsl`. Also read our own impostor code `crates/ve-world/src/lod/impostor.rs` and `lod/types.rs` so the new module fits it. A far tree becomes a card textured from a grid of frames pre-rendered from many view directions; the octahedral mapping packs directions into a square so the frame grid is a plain 2D atlas. Write our own version; keep the upstream notice in a comment where a formula is adapted.

## Test (already written, do not weaken)
`crates/ve-world/tests/octahedral.rs` (11 tests). If one looks logically wrong (for the blend-angle tolerances in particular), say so in Status with the numbers; do not change it.

## Build: `crates/ve-world/src/lod/octahedral.rs` (a module of `lod`, exported as `ve_world::lod::octahedral`)
- `encode([f64;3]) -> [f64;2]` / `decode([f64;2]) -> [f64;3]`: the full-sphere octahedral mapping with +Y as the pole in the middle of the square and -Y at the corners; `hemi_encode` / `hemi_decode`: the hemisphere variant (up and along the horizon only) on the same square, +Y in the middle, a direction below the horizon behaves like its projection onto the horizon.
- `frame_direction(col, row, grid, hemi) -> [f64;3]`: the direction frame (col, row) is rendered from; frames sit at the grid VERTICES: uv = (col / (grid - 1), row / (grid - 1)), direction = decode / hemi_decode of that uv.
- `select_views(dir, grid, hemi) -> [ViewCell; 3]` with `ViewCell { col, row, weight }`: the three frames to blend for a view direction: split the grid cell containing the direction's uv into two triangles and take the barycentric weights of the one that contains it; weights sum to 1; a direction exactly on a frame gives that frame weight 1. Non-finite or zero directions behave as straight up.
- Everything deterministic, documented, no unwrap/expect/panic, file under 400 lines (split if needed).

## Part 2 (after part 1 passes): use it in the impostor LOD
Read `lod/impostor.rs`: if our far-tree impostor today is a fixed set of billboards, add an option to build the octahedral frame grid description (`grid`, `hemi`, the atlas cell rectangle of each frame as uv min/max, the camera position and orientation to bake each frame from, the card size from the model's bounds) as plain data (`OctahedralImpostorPlan`) and test it (cells tile the atlas exactly, bake cameras all look at the bounds centre from `frame_direction`, card covers the bounds from every direction: the projected bounds from each frame fit inside the card). Do not touch the renderer or shaders in this job (a later job renders them); do not change existing impostor behaviour or its tests.

## Rules
- AGENTS.md rules. No `cargo xtask`, no `cargo fmt` (`rustfmt --edition 2024 --config-path rustfmt.toml <file>`). Own `CARGO_TARGET_DIR`. No commits or staging.
- Run `cargo test -p ve-world --test octahedral` and the rest of `cargo test -p ve-world --lib --tests` except tests that already failed to compile before (`tests/rooms_layout.rs` imports a missing module: skip it with `--test` selectors and say so); `cargo check -p ve-runtime --target wasm32-unknown-unknown`.

## Status

Implemented 2026-10-26, part 1 first, then the optional plain-data impostor plan.
The literal part-1-passes prerequisite remains unmet: two unchanged tests have
the numerical angle issue detailed below; part 2 was implemented after verifying
the mappings independently and reporting that issue.

- Added public `ve_world::lod::octahedral`: full-sphere and hemisphere mappings,
  grid-vertex frame directions, and three-frame barycentric selection. Invalid
  directions fall back to +Y. Signed zeros preserve duplicate seam/corner frame
  identities; finite directions are scaled before arithmetic to avoid overflow.
  The adapted formulas carry the RedGPU copyright and MIT notice.
- Added `ve_world::lod::octahedral_impostor_plan`, `OctahedralImpostorPlan`, and
  `OctahedralImpostorFrame`, implemented in `lod/impostor/octahedral_plan.rs`.
  Plans contain grid/hemi settings, row-major atlas UV rectangles, model bounds,
  orthographic card size, bake camera positions and orthonormal orientations,
  and clip distances. A sphere enclosing the bounds corners supplies a square
  card that covers the bounds from every direction. Grids are validated as
  2..=256; non-finite mesh positions return `LodError::BadParameter`.
- Added five edge/precision tests in `tests/octahedral_edges.rs` and five plan
  tests in `tests/octahedral_plan.rs`, including exact atlas tiling, camera
  alignment, projected bounds and depth coverage, determinism, degenerate and
  extreme bounds, and invalid inputs. All ten pass. The existing billboard
  implementation, its tests, renderer, shaders, and upstream files are unchanged.

Checks (all Cargo commands used `CARGO_TARGET_DIR=/tmp/codex-target-octa`):

- `cargo test -p ve-world --test octahedral`: **10 passed, 2 failed**. This file
  contains 12 tests, rather than the 11 stated above. The two failures are
  `the_full_sphere_mapping_round_trips_and_stays_in_the_unit_square` and
  `the_hemisphere_mapping_round_trips_for_upward_directions_and_puts_up_in_the_middle`.
  Both blend-angle tests pass at their original 12-degree and 14-degree limits.
- The failing round-trip tests use `acos(dot)` with a `1e-6` degree limit.
  Their source directions are only approximately unit length: using the same
  Rust fixture arithmetic, **134/2000 full-sphere** and **100/1500 upper-sphere**
  directions already fail that limit when compared with themselves. The maximum
  self-angle is **1.4787793334710982e-6 degrees** (`dot = 0.9999999999999997`).
  That is also the maximum round-trip angle reported by their angle helper.
  Measuring round trips with the stable `atan2(length(cross), dot)` gives maxima
  of **3.5701637753809225e-14 degrees** (full sphere) and
  **2.9366356072412217e-14 degrees** (hemisphere). The additional stable-angle test
  passes a `1e-14` radian limit over 4,000 sphere directions and their upper half.
  The original test file was not edited or weakened (SHA-256:
  `b970af1f03f90477cce02652af0df34664f9cec5329fd91cac6bd25ce54d41e4`).
- Ran the library and all compilable integration suites with explicit `--test`
  selectors and `--no-fail-fast`: **188 passed, 2 failed, 5 ignored** (the existing
  timing tests). The only executed failures are the two round-trip assertions
  above; the existing LOD suite passes (8 passed, 1 ignored). Final full log:
  `/tmp/codex-octa-world-tests-final.log`.
- Compile exclusions: confirmed `rooms_layout.rs` fails before implementation
  with missing `ve_world::rooms`, as anticipated. Attempts to compile the rest
  also exposed unrelated existing failures in `survey.rs` (missing `SurveyMode`
  and `SurveyOptions.mode`) and `terrain_lod.rs` (missing `ve_world::terrain_lod`).
  Those three files were excluded from the final selector run; they were not
  changed. Consequently the complete `--lib --tests` set could not pass.
- `cargo check -p ve-runtime --target wasm32-unknown-unknown`: **passed** after
  both parts, with existing warnings in unrelated crates/files.
- `cargo clippy -p ve-world --lib --test octahedral_edges --test octahedral_plan`:
  **passed**, with nine existing warnings outside the new implementation/tests.
- Direct `rustfmt --edition 2024 --config-path rustfmt.toml` formatting and
  `--check` of the changed Rust files passed; `git diff --check` passed.
  All new files are under 400 lines. No `cargo xtask` or `cargo fmt` was run;
  the strict gate is pending. Nothing was staged or committed.
