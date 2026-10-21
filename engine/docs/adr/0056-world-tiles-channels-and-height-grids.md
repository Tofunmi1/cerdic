# ADR 0056: World tiles, attribute channels and height grids

- Status: proposed (owner confirmed the direction 2026-10-21; details open to overturn)
- Date: 2026-10-21

## Context

`terrain.heightfield` is one noise function as a closed solid slab, at most 192 cells per side, with no masks, no erosion and no way to build a world larger than one object. `inst.scatter_on` filters by normal only and keeps no per-point data. Meshes carry positions, corner UVs and a material, so nothing can say "this vertex has wind weight 0.4" or "this cell is 70 percent rock". Shipped open worlds are tiled, deterministic and locally stable, and layer masks to drive materials and placement (plan: `docs/plans/world-generators.md`).

## Decision

1. **A world is tiles.** A world has a global seed and a tile size (default 256 m). Every generator is a pure function of world coordinates and the seed, so a tile can be built alone and neighbours agree on shared edges. Tests: a window cut from a large grid equals the same window generated alone, bit for bit; adjacent tiles share edge heights; paths and placements crossing tiles equal the joined result.
2. **`HeightGrid` is the terrain value** (`ve_world::terrain`): row-major heights, a cell size, a world origin, and named `f32` layers (masks). It is not a solid. Nodes turn it into a mesh, a vertex texture or a closed slab when needed. Tile sides are at most 2048 cells.
3. **Attribute channels.** Point sets, curves, grids and meshes carry named channels (scalar `f32`, vector `[f32; 3]`); a capture node freezes a computed channel so later nodes read the stored copy (the Blender `Capture Attribute` idea). Meshes gain per-vertex channels (colour, wind data, masks) next to the corner UVs; `ve-scatter` provides `PointSet`.
4. **Erosion in two scales.** A coarse global stream-power pass fixes drainage and relief for the whole world; each tile is refined with thermal and hydraulic passes on the tile plus a halo (cells of reach), pulled toward the coarse result so large-scale shape cannot drift between tiles.
5. **Determinism.** Authoritative results are CPU code using `+ - * /`, `sqrt` and integer hashing; any GPU kernel ships later with a CPU reference and a parity test. Randomness is SplitMix64 streams keyed by world seed and cell coordinates, never by the order work runs.
6. **Local coordinates.** Meshes are `f32`, so each tile's vertices are stored relative to the tile origin and the renderer places the tile with a double-precision (or camera-relative) offset. Nothing stores kilometre-scale absolute `f32` positions.
7. **One pure crate, many modules.** `ve-world` holds `terrain`, `erosion`, `scatter`, `flora`, `lod`, `river` (and `rock` later) as modules; it depends on nothing in the engine (no graph, Bevy or platform code). `ve-modeling`/`ve-graph` wire it to nodes, parameters, estimates, cancellation and the guide. (Owner decision 2026-10-21: the workspace already has too many crates, so one crate per family was rejected.)

## Consequences

- The old `terrain.heightfield` slab stays for small landscapes and gets a note pointing at the new family; nothing is removed.
- Estimates and the result cache must understand tile values (memory is `width * depth * 4` bytes per layer; 8-bit packing for splat masks).
- `Value` gains optional, defaulted fields only (grid, point set, vertex channels), so existing documents and tests are unaffected.
- Renderer work follows in ADR 0057 (clipmap terrain, splat-layer material, triplanar) and ADR 0058 (GPU-driven vegetation, replacing the instance cap of ADR 0054).

## Alternatives considered

- Keep terrain as a solid: simple, but it cannot carry masks, stream, or exceed one object's budget.
- Global (non-tiled) erosion at full resolution: seam-free but limited to one machine-sized grid and not streamable.
- Pure noise terrain with no simulation: perfectly tile-local, but no drainage, rivers or believable valleys.
- GPU-first generation: faster, but not bit-identical across devices; kept for preview later.

## Evidence

Tests in `crates/ve-world/tests/terrain.rs`, `erosion.rs` and `erosion_quality.rs` encode tile invariance, mask definitions and erosion invariants; they fail until the algorithm bodies land (handoffs `docs/handoff-codex-terrain-grid.md`, `docs/handoff-codex-erosion.md`).
