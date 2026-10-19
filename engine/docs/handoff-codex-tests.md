# Handoff to Codex (round 4): write and RUN the missing tests for `poly.pack_uv` and `poly.lod`

Read `engine/AGENTS.md` first and follow it (no `unwrap`/`expect`/`panic!`/`println!` in library code, no `#[allow]`, Rust files under 400 lines, tests, no commits, no
`Co-Authored-By` or tool attribution). **Never run `cargo fmt`/`cargo fmt --all`; format only your Rust files with `rustfmt --edition 2024 --config-path rustfmt.toml
<file>` from `engine`.** Do not build the browser bundle. No network. Do not touch any non-test source unless a test proves a bug, and then say exactly what.

The two operators already exist and compile (I fixed the triangulate bug in `cpp/blender/ops/mesh_ops.cc`; `cargo test -p ve-kernel --test mesh_ops` passes). What is
missing is **evidence that they work**. This round is only tests. Create one new file `crates/ve-modeling/tests/g11_uv_lod.rs` (under 400 lines; model the helpers on
`tests/g11_round_edges.rs`, which builds a session, evaluates, and reads `parts[0]`). Cargo may wait on a build lock: wait, do not stop. If a test fails, report the
failure honestly and say which side is wrong; do not weaken the assertion to pass.

## Tests to write (each one a separate `#[test]`, real assertions, numbers printed in the failure message)
Programs in the text language. A box: `solid.box@1(size=[1,1,1])` then `poly.from_solid@1`, `poly.merge_coplanar@1`. Get UVs with `poly.unwrap_uv@1` (see its params in
`crates/ve-graph/src/builtin/poly_network.rs` or `grep -rn "unwrap_uv" crates/ve-graph/src`), then `poly.pack_uv@1`.
To read UVs and triangles in a test use the same route `tests/g10_blender_ops.rs` uses for UV checks (look at how it reads corner UVs from the evaluation or the poly mesh);
if there is no way, expose nothing new: evaluate `poly.to_solid` and read `parts[0]`'s UVs if present, otherwise say so.

pack_uv:
1. All packed UVs lie in 0..1 (min and max per axis printed).
2. Island overlap is zero: rasterise each island's triangles into a 256x256 grid in the test, island id per cell; no cell may be claimed by two islands.
3. Coverage: fraction of cells covered is above 0.5 for the unwrapped box.
4. `margin=0.05` leaves at least 0.04 gap between islands (test by rasterising with 512 grid and checking no two islands' cells are within 0.04).
5. `rotate=false` keeps each island's bounding-box aspect ratio equal before and after packing within 2% (aspect before packing is the island's own, ignoring scale).
6. `keep_density=true`: per-island ratio of UV area to 3D area equal across islands within 5%.
7. Determinism: two evaluations give bit-identical UVs.
8. Refusal: pack_uv on a mesh with no UVs returns a diagnostic whose suggestion mentions `poly.unwrap_uv`.

lod (`poly.lod@1`, parameters `levels`, `level`, `ratio`, `protect_uv_seams`, `protect_boundary`; build a sphere-like mesh with `solid.sphere@1(radius=0.5, segments=48)` then `poly.from_solid`):
9. Triangle count of level 1 and level 2 are within 10% of ratio^level times the input count (print all three counts).
10. Bounding box of each level differs from the input's by less than 2% of its size.
11. Each level converts back with `poly.to_solid` (watertight) when the input does.
12. Same input twice gives the same triangle counts and volume to 1e-12.
13. `protect_uv_seams=true` refuses with code `poly.lod_protection_unsupported` and a suggestion.
14. A ratio that would collapse the mesh refuses with `poly.lod_collapse`.

## Acceptance
`cargo test -p ve-modeling --test g11_uv_lod` shows every test result, pass or fail, with real numbers. Then `cargo test -p ve-kernel -p ve-geometry -p ve-modeling -p ve-graph` once
and report the list of failing tests if any (not just "failed"). `cargo clippy -p ve-modeling --tests -- -D warnings` shows nothing new from your file. `cargo xtask rules` shows
your file under 400 lines.

## Status (Codex: fill in with real numbers, then stop)
- State: not started
- Test results (one line per test: pass/fail and the key numbers):
- Failing tests anywhere in the package run:
- Bugs found in pack_uv or lod (and what you changed, if anything):
