# Plan v2: world generators to a AAA bar (terrain, placement, vegetation, rivers, weather)

Written 2026-10-21, revised the same day. Owner direction: every domain gets a geometry-nodes-like family of small composable nodes, reachable from the text language and LLM tools (memory `everything-via-language.md`); gameplay and humans come later. Owner asked whether v1 was AAA or close; **it was not** (section 2), so this version replans. Owner also asked that Claude (this session) and Codex work hand in hand (section 5).

## 1. What the code has today (facts)
- `Value` (`ve-modeling/src/cache.rs`) carries mesh, poly, field, instances, material... and **no per-point attributes**.
- `FieldExpr` is a signed-distance tree only; no general scalar field.
- `terrain.heightfield` is one noise function as a **closed solid slab**, at most 192 cells per side, no erosion, no masks, no LOD, no streaming.
- `inst.scatter_on`: normal filter only, at most 5,000 points per node, quadratic spacing, no per-point data. `InstanceSet` is matrices only, **capped at 16,384, and the viewer spawns one entity per placement** (ADR 0054 notes this).
- Wind is one global shader field (ADR 0051), single-level sway. No tree growth, erosion, rivers, impostors or LOD generation exists.
- Reusable: `ve-nav`, `ve-particles`, `ve-chain`, `ve-sky`, `ve-gi`, `water.surface`, `curve.tube`, `solid.sweep`, `poly.lod`, `poly.decimate`, the texture program system, the retained instance sets.
- Hard rules: native and WASM results match bit for bit (parity corpus), seeded SplitMix64, cancellation and estimate on every node.

## 2. Honest gap against AAA (why v1 fell short)
What shipped open-world games do, from the sources in the chat report, and where v1 stopped:
| AAA practice | v1 | Verdict |
| --- | --- | --- |
| World is tiled and streamed; kilometres across (Far Cry 4 virtual texture covers 10 by 10 km at 10 texels/cm) | one grid, 2048 cells per side | far short |
| Erosion across tiles is a known seam problem: erosion is non-local, so tiles need a halo larger than its reach, or a coarse global pass plus local refinement | erosion on one slice | missing |
| Terrain drawn with clipmaps or a quadtree LOD from height textures, no cracks | `to_mesh(stride)` | far short |
| Terrain surface is layered materials blended in the shader from mask layers (runtime virtual texture or splat layers, triplanar on cliffs, macro variation) | a mask crossing into `solid.texture` | missing |
| Placement is data-driven, deterministic and **locally stable** (move a mountain, the rest of the world does not redress), run per tile at runtime (Horizon Zero Dawn, GPU density-based) | CPU Poisson in one call | partly (cell-hashed Poisson is right, tiles and runtime are missing) |
| Vegetation is mostly rendering: GPU culling to a drawn subset (Ghost of Yotei culls over a million items to about sixty thousand), LOD chain, impostors, GPU-generated grass blades, hierarchical wind | skeleton only; 16,384 instance cap, one entity per copy | the biggest gap |
| Tree quality: leaf cards on atlases, bark UVs, trunk-to-branch blending, LODs, billboard impostors, baked wind data per vertex | skeleton only | missing |
| Rivers and roads are splines that reshape terrain, water meshes carry flow maps and shore foam | masks only | missing |
| Biomes come from climate rules (height, slope, moisture, temperature) plus art-directed overrides | none | missing |
| Hand control over generated results (stamps, painted masks, splines) | none | missing |
| Collision and navigation from the same terrain | none planned | missing |
| Budgets for the target hardware | none | missing (VR-first: a Quest-class budget is the real bar) |

## 3. Quality bar and budgets (each phase is judged by these)
- **Reference scenes** (kept as browser gallery scenes with eye review and recorded frame cost): (R1) 4 by 4 km mountain valley with river, road and forest; (R2) dense forest floor close view; (R3) coastline with beach and cliffs; (R4) meadow with grass to the horizon.
- **VR budget (stereo, 72 fps target):** terrain at most 300k triangles per eye after LOD; at most 150k vegetation triangles drawn per eye; at most 2,500 draw calls; grass up to 250k blades visible through GPU generation; memory under 1.5 GB total in the page.
- **Visual bar:** no visible terrain cracks or LOD popping beyond a documented distance; no tiling repetition visible at 100 m; seams between tiles invisible; trees readable from 5 m (cards and bark) to 2 km (impostors); wind shows trunk sway, branch sway and leaf flutter as separate motions.
- **Determinism:** the same world seed gives the same terrain and placements natively and in WASM, and the same result no matter which tile is generated first.
- **LLM bar:** a model with only the guide builds R1 from a one-line request in at most 25 node lines; refusals name the fix.

## 4. Architecture (revised decisions; overturn through a new ADR)
1. **World = tiles.** A `World` has a global seed and a tile size (default 256 m). Every generator is a pure function of world coordinates and the seed, so any tile can be built alone and neighbours match. ADR 0056 defines this plus attribute channels and the `HeightGrid` tile value.
2. **Erosion in two scales.** (a) A coarse global stream-power pass (for example 1 m cells scaled to 16 m) fixes valleys, drainage and rivers for the whole world, cheap and seam-free because it is global and low resolution. (b) Per-tile refinement adds fine detail with thermal and hydraulic passes on the tile **plus a halo** at least as wide as the erosion reach, and cuts the halo off; the coarse result constrains the tile so detail cannot move water between tiles.
3. **Attributes everywhere.** Point sets, curves and grids carry named channels; a `capture` node freezes a computed channel; masks are channels.
4. **Layered terrain material in the shader.** Mask layers (rock, grass, soil, snow, sand, wet) are stored as an 8-bit splat atlas per tile; the terrain shader blends material layers by height-weighted masks, uses triplanar mapping on steep faces, adds macro colour variation and a detail normal. No unique baked megatexture.
5. **Terrain rendering is geometry clipmaps** driven by height textures (the vertex-texture approach), with skirts or morphing for crack-free LOD. WebGL2 fallback keeps the existing mesh path with a lower cap.
6. **Vegetation rendering is a GPU-driven path.** Instance data lives in GPU buffers per tile; a compute pass culls (frustum, distance, terrain horizon test) and writes indirect draws; LOD chain per species and impostors beyond a distance; grass blades generated on the GPU from a density mask. WebGL2 fallback culls by chunk on the CPU with a lower cap. This replaces the one-entity-per-placement limit of ADR 0054.
7. **Placement is cell-hashed and runtime, per tile.** Poisson-disk by cell, density and radius from the mask channels, species chosen by biome rules. Results depend only on world coordinates, so they are locally stable. Persist nothing but the rules and the seed.
8. **Vegetation meshing is a node stage.** Skeleton to geometry with UVs, leaf cards on an atlas the texture system makes, LOD chain, impostor billboards (capture N views), per-vertex baked wind data (branch pivot, phase, edge stiffness) feeding the extended wind function.
9. **Rivers, roads, shores are splines** (`curve.*`) that carve and flatten terrain, write masks, and feed water meshes with flow maps. Water remains `water.surface` extended, not rebuilt.
10. **Biomes from climate rules.** Channels for height, slope, moisture (flow and distance to water), temperature (height and latitude) feed a rule table to species densities. Overrides by painted or primitive region masks.
11. **CPU deterministic first, GPU later with CPU reference.** Authoritative results come from CPU code; any GPU kernel (erosion, culling) ships with a CPU reference and a parity test.
12. **Collision and navigation read the same grid.** Heightfield collider for `ve-physics`, navmesh from `ve-nav`, tree trunks as capsules. Tracked in phase G, designed now so the grid format allows it.

## 5. How Claude and Codex work hand in hand
Codex is strong at self-contained algorithm crates with fixed tests; this session owns architecture, integration, rendering and judging. The rule is: **no hand-off without a contract on both sides, and no merge without the other side reading it.**
1. **Contract first.** Claude writes the ADR, the Rust type and function signatures (as stubs that compile) and the failing tests. Codex fills the bodies. Claude never hands over prose-only specs.
2. **Parallel by module, one crate.** (Owner decision 2026-10-21: too many crates, so every algorithm family is a module of the single crate `ve-world`; where this plan says `ve-terrain`, `ve-erosion`, `ve-scatter`, `ve-flora`, `ve-lod`, `ve-river` or `ve-rock`, read the module of that name in `ve-world`.) Codex works in the modules `terrain`, `erosion`, `scatter`, `flora`, `lod`, `river`, one module per job so agents never share a file; Claude works in `ve-modeling`/`ve-graph` (nodes, `Value`, estimates, guide), `ve-render`/`ve-bevy`/shaders (clipmaps, splat material, GPU culling, wind) and the demos. Separate `CARGO_TARGET_DIR`, no shared files.
3. **Claude does hard parts too.** Claude writes the geometry clipmap renderer, the terrain material shader, the GPU culling pass, impostor capture and the wiring; Codex does not own anything that touches the renderer. Codex-side algorithms that need design judgement (erosion tuning, tree look) are paired: Claude reviews outputs visually in the reference scenes and sends parameter and test corrections back.
4. **Short loops.** Every Codex job is at most one crate and one day of work. When it lands, Claude within the same session: reads the diff, runs the tests, wires the node, renders the reference scene, and either accepts or writes a follow-up handoff with the specific defects. Codex Status sections are checked against what actually runs.
5. **Visual review is shared.** After each phase Claude renders the reference scenes and posts screenshots; defects become test cases (an invariant when it can be, a golden image when not).
6. **Codex also reviews.** After a Claude-owned component lands, a Codex job reviews it against its tests and the rules (clippy, file length, determinism, wasm). Findings are fixed by Claude.
7. **No stale handoffs.** A handoff file is updated when its premise changes; the `field.warp` file is currently stale and must be redone before any relaunch.

## 6. Phases
Phase tags: **C** = Claude (this session), **X** = Codex. Each phase ends with its reference scene and budget numbers reported in chat.

### Phase A: contracts (C)
ADR 0056 (world tiles, channels, `HeightGrid`, determinism, halos), ADR 0057 (terrain rendering and material), ADR 0058 (GPU-driven vegetation, replaces 0054's cap). Stub crates with signatures. Failing tests for W1 to W4. Budget and reference scene definitions.

### Phase B: terrain core
- X: **W1** `ve-terrain` (tile-aware grid, world-space noise, masks, carve, mesh), **W2** `ve-erosion` (coarse global stream power, tiled refinement with halo, thermal, hydraulic, flow and river masks).
- C: `terrain.*` nodes, tile `Value`, estimates, cancellation, guide recipe; geometry clipmap renderer and splat-layer terrain material (ADR 0057); terrain collider hook.
- Exit: R1 terrain (no vegetation yet) renders with clipmap LOD, no cracks, layered materials, within the terrain budget; erosion passes the tile-invariance test (a tile built alone equals the same tile built within a larger region, up to the documented halo tolerance).

### Phase C: rivers, roads, biomes
- X: **W6** `ve-river` (spline carve, bank profiles, flow direction maps, shore masks, road flatten with grade limit), biome rule evaluator (pure function: channels to species weights).
- C: nodes and water-mesh flow map integration with `water.surface`; biome/region nodes; stamps and painted-region masks.
- Exit: R1 has a river with banks, a road, shore sand, biome-driven ground materials.

### Phase D: placement
- X: **W3** `ve-scatter` (cell-hashed Poisson per tile, density and radius from masks, variation, relax, cluster).
- C: `scatter.*` nodes, `PointSet` value, tile-streamed placement, a CPU chunk culling path.
- Exit: placement is locally stable (moving a mountain changes only placements whose masks changed); 1M candidate points per tile area generate under budget.

### Phase E: vegetation
- X: **W4** `ve-flora` (skeletons: parametric, colonization, grass, presets), **W7** `ve-lod` (branch meshing with UVs, leaf cards, LOD chain, impostor view set, baked wind data).
- C: GPU-driven instancing and culling, hierarchical wind (trunk, branch, leaf), grass blade generation, impostor rendering, leaf/bark materials through the texture programs.
- Exit: R2 and R4 within the vegetation budget; wind shows three motion scales; transitions between LODs and impostors are not visible at the documented distance.

### Phase F0: water (ADR 0059, owner asked for the best, most optimised and realistic system)
Survey result: our Bevy has no water; Bevy's SSR, transmission and compute passes do not run on the WebGL2 (WebXR) build; Blender's `intern/ocean` is not vendored (use its parameters and spectra as reference, write our own FFT). Design: one `WaterExtension` material, three surfaces (river strips with baked flow and depth, lakes, ocean), depth from data not from the scene (no screen copy), flow-map two-phase shading for rivers, Gerstner waves on low tiers, CPU FFT cascades on the high tier, a local ripple window for interaction, underwater fog and caustics, a `WaterField` query for buoyancy.
- X (after tests): module `ocean` of `ve-world` (spectra after Blender, FFT, cascade synthesis, Jacobian foam), module `ripple` (local wave-equation window); the river module (in flight) supplies the flow and depth data.
- C: `river.carve` node and water strip meshes, `water.lake`, `water.ocean`, `water.material`; the water shader (WGSL/WESL), vertex-channel transport, tiers, underwater fog, terrain caustics term; measurements.
- Exit: reference scenes R5 to R7 reviewed by eye, per-tier shader cost and CPU cascade cost measured, native-versus-WASM hashes.

### Phase F: weather and atmosphere
Bevy fog mapping for the existing haze parameters; wind and rain density as scalar fields; terrain wetness and snow cover from them; particle rain and snow; terrain shadows and ambient occlusion for forests. Mostly C, with X for the field evaluators.

### Phase G: closing the loop
Collision and navmesh from the grid; terrain editing (stamps, painted masks, splines through language commands); the G9c eval comparing typed nodes against generated code on the reference scenes; performance pass against the VR budget on the headset.

## 6b. Modelling and detail prerequisites (added after the owner asked whether modelling needs improving first)
Prerequisites, done in Phase A/B because the world work depends on them:
1. **Vertex attributes and colours** on `Value`/meshes: part of ADR 0056 (channels), needed for baked wind data, terrain masks and per-vertex tint.
2. **Triplanar and world-space texturing with layered detail normals** in the material path (used by the terrain material, rocks and organic shapes; removes UV stretch). Claude, ADR 0057.
3. **Tile-local / camera-relative coordinates**: meshes are f32; at kilometre scale vertices jitter. The renderer and evaluator agree on a world origin per tile. ADR 0056 and 0057.
In parallel with the world work (Phases B to E):
4. **`poly.displace`**: real displacement of polygon meshes by a texture, field or mask (silhouettes, cliffs, bark), with subdivision as the mechanism; ADR 0034 lists deformers on polys as not built. Claude.
5. **Rock and cliff generator** in the language: field-based boulders and cliffs (noise, fracture planes, smooth cuts, overhangs and caves that a heightfield cannot make), meshed per tile. Codex crate `ve-rock` once its tests are written; node wiring by Claude.
6. **Live curvature, cavity and height attributes** on meshes (today only bake-time texture signals, ADR 0043) for edge wear, crevice moss and snow on top faces. Claude.
Later: decals and stamps, trim sheets and panel-line generators, fur and feather cards, tile-world triangle budgets (the 200k/2M stored limits were set for single objects).

## 7. Parallel track: modelling and unfinished work
`field.warp` (taper, twist, bend) is redone from a clean handoff after the Phase A contracts; Blender modelling operators (skin modifier, metaballs, decimation) follow when a world-building need appears.

## 8. Risks
- Renderer work (clipmaps, GPU culling, impostors) is the largest and is Claude-owned; WebGPU compute is required for the GPU path, so the WebGL2 fallback must be a documented lower tier.
- WebGPU limits (storage buffer sizes, workgroup sizes) and WASM memory shape tile size and channel formats; measure before fixing numbers (open task in Phase A).
- Erosion halos and two-scale erosion are plausible but unproven in this codebase; the tile-invariance test is the safety net.
- Art quality needs eye review at every step; budget time for iteration, not just first builds.
- Evidence behind tree-method and erosion choices is thin; keep methods behind parameters.

## 9. Handoffs (Codex)
| Job | Crate | File | Phase |
| --- | --- | --- | --- |
| W1 | `ve-terrain` | `docs/handoff-codex-terrain-grid.md` (tile-aware, v2) | B |
| W2 | `ve-erosion` | `docs/handoff-codex-erosion.md` (two-scale, halo, v2) | B |
| W6 | `ve-river` | `docs/handoff-codex-river.md` | C |
| W3 | `ve-scatter` | `docs/handoff-codex-scatter.md` (locally stable, species, v2) | D |
| W4 | `ve-flora` | `docs/handoff-codex-flora.md` (wind phase and stiffness added) | E |
| W7 | `ve-lod` | `docs/handoff-codex-lod.md` | E |
| W5 | `field.warp` | `docs/handoff-codex-field-warp.md` (stale, redo) | parallel |

## 10. Status (2026-10-21, end of day)
Done and verified (`cargo test -p ve-world`: 69 tests pass; `cargo clippy -p ve-world --all-targets -- -D warnings` clean; wasm check passes):
- ADR 0056 (tiles, channels, grids) and ADR 0057 (terrain rendering and material) written.
- `ve-world` crate with modules `terrain` (grid, gradient-noise generators, five masks, carve, stamp, warp, mesh and height-texture output), `erosion` (priority-flood flow, implicit stream power, thermal, Mei-style hydraulic, two-scale tile refinement with halo; 512 by 512 tile in 1.9 s release) and `scatter` (locally stable cell-hashed Poisson, species rules, vary/align/filter/relax/cluster; 206k points in 0.28 s release). Codex built the bodies; the owner-side session wrote contracts, tests, and reviewed (and corrected two wrong test premises of its own).
- Graph nodes `terrain.tile`, `terrain.mask`, `terrain.erode` wired through `ve-graph`/`ve-modeling` (9 tests in `g12_world.rs`, including eroded neighbouring tiles meeting at the shared edge).
- Rendered: a 3 by 3 block of tiles reads as one continuous landscape without cracks.
Known defects and findings:
- Shading seam at tile borders: vertex normals are computed per tile; needs per-vertex normals from the haloed grid (part of the vertex-channel work of ADR 0056).
- Baked texture programs that blend two 3D textures by a surface signal came out blank on terrain tiles (single programs work); this confirms the plan: terrain gets a shader-blended material (ADR 0057), not per-tile baked textures.
- Erosion at current settings is subtle; needs tuning by eye (river_erosion strength, coarse world size).
- `terrain.mask` layers are not yet visible anywhere (no consumer): the next consumers are the terrain material and `scatter.*` nodes.
- `cargo xtask rules` fails on `ve-graph/src/builtin/fields.rs` (409 lines): the unfinished Codex `field.warp` work.
Next: scatter nodes (`scatter.points`, `scatter.instance`) consuming masks; the terrain material shader (ADR 0057) and per-vertex normals; river/road module (handoff-codex-river.md, tests first); flora and lod modules; redo `field.warp`.
