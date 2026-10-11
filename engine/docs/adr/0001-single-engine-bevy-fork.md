# 0001: One engine, a trimmed Bevy fork
- Status: accepted
- Date: 2026-10-11

## Context
We need one runtime for editing and playing interactive 3D in the browser and in WebXR, with Quest 3
as the reference device. Bevy 0.20.0 is MIT/Apache-2.0 and its `bevy_render` carves out cleanly:
it no longer depends on `core_pipeline`, `pbr`, `ui` or `winit` (spike: 211 crates, 1.82 MB brotli).
Rendering is one ECS schedule per camera, driven by a camera driver, which suits per-eye stereo.

## Decision
Vendor a pinned Bevy fork and keep the ECS, reflection, assets, mesh, camera and render crates.
Replace `core_pipeline`/`pbr` with a lean forward pipeline, `winit` with our own web/XR runner,
and `bevy_ui` with world-space panels. Core crates stay independent of Bevy types.

## Consequences
We own an XR layer and a stereo pipeline (largest unproven risk: Quest performance). Fallback:
a thin wgpu renderer keeping `bevy_ecs`.
