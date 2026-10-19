# ADR 0042: Texture layers, tiling and sharing
- Status: proposed
- Date: 2026-10-18

## Context

The viewport needs baked maps on many surfaces while retaining one shared texture binding for draws. Texture memory and the array-layer count are bounded, and ordinary per-mesh UV charts are not available for every generated mesh.

## Decision

`ve-render/src/textures.rs` stores 1024-by-1024 RGBA8 layers with an 11-level mip chain and caps the array at 32 maps. Each complete textured surface may consume albedo, normal, metallic-roughness, and emissive layers. `allocated_layers` allocates at least two layers and adds one spare when the requested count is a multiple of six: the source documents that wgpu's GL backend infers an incompatible view dimension for a one-layer texture or a multiple-of-six layer count. The maps share one 2D-array binding and sampler.

`solid.texture@1` carries the program and optional `tile` through modeling material data. In `src/engine/texture/uv.ts`, `worldTileUvs` projects triangles along their dominant normal and divides world coordinates by `tile`, making the repeating map scale correspond to metres. The viewport sampler repeats. When the author has not requested world tiling, the texture pipeline can use the provided UVs or a box-chart fallback.

Viewport baking shares maps by reference within a scene and in a bounded cache in `src/engine/mesh-upload.ts`. Explicitly tiled, non-surface-reading programs without authored UVs use a key based on program, style, tile size, and map size, so equal inputs reuse baked pixels and GPU layers while each part gets its own world-space UVs. `src/engine/texture/budget.ts::fitToLayerBudget` counts distinct map references against the 32-layer viewport budget and drops maps in the documented priority order, starting with emissive. Rust's `MaterialTextures::add` appends layers and returns their indices; it does not deduplicate pixel contents.

## Consequences

- A scene's viewport maps share one binding, with a hard 32-layer budget and a per-map 1024-square footprint.
- Explicit metre tiling makes the same procedural texture scale consistently across differently sized parts.
- The viewport may drop less-prioritized maps when over budget; `mesh-upload.ts` states that this does not affect exported GLB maps.
- Layer sharing is driven by equal baking keys and shared references, not by comparing/deduplicating Rust pixel buffers.

## Alternatives considered

- Allocating exactly the used layer count is incompatible with the observed GL backend layer-count quirk.
- Baking a separate tiled map for every part would use additional layers despite identical program/tile/size inputs.
- A seam-aware UV unwrap is preferred when authored, but `uv.ts` describes box-chart projection as a bounded fallback rather than production distortion control.

## Evidence

`crates/ve-render/src/textures.rs` tests layer limits, allocation counts, and mip downsampling. The UV and budget algorithms are in `src/engine/texture/uv.ts` and `src/engine/texture/budget.ts`; viewport keys and map reuse are in `src/engine/mesh-upload.ts`. Tests were inspected, not run for this documentation task.
