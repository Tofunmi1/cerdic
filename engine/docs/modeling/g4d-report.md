# G4d sockets and attachment — partial implementation (2026-10-13)

Status: incomplete. G4d-04 alignment is implemented and strict gate passed.
G4d-01, G4d-02, G4d-03 and G4d-05 are not implemented. G4 remains open.

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

Draft [ADR 0015](../adr/0015-assembly-frames.md) records the independent alignment contract and
explicitly leaves those socket decisions pending. `xform.align@1` accepts a solid, two nonzero
directions and a pivot (default origin). It applies the shortest proper rotation for non-collinear
directions. Parallel inputs are identity; opposite directions rotate about the source crossed with
the coordinate axis whose absolute source component is smallest (ties X, Y, Z). The pivot stays
fixed and material remains intact. Zero directions are refused in preflight with the node named.
Instance-set inputs are outside its contract and are rejected by port validation.

Four native tests cover direction magnitudes, explicit pivot, volume/bounds, parallel and opposite
directions, zero-direction errors, small angles and oblique inputs. The parity corpus adds oblique
alignment. The `aligned_beam.txt` gallery piece exercises the public text operator through the
worker and projection; the MCP guide describes the operator and states sockets are unavailable.

`cargo xtask check --strict` passed with 364 native tests, WASM lint, C++ sanitizer/tidy,
native/WASI parity (20 kernel lines, four evaluator lines), all browser suites including IWER XR,
and source rules. Log: `target/g4d-align-strict-01.log`. Separate advisories and TypeScript checks
passed. All six aligned-beam gallery views were reviewed: the 45-degree orientation and intact
cross-section are visible; native bounds tests establish the pivot invariant quantitatively.

`bun run build` passed (release WASM, schema checks, TypeScript and Vite); log:
`target/g4d-align-release.log`. Brotli q9 measurements strip custom sections only in memory:

| Artifact | Raw bytes | Brotli bytes | Stripped raw bytes | Stripped Brotli bytes |
| --- | ---: | ---: | ---: | ---: |
| Runtime | 34,465,228 | 4,153,534 | 19,534,635 | 3,504,362 |
| Evaluator | 1,245,350 | 346,551 | 1,004,565 | 313,312 |

Against the immediately preceding G4c build, stripped Brotli changes are +237 B runtime and
+1,303 B evaluator. Implementation/tests/gallery/MCP guide commit: `24792e5`.

No ADR 0015 has been finalized. Remaining work: named frames, group propagation, frame-aware
transform/mirror behavior, attach and align, typed socket errors, inspector selection/listing,
assembly gallery and MCP guide, then focused native/browser tests and a fresh strict gate.
No physical headset testing has been performed.
