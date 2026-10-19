# ADR 0040: opt-in render quality passes

Date: 2026-10-18  
Status: Proposed

## Context

The renderer already uses an ACES filmic curve, per-view exposure and grading, baked vertex
occlusion, environment-derived specular, and an analytic sky reflection. These run in the forward
pass. Screen-space ambient occlusion and bloom need a sampled scene color/depth image and one or
more full-screen passes. The previous present path is only a conditional copy, and the normal output
path renders directly to the window or XR layer. Replacing that path unconditionally would add a
copy and risk changing existing golden pixels when effects are off.

## Decision

Keep the existing forward output as the exact default. Add renderer effects behind independent
viewer controls and only route a view through offscreen targets when an effect that requires them is
enabled. Keep authored scene data and evaluator output independent of these viewer settings.

Geometric specular anti-aliasing is an opt-in forward shading setting. It estimates normal variance
from screen derivatives and increases microfacet roughness by that variance, which suppresses
sub-pixel highlight shimmer. It is packed into a per-view uniform, so stereo eyes use the same
setting. It is off by default and does not modify material data.

The renderer's existing ACES curve is the filmic tone mapper; exposure remains a viewer-only control.
Environment lighting already provides analytic sky-based reflections. These are documented as
existing capabilities rather than duplicated in another pass.

The first screen-space pass combines two independent settings in one conditional post draw. SSAO
uses eight reverse-Z depth comparisons within each eye's viewport; bloom uses a bright-pixel
threshold and a five-tap cross kernel. Both are disabled by default. Enabling either routes the view
through a sampleable offscreen colour target and switches depth to single-sample. This is a cheap
prototype, not a full GTAO implementation or a multi-scale bloom chain. The focused browser probe
now stages a room corner, an open floor, a contact block and an emissive sphere. The quality flags
change in runtime status, but browser captures remain pixel-identical when toggled, so the post pass
is not yet visibly effective and this decision is not ready for acceptance.

## Consequences

- The viewer can opt into geometric specular AA without affecting saved scenes or evaluator parity.
- The option is off by default and survives quality-tier changes.
- ACES, exposure, and analytic environment reflections remain on their established shader path.
- AO and bloom browser acceptance is currently failing: each toggle changes runtime status but the
  captured viewport changes by 0 pixels. The post-pass presentation path must be fixed before these
  effects can be considered implemented.
- Dedicated AO corner/open-floor margins, emissive halo evidence, specular-AA stability, stereo seam
  tests, per-effect timing and physical headset timing remain open work.
- Quest-class GPU cost has not been measured on physical hardware.
- Browser visual evidence is from low tier only. One high-tier SwiftShader probe reported that multisampled texture storage is unsupported; high-tier browser behavior still needs a backend-capability path or validation on a backend with multisample support.
