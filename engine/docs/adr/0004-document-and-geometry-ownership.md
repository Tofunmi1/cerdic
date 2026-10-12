# 0004: The Rust engine owns the authored document and the geometry model
- Status: accepted (owner, 2026-10-12)
- Date: 2026-10-12

## Context
Track 2 adds many tools. Without one authoritative document, the editor, the LLM, the node graph and
XR manipulation would each hold their own state. The repository currently has two command layers: the
root app's TypeScript slice (`shared/commands.ts`: create, update, delete of three shapes) and none in
the engine.

## Decision
- One versioned authored document, owned by the Rust engine (`ve-graph`), with stable IDs, typed
  parameters, links, outputs, materials, seed, units and an editor layer (positions, frames,
  reroutes). Everything changes through `CommandBatch` with `base_revision` and a transaction key.
- Geometry is a `GeometrySet` of typed components (`ve-geometry`): `Solid`, `PolyMesh`, `Profile2`,
  `Curve3`, `Surface`, `Instances`, `Field3`, later `Strokes` and `Sketch`. Conversions are explicit
  nodes with a stated tolerance. A render mesh is derived, never the only representation.
- Operators are described by `OperatorSpec` in the existing registry. Help, TS types, tool schemas,
  inspector widgets and node-search entries are generated from it.
- The editor layer is part of the document but outside the evaluation hash.
- Commands and diagnostics follow `docs/modeling/commands.md` and `diagnostics.md`.

## Consequences
The TypeScript slice is retired after V0. Pure crates `ve-geometry`, `ve-graph` and `ve-modeling`
join `ENGINE_AGNOSTIC`. Adding a representation needs a working operation and a case list, not just
a type. Stable authored identity is guaranteed; stable face indices after booleans are not.
