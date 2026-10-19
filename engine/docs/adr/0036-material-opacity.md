# ADR 0036: Material opacity

- Status: proposed
- Date: 2026-10-17

## Context

Water, windows and glass need material alpha that survives evaluation, browser rendering and glTF export. The renderer batches opaque instances by mesh, so transparent draws need separate ordering and must not write depth.

## Decision

Add `solid.material.opacity`, bounded from 0 (transparent) to 1 (opaque) and defaulting to 1 for old documents. Carry it through modeling, the typed worker material and the runtime projection. Keep opaque draws batched. Draw transparent instances after opaque geometry, sorted back to front by object origin with authored node ID as a deterministic tie-breaker, depth testing enabled and depth writes disabled. Export opacity below 1 with glTF `alphaMode: BLEND` and base-colour alpha; retain `OPAQUE` at opacity 1.

## Consequences

This is conventional surface transparency, not physically based transmission or refraction. Sorting object origins is approximate for large intersecting or concave transparent objects; those cases may need triangle sorting or weighted blended transparency later. Transparent surfaces can show backfaces and may exhibit sorting artifacts. Existing opaque materials retain alpha-one output.
