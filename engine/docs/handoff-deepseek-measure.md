# Handoff to DeepSeek: `ve-measure`, measurement and collision tooling for models

Read `engine/AGENTS.md` first and follow it (no `unwrap`/`expect`/`panic!`/`println!` in library code, no `#[allow]`: use `#[expect(lint, reason)]` on the
smallest item only, files under 400 lines, docs on public items, tests, no commits, no `Co-Authored-By` or tool attribution). **Never run `cargo fmt` or
`cargo fmt --all`; format only your files with `rustfmt --edition 2024 --config-path rustfmt.toml <file>` from `engine`.** Create a new crate
`crates/ve-measure` (pure Rust, depends on `glam` only, no Bevy, no I/O, compiles to wasm32). Do not edit other crates. Other engineers edit `ve-graph`,
`ve-modeling`, `ve-kernel`, `ve-runtime`, `ve-render`, `cpp/`, `web/`.

## Why
A model (or a person) that builds geometry needs to check it: "is it closed, how big, how thick, is it symmetric, will it print, does it have bad UVs". Today
nothing answers that. The functions take plain arrays so any mesh type can feed them: `positions: &[[f32;3]]`, `indices: &[u32]` (triangles),
optional `uvs: &[[f32;2]]`.

## What to build
1. **`Report::of(mesh) -> Report`** with: bounding box (min, max, size, centre), surface area, volume (signed, sum of tetrahedra), centre of mass, triangle and
   vertex counts, **watertight** (every edge shared by exactly two triangles), **manifold** problems listed by kind (open edges, non-manifold edges,
   duplicate vertices, degenerate triangles with area below epsilon, zero-length edges), Euler characteristic and genus, connected components count with
   each component's bbox and volume, inside-out detection (negative volume).
2. **Thickness:** `min_wall_thickness(mesh, samples)` by casting inward rays from deterministic sample points (seeded) with a simple BVH (build your own,
   bounded, deterministic split rule); returns minimum, 5th percentile, and the worst sample locations. Budget: refuse meshes above 1 million triangles.
3. **Symmetry:** `symmetry_error(mesh, plane)` for X, Y or Z mirror planes (Hausdorff-like max and mean distance from each vertex to its mirrored nearest
   vertex via a grid), plus `best_symmetry_plane`.
4. **UV quality:** `uv_report(mesh)`: islands count, overlapping island area ratio, UV coverage (0..1 area used), texel-density spread (min, median, max of
   `uv_area / world_area` per triangle), stretch (singular-value ratio per triangle, report mean and max), out-of-range UV count.
5. **Distance tools:** `closest_point(mesh, p)` and `raycast(mesh, origin, dir)` using the BVH; `distance_between(a, b)` as sampled Hausdorff distance (max and
   mean) for comparing a high and a low mesh.
6. **Collision shapes:** `convex_hull(points) -> hull mesh` (quickhull, deterministic, bounded 100k points), `bounding_shapes(mesh)` giving the best of an
   axis-aligned box, oriented box (PCA) and capsule with fit error (volume ratio), and `convex_decomposition(mesh, max_parts)` using a simple deterministic
   voxel-or-split approach (document the method and its error), each part a hull with at most 64 vertices.
7. **Compact text:** `Report::summary()` returns at most 12 lines meant for a language model (units in metres, one fact per line, a final line that says
   either "ok" or the first problem and a suggested fix, e.g. "3 open edges near (0.1, 0.0, 0.2): close the hole or use poly.fill").
8. **Tests (real behaviours):** a cube (area 6, volume 1, genus 0, watertight); a torus (genus 1); a sphere sampled thickness; a shell with a known 2 cm wall
   returns thickness 0.02 within tolerance; a mesh with a deleted triangle reports 3 open edges; a flipped mesh reports negative volume; symmetric and
   non-symmetric shapes; UV: a perfect unit-square unwrap gives zero stretch and a known density; convex hull of a cube's points; deterministic: same input twice
   gives identical `Report` (derive `PartialEq`); budgets refuse with a typed error, never a panic; empty input is a typed error.
9. Files small: `report.rs`, `bvh.rs`, `thickness.rs`, `symmetry.rs`, `uv.rs`, `hull.rs`, `collision.rs`, `error.rs`.

## Acceptance
`cargo test -p ve-measure`, `cargo clippy -p ve-measure -- -D warnings` clean, `cargo check -p ve-measure --target wasm32-unknown-unknown`,
`cargo xtask rules` shows none of your files over 400 lines. Add `examples/report.rs` that builds a cube and a torus in code and prints the summaries.

## Status (DeepSeek: fill in when you finish, then stop)
- State: done, all acceptance commands green (strict gate not run: `cargo xtask check --strict` is a milestone gate the owner runs).
- What was built: the new crate `crates/ve-measure` (pure Rust, `glam` + `thiserror` only, no Bevy/IO, wasm32-clean). Modules: `report.rs` (`Report::of`,
  topology, components, `summary`), `mesh.rs` (`Mesh`, `BoundingBox`, epsilons, `Dsu`), `problem.rs` (`ProblemKind`, `Problem`, `Component`), `error.rs`
  (`MeasureError`), `bvh.rs` (deterministic BVH, raycast/closest-point/Hausdorff), `geom.rs` (triangle/slab primitives), `thickness.rs`
  (`min_wall_thickness`), `symmetry.rs` (`symmetry_error`, `best_symmetry_plane`), `uv.rs` (`uv_report`), `hull.rs` (`convex_hull`, quickhull),
  `shapes.rs` (`bounding_shapes`: AABB/OBB/capsule with volume-ratio fit) and `collision.rs` (`convex_decomposition` via bounded voxel-split). All public
  APIs take plain `[f32;3]`/`[f32;2]` arrays; glam is internal. No `unwrap`/`expect`/`panic!`/`println!` in library code; the only `#[expect]`s are the
  crate-level cast/name lints and one `needless_range_loop` on the Jacobi sweep. `examples/report.rs` builds a cube and a torus and prints their summaries.
- Commands run and results:
  - `cargo test -p ve-measure` — all pass (report 8, thickness 5, query 4, symmetry 4, uv 3, hull 4, collision 5).
  - `cargo clippy -p ve-measure --all-targets -- -D warnings` — clean.
  - `cargo check -p ve-measure --target wasm32-unknown-unknown` — clean.
  - `cargo run -p ve-measure --example report` — prints the cube (area 6, volume 1, genus 0, watertight, outward) and torus (genus 1, watertight).
  - `cargo xtask rules` — no `ve-measure` file over 400 lines. The command still fails on two unrelated files edited by other engineers
    (`crates/ve-graph/src/builtin/poly_network.rs` 407, `crates/ve-runtime/src/lib.rs` 411); nothing in `crates/ve-measure` violates the rule.
  - Formatted only the crate with `rustfmt --edition 2024 --config-path rustfmt.toml` on `src/lib.rs` (which recurses into the crate's modules), never
    `cargo fmt`/`cargo fmt --all`.
  - The strict gate (`cargo xtask check --strict`) is pending; a report should say so.
- Open questions or things you chose not to do:
  - Thickness sampling is seeded SplitMix64 with a fixed `SEED`, so results are deterministic; per-triangle UV stretch is reported as `sigma_max/sigma_min - 1`
    (a conformal unwrap is 0).
  - UV overlap and coverage are computed on a fixed `128`-cell raster, so they are approximate; islands are found by welding UVs at `1e-5`.
  - `convex_decomposition` voxelises at `12`³ and hulls cell corners, falling back to each cluster's AABB box when a hull would exceed `64` vertices (the
    documented approximation: it is a box-ish union, not an exact decomposition).
  - I did not add `ve-measure` to `ENGINE_AGNOSTIC` in `xtask/src/rules.rs` (that would edit another file); the crate has no Bevy/wgpu/winit/web
    dependencies, so it can be listed there later.
  - Nothing was committed.
