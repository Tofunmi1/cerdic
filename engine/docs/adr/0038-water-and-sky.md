# ADR 0038: Procedural water surfaces and scene sky

Status: proposed  
Date: 2026-10-17

## Context

The viewer needs ordinary editable water geometry and an authored outdoor background. Reusing `solid.material` alone
would describe transparency, but not provide a coherent water default or a scene-wide atmosphere. Imported skyboxes and
normal maps conflict with the browser-first procedural asset policy. A sky also needs to illuminate geometry and survive
the viewer's stereo/XR rendering path.

## Decision

`water.surface@1` emits a closed, bounded Solid with a regular grid top. Its rest shape uses the existing deterministic
fractal value noise, with a 192-cell-per-side cap. It carries a default blue-green, low-roughness, 0.6-opacity material;
later `solid.material` nodes may replace it. A material recipe selects a small vertex-shader ripple driven by the viewer's
shared animation clock. Only the upper-facing water vertices move. Exported mesh geometry remains at its deterministic
rest shape, and water does not participate in physics.

`scene.sky@1` emits the existing Environment port with an optional serializable sky payload. A lightweight analytic
gradient, sun disc/glow and seeded value-noise cloud layer are rendered as a per-view full-screen pass. Its sun direction
and colour produce an ordinary directional scene light; its hemisphere provides a low-cost ambient approximation. A
document may contain one sky and one `scene.environment`. If both are present, the authored environment replaces ambient
lighting while the sky continues to paint the background. Without a sky, the existing clear-color path remains active.

## Consequences

- Water can flow through existing solid material, selection, local save, and export paths without introducing a separate
  geometry type. Its rest mesh is deterministic across native and WASI evaluation.
- Sky shading is procedural and cheap enough for the normal and stereo viewer paths. The atmosphere is an analytic
  approximation, not a physical Nishita implementation; clouds are a sparse procedural layer rather than volumetric
  weather.
- The viewer ripple is not part of exported geometry. Headset GPU timing and visual quality still need real-device review.

## Evidence

Proposed with the initial implementation. Targeted Rust tests, 31 kernel and 100 modeling native/WASI parity lines,
water/sky browser pixel checks, the scenery dropdown check and an IWER Quest 3 two-eye sky capture pass. Sonnet `water-tools`
passes 3/3; `sky-tools` has one passing prompt, with two interrupted by the provider session usage limit. See
`docs/plan-animation-and-blender.md` for the remaining fidelity work. No physical headset was tested.
