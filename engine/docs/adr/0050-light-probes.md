# ADR 0050: Global illumination with traced light probes

- Status: accepted
- Date: 2026-10-19

## Context

Ambient light was a constant two-colour sky/ground hemisphere. A closed room was as bright as an open field, and a red wall never tinted its neighbour. The browser backend (WebGL2 through wgpu) has no compute shaders, no storage
buffers and no readable depth, which rules out screen-space and GPU path-traced methods that need them. Everything must stay procedural and in the browser (no servers), and a bake must give the same answer for the same scene.

## Decision

1. **Probes are traced on the CPU in WASM** by the `ve-gi` crate: a grid of probes, each storing first-order spherical-harmonic irradiance over pi, from Fibonacci-sphere rays against a BVH of the scene's triangles, with the
   sun, a gradient sky and emissive surfaces as light sources and 1 to 4 bounces (each pass reads the previous grid). Probes inside solids are detected (back-face hits) and filled from neighbours. Output is deterministic.
2. **Opt-in on the environment node.** `scene.environment` and `scene.sky` take `gi`, `gi_spacing`, `gi_bounces` and `gi_rays` (carried as `SceneEnvironment.gi`, merged when a sky and an environment are both authored).
   Without `gi` nothing changes, so existing scenes and golden images are untouched.
3. **The runtime collects the committed geometry** (instanced sets expanded by their placements, thinned evenly past 500 000 triangles, clear glass skipped), sizes the grid from the scene (about 24 probes along the longest side unless `gi_spacing` is
   given, at most 64 per axis) and bakes about 4 ms per frame, so the editor never stalls. The scene keeps the constant ambient until the bake finishes, then the result is published as the `ProbeVolume` resource.
4. **The renderer uploads the volume as four 3D `Rgba16Float` textures** (constant, x, y and z coefficients, rgb each) in the lights bind group (bindings 3 to 7), with zero 1 by 1 by 1 stand-ins when there is no volume. The forward shader
   looks a surface up with one hardware trilinear sample per texture (nudged off the surface along its normal, clamped to the grid) and uses it in place of the sky ambient; the ground-bounce term is switched off because the probes
   already contain it. f32 to f16 conversion is ours and tested (round to nearest even, denormals, overflow).

## Consequences

- A browser test (`web/tests/g11-gi.mjs`): the far wall of a closed room with one window measures 71 with the constant ambient and 36 with probes.
- Static geometry only: moving parts are baked where they were at commit. Light moving or animated materials do not update the probes until the next commit.
- Probe light leaks through thin walls thinner than the probe spacing; the normal nudge and the inside-solid fill reduce it but a finer `gi_spacing` is the fix.
- The bake is single-threaded. Large scenes take seconds to minutes at the default settings; progress is reported as `gi_progress` in the projection status.
- Not yet done: specular occlusion from the probes, spatially varying sky visibility beyond L1, and a per-frame probe update for animated scenes.
