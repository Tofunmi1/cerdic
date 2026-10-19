# ADR 0052: Texture memory as a byte budget over three map sizes

- Status: accepted
- Date: 2026-10-19
- Amends: ADR 0042 (texture layers, tiling and sharing)

## Context

ADR 0042 kept every baked map in one array of 1024 by 1024 layers, at most 32 of them. A leaf, a pebble or a distant prop paid the same 5.3 MiB as a hero wall, and a scene with more than 32 distinct maps lost channels silently
(the later surfaces lost glow, then shininess, then bumps). A forest with several species, bark, leaves, grass, ground and rocks needs well over 32 maps, but almost none of them need 1024 pixels. The forward shader also read all
four map slots for every pixel, textured or not.

## Decision

1. **Three arrays, one budget.** Maps are 1024, 512 or 256 pixels square, each size in its own `texture_2d_array` (`textures.rs`, `textures_gpu.rs`). The scene's limit is bytes on the GPU, mip chains counted, 192 MiB
   (`BUDGET_BYTES`): 36 maps of 1024, 144 of 512 or 576 of 256, or any mix. The old 32 large maps still fit. Unused sizes bind a 1 by 1 stand-in.
2. **Ids carry the size.** A surface names a map by `tier * 4096 + layer` in the existing four-number `map_layers`, so ids of 1024 maps are the plain layer numbers they were before. The runtime refuses a scene whose distinct maps exceed the budget and
   a map that is not one of the three sizes.
3. **The shader picks the array per instance with explicit derivatives.** The forward pass takes `dpdx`/`dpdy` of the UVs once, in uniform control flow, and every map read uses `textureSampleGrad` on the array its id names, so the choice can differ between instances.
   A surface without a map no longer fetches at all (it used to read layer 0 four times and ignore the result). The cutout shadow and depth shaders read the albedo alpha at the top level from the same arrays.
4. **The author picks the size, the viewer degrades gracefully.** `solid.texture size="auto"|"256"|"512"|"1024"` (auto is 1024) is carried as `Material.texture_size`. When a scene is still over budget the viewer (`texture/budget.ts`) shrinks the largest maps first, halving with a box
   filter, the least useful channel first and the last surface first, and only when everything is already 256 does it drop channels, as before. Shared maps (identical programs) count once and shrink together. A baked normal map from `bake.maps` is counted too.
   Warnings `texture.viewport_shrunk` and `texture.viewport_budget` say which.

## Consequences

- A browser test (`web/tests/g11-texture-budget.mjs`): 40 distinct maps at 256 take 13.3 MiB and each part shows its own colour; 40 at the default 1024 are shrunk to 189.3 MiB with none dropped and the shrinking reported.
- The GL backend counts texture image units: forward now uses 1 (exposure) + 1 (shadow) + 4 (probes) + 3 (maps) = 9 of the 16 WebGL2 guarantees.
- The mip chain is still built on the CPU in the WASM (box filter) and the arrays are rebuilt whenever a map changes; a streaming or GPU mip path is not done.
- Shrinking happens after baking, so an over-budget scene still pays for baking at full size. Choosing sizes before baking is possible later.
- The exported glTF is unaffected: it bakes at its own size.
