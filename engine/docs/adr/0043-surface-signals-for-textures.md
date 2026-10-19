# ADR 0043: Surface signals for procedural textures
- Status: proposed
- Date: 2026-10-18

## Context

Texture programs need masks that depend on the generated surface, such as curvature, cavities, orientation, occlusion, and height. These signals depend on the individual mesh and its UV mapping, unlike a purely procedural tiled texture.

## Decision

`src/engine/texture/surface-nodes.ts` adds six mesh-reading nodes: `surface_convex`, `surface_cavity`, `surface_curvature`, `surface_ao`, `surface_up`, and `surface_height`. `signals.ts` supplies corner normals and occlusion. The `SURFACE` shader in `bake.ts` combines per-texel position, smooth normal, and occlusion maps into signed curvature, AO, and vertical normal; the position map supplies height.

Curvature is estimated from normal changes over the UV texel neighbourhood using the local surface metric. It is scaled by 0.05 and clamped to [-1, 1]. Thus a sphere with radius 5 cm reads approximately 1 before remapping/clamping; the value is mean curvature calibrated to `1/r`, not a dimensionless curvature. The browser probe describes the expectation as `gain * 0.05 / r` for `surface_convex`.

In `src/engine/mesh-upload.ts`, a program whose source includes any name in `SURFACE_NODE_NAMES` is treated as mesh-dependent and is never shared through the tiled-program key. The same programs are not eligible for the shared tiled bake path. This avoids reusing a signal map that belongs to another mesh or UV chart.

## Consequences

- Texture authors can build edge wear, cavity grime, top-facing masks, AO masks, and height transitions from generated geometry.
- The signal bake costs additional map rendering and storage per mesh-dependent program.
- Curvature depends on neighbouring texels belonging to the same surface; `bake.ts` sets it to zero where chart boundaries, gaps, or invalid position samples prevent a reliable neighbourhood.
- Signals are texture-bake inputs, not live render-time geometry attributes.

## Alternatives considered

- Computing signals only from the procedural texture would not provide mesh-dependent curvature, occlusion, or height.
- Sharing/tile-baking surface-reading programs would reuse maps that depend on another mesh, so this path explicitly disables that sharing.

## Evidence

`web/tests/g10-surface-signals.mjs` checks convex curvature on 25 cm and 1 m spheres, zero curvature on a plane, concavity on an inward sphere, and upward slope on a plane. `surface-nodes.ts`, `signals.ts`, and the `SURFACE` shader in `bake.ts` were inspected; the browser test was not run for this documentation task.
