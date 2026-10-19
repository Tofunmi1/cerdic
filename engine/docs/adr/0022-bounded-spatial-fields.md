# ADR 0022: Bounded spatial fields

Status: Proposed (G8 implementation in progress)  
Date: 2026-10-14

## Context

Fields add organic shapes without changing the downstream solid/mesh contract. Manifold's
`ExecutionContext::LevelSet` samples a scalar function over an explicit box. It can return a
closed clipped result when the surface reaches that box, so the adapter must check the box faces.
Field evaluation costs roughly cells times tape instructions and Manifold allocates padded voxel
storage in addition to the sample grid.

## Decisions

1. A field is a spatial implicit function and must have its own graph socket type. Only
   `field.to_solid` may turn it into a solid.
2. Rust owns validated expression values and lowers them to a topological C ABI tape. C++ runs
   that tape inside `LevelSet`; no per-sample calls cross back into Rust or JavaScript.
3. Tape instructions use IEEE arithmetic, square root, min/max and absolute value. Bounded repeat
   folds coordinates with round and clamp and needs no transcendental. No libm
   transcendental is included until native/WASI parity has been proven. The current vocabulary
   therefore has no noise, twist or bend.
4. Primitives report `Exact`; hard min/max combinations report `LowerBound`; smooth union reports
   `None`. Uniform scale preserves the stated guarantee; offset and shell preserve their child's
   guarantee only as a scalar-field property and do not upgrade it.
5. Meshing requires explicit bounds and edge length. The adapter rejects sampled boundary points
   inside the field, an empty result, non-finite input, cancellation and work/storage overages.
   Boundary sampling is on the grid lattice; a feature thinner than that spacing can cross between
   samples undetected.
6. Initial enforced limits are 2,000,000 grid samples, 64,000,000 sample-instruction operations,
   4,096 tape instructions, and 2,097,152 padded voxel slots. These are lower than the reserved
   8-million-cell planning value because the measurement spike reached roughly 386 MiB of WASI
   linear memory at 8M cells before tape work. A 1M-cell run used roughly 89 MiB. The 64M work cap
   bounds a full 1M by 64 tape, whose production release run took 1.51 s on the baseline host.
7. Plane, ellipsoid, bounded polyline tube and bounded repeat are registered in the graph.
   Repeat is limited to 16 copies per axis and compiles to one instruction. Named field regions are
   explicitly deferred from G8: `LevelSet` currently returns geometry without per-triangle source
   labels, and no second label tape or propagation ABI exists. Separate `field.to_solid` outputs are
   the supported fallback; the creature keeps its eyes separate and makes no field-region claim.
   Deterministic noise remains follow-up work. Ellipsoid uses the minimum-radius-scaled
   unit-sphere field; equal radii are exact, while unequal radii are a 1-Lipschitz `LowerBound`, not
   exact Euclidean distance. Do not claim the remaining features until their parity and
   conformance tests pass.

## Measurements

The throwaway P0 adapter ran Manifold release builds, native and `wasm32-wasip1`, with a sphere
inside `[-1,1]^3`. At 1M samples the one-instruction baseline took 0.267 s native and 0.430 s WASI;
at 8M it took 1.938 s native and 3.031 s WASI. Approximate peak process RSS was 73 MiB / 127 MiB
at 1M and 404 MiB / 424 MiB at 8M; WASI linear memory was approximately 89 MiB / 386 MiB.
The 64-instruction tape took 1.120 s / 1.291 s at 1M and 9.042 s / 10.133 s at 8M. The production
Rust-to-kernel path at 1M and 64 instructions took 1.506 s and produced 45,866 vertices, 91,728
triangles and volume 1.767233886518835 m³.

The sampled sphere converges toward analytic volume and area at finer resolutions; the torus
fixture checks analytic volume and genus one. A sphere that crosses the requested box is returned
by Manifold as a closed clipped solid, confirming that a post-mesh manifold check cannot detect
this error. The adapter's lattice boundary check catches the ordinary case.

The release `ve-modeling` production adapter was measured three times natively and under Node 26.7.0
WASI on Apple M1 arm64 using `examples/field_measure.rs`. A 1,030,301-point sphere at 0.02 m took
344.7 ms median native (323.4–850.5 ms) and 355.0 ms median WASI (330.8–451.4 ms), with 52.2 MiB
WASI linear memory and 40,536 triangles. The 32-instruction creature body used 300,440 field-grid
points plus 2,475 eye-grid points; it took 286.9 ms median native (269.9–300.9 ms) and 349.1 ms
median WASI (328.9–350.4 ms), with 22.1 MiB WASI linear memory and 29,112 triangles. The Node
process wall time includes module startup and had medians of 368.8 ms for sphere and 364.0 ms for
creature. These are local microbenchmarks, not browser-device or headset measurements.

Native and WASI sphere meshes matched byte-for-byte in the P0 standalone probe. Million-input
results for `sin`, `cos`, `exp`, `pow`, `atan2`, and `tanh` differed between native and WASI;
`sqrt` matched. The implementation excludes the differing operations. A native regression case
cancels a 64-instruction field with a 0.021 m grid and measured 0.7–3.3 ms from request to
return across recent native runs; it asserts under 500 ms. Maximum-workload and WASI cancellation latency and
browser release-size delta remain unmeasured.

## Consequences

The 8M planned cap is not enabled. Preview uses twice the authored edge length, reducing a cubic
grid to roughly one eighth the samples. The planner checks sample, tape, work and padded-slot limits
before evaluation. It estimates output triangles as primitive count × 10 × the square of the longest
grid axis; this is a planning heuristic for the current low-frequency vocabulary, and the actual
post-mesh triangle check remains authoritative. The sample callback checks cancellation every 4,096
points and returns a constant after observing cancellation so Manifold can leave its sampling phase;
accepted-cap and WASI latency remain unmeasured. Region labels and noise/deformation instructions remain follow-up work. Revisit caps after measuring on
the target browser memory budget and include a WASI run with the production adapter.
