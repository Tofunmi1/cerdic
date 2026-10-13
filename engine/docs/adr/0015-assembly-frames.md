# 0015: Named rigid assembly frames and attachment

- Date: 2026-10-13
- Status: accepted by the owner for implementation and implemented.

Named sockets are carried by evaluated values, like material. They do not belong to a group's
versioned interface: the group's result carries its value frames through each use. A new socket
therefore does not force a group upgrade.

`frame.add@1(in: Solid, name, position, rotation)` adds one unique named rigid frame. Positions
are in metres and rotations are Euler angles X then Y then Z (radians in document values; degrees
in text without a suffix). Frames contain a finite position and a unit proper quaternion. Names
are 1 to 64 ASCII letters, numbers, `_`, `-` or `.`. One value can carry at most 128 frames.

`xform.attach@1(in: Solid, to: Solid, socket, to_socket)` moves the source so **both** named
frames coincide: `T = F_target * inverse(F_source)`. This matches position and orientation by
default. The operation is a checked proper rigid transform, keeps the source material and frames,
and leaves the target value unchanged. Missing names produce `socket.unknown` at the attach node,
name both input ports, list the related nodes, and suggest the frames available on the input.
Input kinds are checked by the graph before evaluation.

Transforming values transforms socket position and extracts the nearest proper orientation, so
nonuniform positive scale never puts scale into a frame. Mirrors use `M * F * M`; names on the
reflected copy receive the `mirror.` prefix, while originals keep their names. Array, repeat and
radial patterns keep the first copy's names and prefix later copies with `copyN.`. A generated-name
collision is an explicit refusal. Predicted frame growth is capped at 128 before geometry runs.
`inst.realize` expands prototype frames with the same `copyN.` rule; un-realized sets retain their
shared prototype frames and per-placement transform matrices.

`xform.align@1` aligns nonzero directions about a fixed pivot. Parallel inputs are identity;
opposite directions use a deterministic half turn about the source crossed with the least-aligned
coordinate axis (ties X, Y, Z). Other angles use cross-product `atan2`, which preserves small turns.
Its rigid transform also propagates frames and material.

Sockets appear by name and position in the viewer inspector and can be selected there. The
selection is view state. Drawing frame gizmos and freehand socket picking remain V3 work.
