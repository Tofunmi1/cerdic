# 0014: Instance sets retain a shared prototype until explicit realization

- Date: 2026-10-13
- Status: accepted for implementation; entity representation is subject to the required probe.

Instance sets contain an immutable shared solid prototype and ordered f64 affine placements.
Nested patterns flatten placements by checked composition, preserving deterministic instance indices.
The cap is 16,384 placements per set and per result; products are checked before allocation.
Instances render a prototype once with placements; reported volume sums placed volumes and ignores
overlaps. `inst.realize` alone builds and unions placed solids. Its preflight respects the quality
triangle cap, the additional 4,000,000 realization cap and 256 MiB temporary-storage cap.
Rendered result triangles also count placements (prototype triangles times placement count), so
sharing does not make a multi-billion-triangle draw pass preflight merely because its upload is small.

The first implementation probes one ECS entity per placement with shared mesh handles, using the
existing GPU batching and picking. The 10,000-entity probe drew 120,000 triangles in one draw with
one mesh asset, at 5.78 fps on SwiftShader (p50 164.7 ms, p95 329.4 ms). Retain this representation;
these are software GPU/frame measurements, not headset performance. The cap keeps the renderer's
per-frame packed instance buffer below 8 MiB. Raise it only after measuring a different representation.
Negative determinants use a reflected X prototype with flipped winding and an X reflection folded
into each placement, leaving strictly positive renderer matrices. Original instance indices survive
the split into ordinary and reflected prototype batches. No negative renderer scale is accepted.

Move, rotate and scale gain instance-set behavior; the unioning `xform.*` patterns remain solid-only.
Group instances already evaluate to shared solid prototypes and require no document schema change.
The existing measured expansion guard remains 20,000 nodes; the former 50,000 documentation was
aspirational. Per-instance materials and individual instance editing are outside G4c.
