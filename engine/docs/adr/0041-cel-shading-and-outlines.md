# ADR 0041: Cel shading and inverted-hull outlines
- Status: proposed
- Date: 2026-10-18

## Context

Generated materials need a compact cel-shading mode and a controllable silhouette line. A hard-edged mesh has separate face normals at a corner, so simply offsetting its vertices along those normals can leave gaps in an inverted hull. The forward instance vertex layout is also bounded by the GPU's vertex attribute locations.

## Decision

`solid.toon@1` supplies the toon material parameters consumed by `ve-render/src/material.rs` and `shaders/forward.wgsl`: band count, feather, shadow tint, highlight, rim, and outline settings. In the forward fragment shader, positive band count replaces the ordinary physically based response with tinted cel bands; emitted colour remains additive.

Outlined opaque batches receive a second `PassKind::Outline` draw (`pipeline.rs`, `pass.rs`). It culls front faces and uses `vertex_outline` in `shaders/forward.wgsl`, which pushes the hull outward by a screen-space pixel width. Instances without a line are clipped away in that pass. The outline pass is not queued for transparent batches.

During `ve-runtime/src/projection/upload.rs::pack_smooth_normals`, static mesh corner normals at positions quantized to millimetres are averaged and octahedrally packed into the mesh's occlusion float with four-bit occlusion. The shader's `unpack_occlusion` recovers the occlusion value for lighting; `outline_normal` uses the packed smooth normal to close the hull across hard corners. Skinned meshes retain plain occlusion and use their face normal for outlines.

The current vertex layout uses locations 0 through 14 (position, normal, occlusion, instance data, optional UV, and toon data): 15 of the 16 available locations.

## Consequences

- A toon look and its outline are material settings and require no separate mesh geometry.
- Outline width is screen-space and clamped when packed; a zero width disables the line.
- Smooth corner normals share the occlusion attribute, so the packing and shader decoding must remain in sync.
- The single remaining vertex attribute location constrains future per-vertex and per-instance inputs.

## Alternatives considered

- A separate outline mesh would require generating and maintaining additional geometry; the implementation instead draws the existing mesh a second time.
- Keeping only face normals was insufficient at hard corners, where the hull can open.

## Evidence

The shader/material packing is covered by unit tests in `crates/ve-render/src/material.rs`; layout construction and pass selection are in `crates/ve-render/src/pipeline.rs` and `crates/ve-render/src/pass.rs`. `crates/ve-modeling/tests/g11_style.rs` checks generated style parameters. These tests were inspected, not run for this documentation task.
