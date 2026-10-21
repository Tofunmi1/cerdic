# ADR 0057: Terrain rendering and the layered terrain material

- Status: proposed (design from research 2026-10-21; each step is measured before the next)
- Date: 2026-10-21

## Context

A terrain tile (ADR 0056) is a grid of heights and mask layers. Shipped open worlds draw terrain with nested LOD rings (geometry clipmaps) sampling a height texture in the vertex shader, and shade it by blending material layers chosen by masks, with triplanar mapping on cliffs, macro variation to hide tiling, and no unique baked megatexture (Far Cry 4 adaptive virtual texturing; GPU Gems 2 geometry clipmaps; runtime splat layers). Our renderer is Bevy 0.20 (`ve-bevy`): materials are `ExtendedMaterial<StandardMaterial, Extension>` with extension shaders in WESL (`detail.wesl`, `toon.wesl`), and the vertex stage can be overridden by an extension.

## Decision

1. **Terrain material is a Bevy material extension (`TerrainExtension`)**, not a new pipeline: PBR lighting, shadows, fog and the AO/exposure passes keep working. The extension adds a splat texture, a height texture and a small uniform block of layer parameters.
2. **Layers are procedural, not imported.** A layer is a colour, roughness, a relief scale and strength, and a style (the `detail.wesl` noise styles), authored in the language (`terrain.material layers=...`). Up to 4 layers per splat texture (RGBA8), up to 8 with two splat textures. This keeps texture memory flat (ADR 0052) and obeys ADR 0003. Texture-program layers (`solid.texture`) can be added later as arrays behind the 32-layer budget.
3. **Blending.** Weights come from the splat texture, are sharpened by a per-layer height term (relief height decides who wins at a boundary, so rock pokes through grass), and a slope term forces the cliff layer on steep faces. Cliffs and steep layers sample noise **triplanar** from the world position (no UV stretch); flat layers use planar projection. A low-frequency macro term varies colour and roughness across tens of metres to hide repetition. A detail normal at two scales is added from the same noise.
4. **Mesh path first (works everywhere).** Each tile becomes mesh LODs built on the CPU (`ve_terrain::to_mesh` with strides, skirts to hide cracks) and chosen by distance. This is the WebGL2 fallback and the first milestone.
5. **Clipmap path second (WebGPU, and WebGL2 if the vertex texture measurement allows).** Nested square rings of a fixed vertex grid are moved with the camera and snapped to the ring cell size; the vertex shader samples a 16-bit height texture (`ve_terrain::to_height_texture`) and morphs vertices toward the coarser ring near the ring edge to avoid popping and cracks. Normals come from the height texture in the fragment stage. WebGL2 guarantees vertex texture fetch units; the measurement in the first milestone confirms the real limit on target devices.
6. **Tiles are placed with local coordinates.** Each tile entity's transform carries the tile origin (large offsets in a double-precision or camera-relative placement); vertex data is relative to the tile, so kilometre-scale worlds do not lose precision.
7. **Collision and placement read the same grid** (heightfield collider for `ve-physics`, `HeightGrid::sample` for placement and foot IK).

## Consequences

- Terrain looks right at VR distances without texture streaming; the cost is shader work per pixel (limit layer count, measure on the Quest-class budget).
- `ve-bevy` gains a terrain plugin; `ve-modeling` gains a terrain part kind in the evaluation result (height texture, splat texture, layer table, tile origin) beside meshes, so exports and caches can treat it explicitly.
- glTF export of a terrain tile is its meshed LOD with baked vertex colours from the layers (the shader blend does not export).
- WebGL2 keeps a lower tier (mesh LODs, fewer layers); the WebGPU path adds clipmaps and higher layer counts.

## Alternatives considered

- Unique baked megatexture per world: storage and streaming cost, blur at close range (Rage's 20 GB); rejected.
- Separate render pipeline for terrain: duplicates lighting and shadows; rejected in favour of the material extension.
- Tessellation or mesh shaders: not baseline WebGPU; revisit later.
- Imported rock and grass textures: forbidden by ADR 0003.

## Evidence needed (first milestone)

Measure on the stage scene: frame time for one 1 km tile at 4 LOD levels; vertex texture fetch support on the target browsers and Quest; shader cost of 4 versus 8 layers; crack-free LOD seams (screenshot test). Reference scenes R1 to R3 (plan section 3) are the visual bar.
