# G4a report: the small operators for furniture and machines

G4a is the first slice of G4 (reuse and assembly): the operators a model needs to build a table, a
gear or a staircase without placing every part by hand. Groups with exposed ports, instances with
overrides, named parts and sockets are the rest of G4 and are not built.

## What exists

| Operator | What it does |
|---|---|
| `profile.rect@1(size)` | Rectangle centred on the origin of the XY plane. |
| `profile.circle@1(radius, segments)` | Regular polygon approximating a circle. |
| `solid.cone@1(height, radius, top_radius, segments)` | Cone or frustum along Y; `top_radius=0` is a point. |
| `solid.torus@1(major, minor, segments, tube_segments)` | Ring in the XZ plane; the tube must be thinner than the ring. |
| `solid.capsule@1(radius, height, segments)` | Cylinder with half-sphere ends along Y; at least as long as wide. |
| `xform.mirror@1(solid, axis)` | The solid plus its mirror image across the plane through the origin. |
| `xform.array@1(solid, count_x, count_y, count_z, spacing)` | A grid; the first copy stays put. |
| `xform.radial@1(solid, count, radius)` | Copies evenly around Y, each moved out along X first. |
| `xform.repeat@1(solid, count, step, rotate_step)` | Copy i turned by i times `rotate_step`, then moved by i times `step`. |

All nine are `ready`. Patterns fuse their copies into one solid with a single multi-way union and keep
the first input's material. Their cost is counted in preflight (copies times input triangles), so a
pattern over the triangle or memory budget is refused before any geometry is built; counts are
capped at 256 per axis.

## Evidence

- `crates/ve-modeling/tests/shapes.rs`: analytic volumes (rect, circle, cone, frustum, torus,
  capsule), bounds, the torus refusal, mirror with and without overlap, grid, ring and staircase
  extents, pre-geometry budget refusal (no node evaluated), material kept through patterns.
- `crates/ve-modeling/tests/gallery.rs` and `tests/gallery/{table,gear,spiral_stair}.txt`: three new
  programs match their analytic volumes (table within 0.01 %, gear and staircase within 2 %).
- `web/tests/gallery.mjs`: all seven gallery pieces go text, commands, worker, scene and are
  photographed from six fixed views into `target/reports/gallery/`.
- The MCP guide (`tools/mcp/server.mjs`) teaches the new operators with the table as the example.

## Not done, and known limits

- A pattern copy has no identity of its own: you cannot select, override or move one copy. That is
  G4 proper (instances).
- Mirror is across planes through the origin only; place the part off-centre first. Mirroring about
  a part's own frame needs the attachment frames of G4.
- Metallic materials used to look almost black because there was nothing to reflect. The renderer now
  has a procedural studio sky (`VeAmbientLight::reflection`, `environment()` in `forward.wgsl`: a
  gradient, a soft light box and a cool strip light, blurred by roughness, with Karis's split-sum
  fit for the Fresnel term). It is on in the studio stage and off elsewhere, so existing golden
  scenes are unchanged. `web/tests/reflection.mjs` checks that the steel gear is lit and shows a
  gradient and that the red vase has a white highlight. It is an analytic stand-in: no real
  environment map, no reflections of the room or the model itself, no occlusion.
- Cone, torus and capsule are faceted by `segments`; pair them with smooth shading for a round look.
