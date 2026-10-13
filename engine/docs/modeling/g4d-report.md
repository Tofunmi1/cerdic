# G4d sockets and attachment — prerequisite only (2026-10-13)

Status: incomplete. G4d-01 through G4d-05 are not implemented. G4 remains open.

The checked `Solid::transform([f64;12])` prerequisite is implemented. The matrix is row-major 3x4,
with translation at entries 3, 7 and 11. Finite proper rigid transforms preserve handedness;
scale, reflection, invalid lengths, null pointers and invalid matrices are refused. Native tests
cover bounds, volume and inverse round trip. C++ sanitizer cases and the native/WASI parity corpus
exercise the new ABI. These passed in `target/g4c-strict-01.log`.

The handoff's section 6 requires owner decisions before finalizing ADR 0015. Questions were sent
and remain unanswered:

1. Are sockets carried by evaluated values without group interface version changes (recommended),
   or explicit versioned group interface entries?
2. Does attachment match position and orientation by default (recommended), or translation only?

No ADR 0015 has been finalized. Remaining work: named frames, group propagation, frame-aware
transform/mirror behavior, attach and align, typed socket errors, inspector selection/listing,
assembly gallery and MCP guide, then focused native/browser tests and a fresh strict gate.
No physical headset testing has been performed.
