# ADR 0025: Procedural texture programs on authored materials

- **Status:** Accepted
- **Date:** 2026-10-16
- **Decision owners:** Spatial Lab maintainers

## Context

The browser texture compiler can parse Material Maker-derived programs and bake channels, but authored
graph materials had no serializable way to refer to a program. Coupling the text source to page-local UI
state would make save/reload, undo and LLM-authored edits unreliable.

## Decision

Add `solid.texture@1(in, program)` with an 8,192-byte string bound. It preserves geometry and overlays
the source on the current PBR material. On a material region or placement prototype, it applies to each
surface while retaining that surface's PBR values. With no existing surface it creates the default
neutral PBR material. Texture program parsing and backend compilation remain explicit downstream stages;
the modeling evaluator treats this source as opaque data and does not claim that it has rendered it.

The source is carried as `Material.texture_program` in evaluation output. This keeps the saved document
and command system authoritative while leaving parser/compiler implementation in the browser texture
module. The language's string byte limit is the first resource bound; stricter DSL/token/node limits are
the compiler's responsibility when the program is parsed.

## Consequences

- Texture source survives graph evaluation and serialization with existing PBR attributes.
- Existing material propagation rules carry the source wherever they carry the material.
- The command can be authored by either an LLM or manual UI using the same typed graph command path.
- A deterministic box-chart UV fallback is available to the glTF exporter, but it is not a seam-aware
  unwrap and no texture images/material maps are exported yet. Channel baking, viewer display, and
  texture-aware MCP tools require separate data-path work and focused tests.
- Programs larger than 8,192 bytes are rejected by graph parameter validation.

## Validation

`crates/ve-modeling/tests/materials.rs` verifies unchanged geometry/PBR values and serializable texture
source. `cargo test -p ve-modeling --test materials` and `cargo test -p ve-graph --lib` pass.
