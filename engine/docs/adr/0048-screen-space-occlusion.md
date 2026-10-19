# ADR 0048: Ambient occlusion on the WebGL2 backend: a depth colour texture, a GTAO pass and a denoising average

- Status: accepted
- Date: 2026-10-19

## Context

The renderer had a screen-space ambient occlusion option (`ssao_enabled`) that sampled the resolved depth texture in the present pass with `textureLoad`, from eight neighbours, and darkened where depth jumped. On the browser's WebGL2
backend (wgpu's GL path) it never worked: naga has no GLSL form for `textureLoad` from a depth texture (`WGSL textureLoad from depth textures is not supported in GLSL`), and a plain sample of a depth texture comes out as a
`sampler2DShadow` that the GLSL compiler rejects for a two-component coordinate. The pipeline failed validation, which stops the whole frame, so the option only ever worked on native or WebGPU targets. Shader tests missed it
because they only validated the WGSL, not its translation. The hack was also a depth-discontinuity filter, not occlusion, and the old design forced MSAA off whenever it or bloom was on.

## Decision

1. **Depth goes through a float colour texture** (`ao_depth.rs`, `shaders/ao_depth.wgsl`). While AO or bloom is on, the opaque batches are drawn once more, single-sampled, into an `R32Float` target with its own depth buffer; the
   fragment writes its reverse-Z device depth, cleared to 0 for the sky. The pass costs one extra draw of the opaque batches (counted as `depth` in the frame counts). Reading it is an ordinary `textureLoad` on a colour texture.
2. **Ground-truth ambient occlusion** (`ao.rs`, `shaders/ao.wgsl`; Jimenez et al. 2016, structured after Intel's XeGTAO, MIT). A full-screen pass writes an `R8Unorm` texture: per pixel three slices, six steps each side, a 1.2 m
   world radius that fades over its last 40%, world position and normal rebuilt from the depth, slice angles and step offsets from a 4 by 4 repeating pattern.
3. **The present pass averages and applies it** (`shaders/present.wgsl`): the 4 by 4 neighbourhood, weighted down where the depth differs from the centre's (relative to its distance), which cancels the pattern exactly without
   blurring across silhouettes; the colour is then multiplied by `mix(1, visibility, 0.9)`.
4. **Post effects no longer force MSAA off.** That rule existed only because the old pass bound the forward depth, which cannot be multisampled and sampled together.
5. **Shader tests translate to GLSL ES 3.00** (`shader_tests.rs`, naga `glsl-out`) as the browser does, and a negative test documents the depth-texture limit, so this class of failure is a `cargo test` failure.

## Consequences

- AO works in the browser (checked in Chromium on a software GL device: no console errors, contact darkening at object bases and wall corners, no visible grain), including with 4x MSAA. The existing `g10-render-aa.mjs` passes.
- The occlusion multiplies the final lit colour, including direct light, because there is no separate ambient buffer: sunlit crevices darken a little more than a physically separate ambient term would. Applying it to the ambient
  term only would need the forward pass to output it, a larger change.
- Cost on a real GPU is not measured (the test device is a software renderer): three slices by six steps by two sides is 36 depth reads per pixel at full resolution. A half-resolution AO texture and a per-tier slice count are the
  next levers if it is too slow on a headset. It is off by default (`ssao=on`, or the viewer checkbox); making it a High-tier default changes the browser golden images and needs the owner's go-ahead.
- Stereo: the pass runs per view slot with that slot's viewport, like the forward pass; not yet checked on a headset.
