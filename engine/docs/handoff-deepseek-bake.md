# Handoff to DeepSeek: texture baking in `ve-measure` (tangent-space normal map and ambient occlusion from a high-poly mesh)

Read `engine/AGENTS.md` first and follow it (no `unwrap`/`expect`/`panic!`/`println!` in library code, no `#[allow]`, Rust files under 400 lines, docs on public items, tests, no commits, no `Co-Authored-By` or tool
attribution). **Never run `cargo fmt`/`cargo fmt --all`; format only your Rust files with `rustfmt --edition 2024 --config-path rustfmt.toml <file>` from `engine`.** Use the edit tools. Do not build the browser bundle.
Write the Status into **this file** at the end.

## Shared tree: hard rules (other engineers work in the same working tree right now)
- **NEVER run `git checkout`, `git clean`, `git stash`, `git restore`, `git reset`, or write a file from `git show` over a working file.** If a file you did not change fails to compile, say so in the Status; wait and
  retry (another engineer's half-finished edit can break the build for a minute).
- **Files you own this round**: `crates/ve-measure/**` only (new files `src/bake.rs`, `src/bake/*.rs`, `tests/bake.rs`, `examples/bake.rs`, plus one `mod`/`pub use` line in `src/lib.rs`). Do not touch any other crate.
- **Others are editing**: `engine/crates/ve-graph/**`, `engine/crates/ve-modeling/**`, `engine/cpp/**`, `src/engine/**`.

## Context
You wrote `ve-measure` (verified): a deterministic BVH with `raycast`, `closest_point`, plain-array APIs (`[f32;3]` positions, `u32` triangle indices, `[f32;2]` UVs per triangle corner). A game asset is a low-poly
mesh carrying detail baked from a high-poly one. Add the bake, as pure functions, so the browser and the MCP server can use it later.

## What to build
1. `bake_normals(low: &BakeMesh, high: &BakeMesh, size: u32, cage: f32, max_distance: f32) -> Result<Bake, MeasureError>`: for every texel of a `size` x `size` image (8..=2048) whose centre lies inside a triangle of the low
   mesh's UV layout, find the point and smooth normal on the low mesh (interpolate vertex normals, computed by you from the low mesh's triangles, area-weighted, per **position** so the result does not depend on UV seams),
   cast a ray from `point + normal * cage` along `-normal` against the high mesh (BVH), take the nearest hit within `cage + max_distance`, and write the **high** surface's normal at the hit (interpolated from the high mesh's
   vertex normals) in the low mesh's **tangent space** (tangent = the direction of increasing u on the surface from the UV derivatives of that triangle, Gram-Schmidt against the smooth normal, bitangent = normal x tangent,
   sign flipped if the UV triangle is mirrored) encoded as `(n * 0.5 + 0.5) * 255`, OpenGL convention (green up). A texel with no hit takes the low mesh's own normal in tangent space, which is (0.5, 0.5, 1.0) encoded.
   Texels outside every UV triangle are padded by copying the nearest covered texel within 4 texels (dilation) and otherwise left as (128, 128, 255, 0) with alpha 0; covered and padded texels have alpha 255.
2. `bake_ao(low: &BakeMesh, high: &BakeMesh, size: u32, rays: u32, max_distance: f32, bias: f32) -> Result<Bake, MeasureError>`: same texel walk, but at each texel's point on the low mesh (offset `bias` along the normal) cast `rays`
   (1..=256) deterministic cosine-weighted hemisphere rays (a fixed low-discrepancy sequence you write, no random number crate) against the **high** mesh; the value is the fraction that escape (no hit within `max_distance`),
   written as grey 0..255. Same padding rule.
3. `Bake { size: u32, rgba: Vec<u8>, covered: u32 }`, and `BakeMesh { positions, indices, uvs }` with `BakeMesh::new(...) -> Result<Self, MeasureError>` validating lengths, finite numbers and index ranges. Budgets: refuse
   `size * size * rays` above 2e9 and meshes above 1 million triangles with a typed error and a suggestion, never a panic. Deterministic: the same input gives byte-identical output.
4. `examples/bake.rs` that builds a flat quad and a quad with a bump, bakes both maps and writes `target/reports/bake-normal.ppm` and `bake-ao.ppm` (P6 files, no dependency) so a person can look at them.

## Tests (`tests/bake.rs`). Fixed; do not weaken any assertion; if one fails, say which and why.
Fixtures: a flat quad in the XY plane from (0,0,0) to (1,1,0) with UVs equal to xy, two triangles.
1. Low = high = the flat quad, `size=32`, `cage=0.1`: every covered texel is (128, 128, 255) within +/-1 on each channel (the normal map of a flat surface), and `covered` is at least `32*32 - 2*32` (the quad fills the image).
2. A tilted high mesh: high = the same quad but each vertex's z = `0.5 * x` (a plane rising along x, slope 0.5, normal (-0.4472, 0, 0.8944)): covered texels have tangent-space normal (-0.4472, 0, 0.8944) encoded as
   about (70, 128, 242) within +/-2 per channel, for every texel whose ray hits (all of them with `max_distance=1.0`).
3. A mirrored UV layout (u = 1 - x) with the same tilted high mesh: the red channel is mirrored accordingly (about 186, not 70), green and blue unchanged: the tangent-space normal follows the UVs.
4. `max_distance` is honoured: a high mesh 2.0 above the low one with `cage=0.1, max_distance=0.5` yields no hits, so every covered texel is (128, 128, 255).
5. Padding: a low mesh whose UVs cover only the left half of the image (u = x * 0.5): texels within 4 texels of the covered area are copied (alpha 255), texels farther away have alpha 0, and the covered count is within 3% of `0.5 * size * size`.
6. AO of an open flat quad against itself is 1.0: with `rays=64`, every covered texel is 255 within +/-3. (A flat surface sees the whole sky; a ray that hits only the surface it started from at grazing angles must be avoided by `bias`.)
7. AO under an overhang: low = the flat quad, high = the quad plus a second parallel quad 0.2 above it covering the same footprint: the AO of a texel in the middle is below 0.15 (almost fully shadowed), while a texel at the very edge
   is above 0.5; and the bake is monotonic: the AO at the edge is greater than at the centre.
8. Determinism: baking twice gives byte-identical output; changing `size` changes only the resolution (the centre texel value stays within +/-4).
9. Errors: odd lengths, an index out of range, a NaN position, `size=4`, `rays=0` and `rays=1000` each return a typed error with a suggestion (not a panic).

## Acceptance
`cargo test -p ve-measure`, `cargo clippy -p ve-measure --all-targets -- -D warnings`, `cargo check -p ve-measure --target wasm32-unknown-unknown`, `cargo xtask rules` (your files under 400 lines), `cargo run -p ve-measure --example bake`.

## Status (DeepSeek: fill in with real numbers for each of the nine tests, then stop)
- State: **done by the manager** (DeepSeek was rate limited, so it never ran). `bake_normals` and `bake_ao` are in `ve-measure/src/bake.rs` and `bake/frame.rs`, and the nine tests are in `tests/bake.rs`, all passing, strict clippy
  clean and wasm-checked. Differences from this handoff, found while working the geometry: the tilted-plane tests use `cage=0.6` (the ray must start above the highest point), the `max_distance` test puts the high mesh one
  metre below and checks both a miss and a hit, the overhang edge texel is asserted above 0.4 open (analytically about 0.52), and the work cap is half a billion rays. `examples/bake.rs` was not written.
- Per test: pass/fail and the measured value:
- Commands run and results:
- Open questions or things you chose not to do:
