# 0015: Rigid assembly alignment; socket decisions pending

- Date: 2026-10-13
- Status: draft; socket ownership and default attachment rotation require owner decisions.

`xform.align@1` takes a Solid plus nonzero `from` and `to` directions and a `pivot` (metres,
default origin). Direction magnitude does not affect the rotation. The rotation is a proper rigid
transform and keeps the pivot fixed. Parallel directions produce identity. For opposite directions,
choose the coordinate axis with the smallest absolute source component (ties X, then Y, then Z),
and rotate by pi about the source crossed with that axis. Other directions use their normalized
cross product and `atan2(cross length, dot)`; this preserves small angles near parallel inputs.
Zero directions are refused during preflight, naming the node. The checked kernel rigid transform
performs geometry movement; material is retained. Instance inputs remain outside this operator's
contract. This independent alignment contract does not settle the pending socket decisions.

Proposed next contract from the handoff: named proper rigid frames carried on evaluated values;
attachment uses `F_target * inverse(F_source)` so both position and orientation coincide. Neither
socket ownership nor this default is accepted until the owner answers. Frame propagation, mirroring,
group exposure, attachment diagnostics and inspector socket selection remain to implement.
