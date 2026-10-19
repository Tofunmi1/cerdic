# Handoff to Codex (round 3, narrow): finish `poly.pack_uv`, then `poly.lod`

Read `engine/AGENTS.md` first and follow it (no `unwrap`/`expect`/`panic!`/`println!` in library code, no `#[allow]`, Rust files under 400 lines, docs on public
items, tests, no commits, no `Co-Authored-By` or tool attribution). **Never run `cargo fmt`/`cargo fmt --all`; format only your Rust files with `rustfmt --edition
2024 --config-path rustfmt.toml <file>` from `engine`.** Do not build the browser bundle. No network. The previous round ran long on task 2 and did not reach LOD:
**this round has two jobs only. Do not do the other Blender operators.** Stop and write the Status as soon as both are finished.

## Job 1: finish `poly.pack_uv` (already wired: C ABI, kernel entry, geometry wrapper, graph node in `builtin/poly_output.rs`)
Missing, in order:
1. Analytic tests through the node language (model them on `crates/ve-modeling/tests/g10_blender_ops.rs`): after `poly.unwrap_uv` (pack=false) then `poly.pack_uv`
   on a box, a sphere-ish mesh and a two-island mesh: all UVs inside 0..1, island overlap area is 0 (compute island bounding polygons or a rasterised
   coverage grid at 512x512 in the test), coverage above 0.5 for the box, `margin` honoured (the gap between islands at least margin minus 1%), `rotate=false`
   keeps island orientation, `keep_density=true` keeps each island's UV-area to 3D-area ratio equal within 5%, and the result is deterministic (two runs, same
   UVs bit for bit). One refusal test (a mesh without UVs says what to do first: run poly.unwrap_uv).
2. The parity corpus line (see how `G10-07` and `G10-08` are done) and confirm the existing corpus output is unchanged: compare with
   `/tmp/blender-ops-corpus-before.txt` (add only your new lines).
3. `cargo clippy -p ve-kernel -p ve-geometry -p ve-modeling -p ve-graph -- -D warnings` shows nothing new from you, and `cargo xtask rules` shows none of your files over 400.

## Job 2: `poly.lod`
A chain of simplified meshes from one polygon mesh, using the existing `poly.decimate` code path (read `poly_ops/blender.rs` and how `MeshOp::Decimate` is
applied). Put the spec in `builtin/poly_output.rs`, the run function in a new `poly_ops/lod.rs`, and register in `poly_ops.rs` like `round.rs` is. Parameters: `levels`
(1..4), `ratio` (per level, 0.05..0.9; level n has about ratio^n of the faces), `protect_uv_seams` (bool), `protect_boundary` (bool). The node's value is the
last (coarsest) level; every intermediate level is also reachable: add `poly.lod_level@1(in, level)` is NOT needed, instead make `poly.lod` take a `level`
parameter (0 = the input, 1..levels) so a program writes one node per level it wants. Tests: triangle counts follow the ratio within 10% on a sphere-like
mesh, bounding box drift under 2% of the size, still watertight (poly.to_solid succeeds) when the input is, UV seams preserved when asked (UV island count
unchanged), deterministic, and a refusal with a suggestion when the ratio would collapse the mesh.
Do not touch the glTF exporter in this round.

## Acceptance
`cargo test -p ve-kernel -p ve-geometry -p ve-modeling -p ve-graph`, clippy and rules as above, corpus unchanged except new lines.

## Status (Codex: fill in, then stop)
- State: partial; acceptance not met.
- Job 1: retained the existing targeted node-language UV test (bounded, bit-identical packed cube UVs, plus missing-UV refusal). Added one `G10-09` corpus fixture for cube unwrap then pack. No analytic assertions or measurements were added/run for zero overlap, >0.5 cube coverage, margin, rotate=false orientation, keep-density, sphere, or two-island meshes. `cargo xtask parity` passed: ve-kernel identical (31 lines), ve-modeling identical native/WASI (101 lines). Historical corpus comparison: `/tmp/blender-ops-corpus-before.txt` has 100 lines; current output has 101. The unified diff contains the new output and additional changed output lines, so corpus unchanged-except-one-line is NOT confirmed.
- Job 2: `poly.lod` now reads whole-number levels in its 0..4 domain and refuses requests for unsupported `protect_uv_seams` / `protect_boundary` with typed code `poly.lod_protection_unsupported` and a suggestion. The decimator API only accepts ratio, symmetry, and triangulate, so the flags cannot be passed through. Focused `cargo test -p ve-modeling --test g10_blender_ops`: 19 passed, including progressive sphere ratio check, conversion through `poly.to_solid`, collapse refusal, and both protection refusals. Bounding-box drift, explicit watertightness evidence beyond conversion success, seam-count preservation, and deterministic LOD output were not measured.
- Commands and results: `cargo test -p ve-kernel -p ve-geometry -p ve-modeling -p ve-graph` ran but failed at `ve-kernel/tests/mesh_ops.rs::triangulate_splits_cube_quads_into_twelve_triangles`; vendored Blender reports `min_vertices` is not a valid BMesh slot and returns `OperatorRefused`. `cargo clippy -p ve-kernel -p ve-geometry -p ve-modeling -p ve-graph -- -D warnings` failed on eight `too_many_lines` diagnostics in shared `ve-graph` files (curve_tools, modeling/{animation,appearance,behavior,rig}, poly_network, poly_network_whole, terrain); it did not report the changed `lod.rs`. `cargo xtask rules` completed with the file-size/dependency-boundary heading and no reported violation. Targeted rustfmt completed on the changed Rust files.
- Remaining acceptance gaps: add and run the requested full analytic UV suite; measure LOD bounds, seams, determinism, and watertightness; compare historical corpus with only the new line differing; resolve or isolate the unrelated package-test and clippy failures before claiming the acceptance gate. No workspace formatter, browser bundle, network, or commit was used. Strict gate remains pending.
