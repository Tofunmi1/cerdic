# G4d sockets and attachment (2026-10-13)

Status: complete (2026-10-13). Strict validation, release build and advisory check passed.
The owner chose result-carried sockets without group interface version bumps, and attachment that
matches both position and orientation. ADR 0015 records these decisions and the implemented rules.

The checked `Solid::transform([f64;12])` prerequisite is implemented. The matrix is row-major 3x4,
with translation at entries 3, 7 and 11. Finite proper rigid transforms preserve handedness;
scale, reflection, invalid lengths, null pointers and invalid matrices are refused. Native tests
cover bounds, volume and inverse round trip. C++ sanitizer cases and the native/WASI parity corpus
exercise the new ABI. These passed in `target/g4c-strict-01.log`.

`xform.align@1` accepts a solid, two nonzero
directions and a pivot (default origin). It applies the shortest proper rotation for non-collinear
directions. Parallel inputs are identity; opposite directions rotate about the source crossed with
the coordinate axis whose absolute source component is smallest (ties X, Y, Z). The pivot stays
fixed and material remains intact. Zero directions are refused in preflight with the node named.
Instance-set inputs are outside its contract and are rejected by port validation.

Four native tests cover direction magnitudes, explicit pivot, volume/bounds, parallel and opposite
directions, zero-direction errors, small angles and oblique inputs. The parity corpus adds oblique
alignment. The `aligned_beam.txt` gallery piece exercises the public text operator through the
worker and projection. Native socket tests cover group propagation, all-axis reflection, pattern
namespacing, attachment and cache invalidation when the parent moves. The socket table fixture
attaches four legs and keeps its named parts separate. The inspector/browser test covers a selected
socket's output name and transformed position, and selecting a group lists its result socket.

`cargo xtask check --strict` passed with 377 native tests, WASM lint, C++ sanitizer/tidy,
native/WASI parity (20 kernel lines, four evaluator lines), all browser suites including IWER XR,
the socket inspector acceptance test, and source rules. Log: `target/g4d-strict-01.log`.
`cargo xtask advisories` passed (`target/g4d-advisories.log`); `bun run typecheck` passed.
All six aligned-beam and socket-table gallery views were reviewed. The table has four aligned legs;
native bounds tests establish the attachment and pivot invariants quantitatively.

`bun run build` passed (release WASM, schema checks, TypeScript and Vite); log:
`target/g4d-release-build.log`. Brotli q9 measurements strip custom sections only in memory:

| Artifact | Raw bytes | Brotli bytes | Stripped raw bytes | Stripped Brotli bytes |
| --- | ---: | ---: | ---: | ---: |
| Runtime | 34,466,766 | 4,152,912 | 19,536,173 | 3,503,538 |
| Evaluator | 1,301,862 | 359,532 | 1,050,788 | 325,572 |

Compared with the preceding G4d align build, stripped Brotli changes are -824 B runtime and
+12,260 B evaluator. The typed graph operators expose `frame.add@1` and `xform.attach@1`; the inspector reads
result-carried sockets on parts and groups, including instance placements. Full limits, propagation
and diagnostics are recorded in ADR 0015 and the modeling reference docs.

There are no frame gizmos or freehand viewport socket picking; those remain V3 work. XR entry and
controller selection are covered by browser emulation elsewhere in the strict gate; no physical
headset was tested or needed for this socket and attachment slice.
