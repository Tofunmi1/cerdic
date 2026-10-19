# ADR 0045: Generated creature maker
- Status: proposed
- Date: 2026-10-18

## Context

The editor needs a way to make a complete editable animated creature without importing a model. The body, rig, and clips should be ordinary modeling graph operations, and a user should be able to supply either a starter plan or a custom skeleton description.

## Decision

`crates/ve-modeling/src/creature.rs` defines and validates a creature specification of bones and blobs. `creature/templates.rs` generates biped, quadruped, and serpent skeletons from bounded sliders. `creature/body.rs` emits tapered `field.tube` limbs and blended body fields; `creature/rig.rs` writes rig bones, motion tracks, clips, `rig.skin`, material and toon nodes as graph text. This is procedural geometry, not imported model data.

The tapered body tubes use `field.tube`'s `end_radius` support. `crates/ve-modeling/src/plan/fields.rs` includes that end radius in its surface-area estimate; the planner uses an area-based triangle estimate with a factor of four over `edge_length²`, bounded by its primitive estimate. The generated skin uses the existing distance-based `rig.skin` rule.

The `make_character` MCP tool is declared in `tools/mcp/server.mjs`; `src/engine/mcp-bridge.ts` calls the runtime creature generator and applies its resulting commands. `web/docpanel.js` exposes a document-panel character button that uses the same runtime generation path.

Validation limits generated rigs to 32 bones, including both sides of paired bones, and limits parent-chain depth to 8. Generated creature and part names are bounded so the longest emitted node name fits the graph language's 32-character node-name limit.

## Consequences

- Users can start from a plan or describe bones, blobs, roles, pairing, radii, and motion phase; generated output remains editable as graph text.
- The generated body is one smooth skinned solid, with bone parts and walk, run, and idle clips written as regular nodes.
- The body quality is bounded by field meshing and the creature resolution; the implementation does not claim artist-authored anatomy or production rigging quality.
- Bone, depth, and name limits are rejected during creature validation instead of producing invalid graph node names or truncated hierarchies.

## Alternatives considered

- An imported character asset would not satisfy the procedural geometry policy or preserve this graph-level editability.
- Requiring users to author all field, rig, and animation nodes manually would omit the plan and JSON character-generation interfaces.

## Evidence

`crates/ve-modeling/tests/g11_creature.rs` checks all three plans, dimensions, symmetry, slider changes, clips, invalid skeletons, and maximum generated names. `crates/ve-modeling/tests/g11_tapered_tube.rs` checks taper volume and direction. The MCP and panel entry points are in the files cited above. Tests were inspected, not run for this documentation task.
