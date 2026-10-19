# Handoff to Codex: baked maps in the browser (read them from the evaluator, show the normal map in the viewer, export both in glTF)

Read `engine/AGENTS.md` first and follow it (no commits, no `Co-Authored-By` or tool attribution; TypeScript follows the style of the surrounding files; no network). Run TypeScript checks from the **repo root**
(`/Users/mac/Work/victoria-vr-hack`): `bunx tsc --noEmit`, `bun test tests/*.test.ts`, `node --check engine/tools/mcp/server.mjs`. Do not build the browser bundle. Write the Status into **this file** at the end.

## Shared tree: hard rules (the manager works in the same working tree right now)
- **NEVER run `git checkout`, `git clean`, `git stash`, `git restore`, `git reset`, or write a file from `git show` over a working file.** If a file you did not change fails, say so in the Status; do not revert it.
- **Commit nothing and create no branches** (the manager commits, directly on `main`).
- **Files you own this round**: everything under `src/engine/**` and `tests/**` (new tests go in `tests/baked-maps.test.ts`), `engine/web/**` only if the viewer staging needs it.
- **Others are editing**: everything under `engine/crates/**` (the manager is adding the Rust side described below), `engine/cpp/**`, `engine/tools/mcp/**`. Do not touch them.

## Context
The manager is adding a graph node `bake.maps@1(low, high)` that bakes a high-poly part's tangent-space normal map and ambient occlusion onto the low-poly part's UVs **inside the evaluator** (the `ve-measure` baker), so
the result belongs to the part like its mesh. You are doing the browser half. The Rust half will expose three functions on the **evaluator** wasm module, next to `eval_uvs(part)` and `eval_occlusion(part)`
(`engine/crates/ve-evaluator/src/lib.rs`), which you can rely on and fake in tests:
- `eval_baked_size(part: number): number`: the side of the baked images in pixels, or 0 when the part has none;
- `eval_baked_normal(part: number): Uint8Array`: `size * size * 4` RGBA bytes (tangent space, OpenGL convention, row 0 at v = 0), or empty;
- `eval_baked_ao(part: number): Uint8Array`: `size * size * 4` RGBA bytes of grey ambient occlusion (255 = open), or empty.
Find how `eval_uvs` and `eval_occlusion` reach a `MeshPart` (`src/engine/worker-types.ts`, the evaluation worker, `evaluation.ts`: trace them) and add the baked data the same way.

## What to build
1. **Plumbing.** `MeshPart` gets `baked?: { size: number; normal?: Uint8Array; ao?: Uint8Array }`, filled from the three exports; absent when `eval_baked_size` is 0. Transfer the buffers the way the other typed arrays are transferred
   (no copies if the worker already uses transfer lists).
2. **Viewer.** Where procedural material maps are staged (`src/engine/mesh-upload.ts`, the call to `runtime.eval_projection_maps(token, node, region, albedo, normal, metallicRoughness, emissive)`; each map must be
   512 x 512 RGBA8), stage the baked **normal** map as the normal map of the part's region 0 when the part has no procedural normal map. A baked map of another size is resampled to 512 x 512 with bilinear filtering in
   TypeScript (a new small function with its own tests); 512 passes through unchanged. The baked AO is not shown in the viewer (the renderer has no AO texture slot; it keeps its per-vertex occlusion).
3. **glTF export** (`src/engine/gltf.ts`, `gltf-mesh.ts`). A part with `baked.normal` gets a PNG (use `rgbaPng` from `src/engine/png.ts`, it is async and compressed) as the material's `normalTexture`, and the mesh gets
   MikkTSpace `TANGENT` (the exporter already does this when a normal texture exists: reuse that path). `baked.ao` becomes the material's `occlusionTexture` (glTF reads the red channel). A baked normal map **replaces** a
   procedural one for the same part; a baked AO replaces a procedural AO. Parts without `baked` export exactly as before.

## Tests (fixed; do not weaken any assertion; if one fails, say which and why). Create `tests/baked-maps.test.ts`; model the fixtures on `tests/gltf.test.ts` and `tests/bake-tool.test.ts` (a fake `runtime`)
1. With a fake evaluator whose `eval_baked_size(0)` is 4 and `eval_baked_normal(0)` is 64 bytes, the part built by the plumbing has `baked.size === 4` and `baked.normal.length === 64`; with size 0 the part has no `baked`.
2. Bilinear resampling: a 2 x 2 map of four distinct colours resampled to 512 x 512 has the four corner pixels equal to the four colours and the centre pixel equal to their average (within 1); a constant map stays constant; a
   512 map is returned as the same array (identity).
3. Viewer staging: with a recording fake runtime and a part whose baked normal is 256 x 256, `eval_projection_maps` is called once with a normal array of exactly `512 * 512 * 4` bytes and an empty `albedo`; with a 512 map it
   receives the same bytes; a part with a procedural normal map and a baked one stages the **procedural** one (viewer rule); a part with only baked AO calls it with an empty normal.
4. glTF with a baked normal: the material has `normalTexture`, there is one image whose PNG header width and height equal the baked size, the primitive has a `TANGENT` accessor, and `gltf-validator` reports 0 errors.
5. glTF with baked AO: `occlusionTexture` present and valid (0 errors); with both maps there are two images.
6. In glTF a baked normal replaces a procedural normal for the same part (give the part both through the existing `textureImages` option: the exported normal image is the baked one, compare the PNG dimensions you chose to be different).
7. A part without `baked` exports byte-identically to before (build the same part twice, with and without an empty `baked: undefined` field).

## Acceptance
`bunx tsc --noEmit` clean, all `tests/*.test.ts` pass (report the counts), every file you write stays under 400 lines.

## Status (Codex: fill in with real numbers for each of the seven tests, then stop)
- State: implemented; verification complete.
- Per test (all pass):
  1. Evaluator map plumbing helper: size `4`, normal bytes `64`; size `0` returned `undefined`.
  2. Bilinear resampling: output `512 × 512 × 4 = 1,048,576` bytes; corner pixels matched; centre `[50, 50, 0, 255]`; constant preserved; 512 input identity preserved.
  3. Viewer staging: 256 input staged as `1,048,576` normal bytes; 512 input retained by identity; AO-only input staged empty normal (`0` bytes).
  4. Baked normal glTF: PNG `4 × 4`; `TANGENT` accessor present; validator errors `0`.
  5. Baked AO glTF: occlusion texture present; validator errors `0`; normal + AO image count `2`.
  6. Procedural normal replacement: exported image width `4` (baked), replacing the procedural PNG width `1`.
  7. No baked map vs `baked: undefined`: byte-for-byte equal GLB output.
- Failing tests anywhere: none. `bunx tsc --noEmit` clean; `bun test tests/*.test.ts`: `116 pass`, `0 fail`, `882 expect()` calls across `20` files; `node --check engine/tools/mcp/server.mjs` clean. The strict gate is pending.
- What you tried when something blocked you: The first baked-only export test tried to load the absent local `src/engine/texture/texture-library/index.json`. Updated the export flow to initialize the procedural texture library only when a part has a procedural texture program; baked-only export now passes without network access. Did not build the browser bundle.
