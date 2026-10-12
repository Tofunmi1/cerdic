# G3 kernel progress

G3 is in progress, not passed. The language/parser, provider integration, evaluator profile values,
wall constraint, incremental vase tests, fuzzing and gallery review remain to be implemented.
G2 is complete in commit `5adfe41`; its report records the strict gate and release measurements.

The safe `ve_kernel::Profile` API and checked C ABI now expose polygon loops, miter offset,
profile difference, extrusion (slabs, twist and top scale) and Y-axis revolve. Owned native handles
remain private; canonical f64 loops can be exported for the future cache. Initial polygons reject
self intersections, zero area, non-finite and excessive input. Empty erosion is a valid empty region;
zero-height extrusion is rejected. Input caps: 64 loops, 4096 points per loop, 16384 total points;
segments 3..512, divisions 1..256. Revolve refuses negative radii rather than silently clipping them.

Tests were written first. Native conformance covers G3-07 to G3-12; C++ tests exercise owned results,
null inputs, malformed offsets, short output buffers, self intersections, non-finite parameters and
bounded work under ASan/UBSan. New C++ source and headers are included in format/tidy gates. Native
clippy passed. `cargo xtask parity` passes: kernel corpus identical in 18 lines, evaluator in one;
the added cases include a hollow extrusion, twisted extrusion, full/half revolve and canonical
profile offset loops.

Two numerical contract issues were exposed rather than hidden:

- A 64-facet revolve cannot meet 0.001 circular-volume accuracy. Its faceted volume is checked to
  1e-6, circle to 0.002, with a separate 128-facet case at 0.001. A half revolve uses the same
  full-circle angular step (minimum three slices for very small angles).
- Backend extrusion with a fixed diagonal had 0.004065 relative volume excess even at 128 slabs.
  Our extrusion factory triangulates caps with Manifold and uses symmetric side fans, removing the
  first-order twist bias. The resulting f64 mesh is checked by Manifold. Native volume/bounds tests
  now pass. Vendor source was not changed.

[ADR 0010](../adr/0010-first-language-numeric-contracts.md) records these decisions and corrects the
vase fixture's inner radius, failing edit direction and six-node count. Operators remain `draft`;
passing kernel cases alone does not establish the end-to-end G3 gate. No headset check is claimed.

Evidence logs: `target/g3-profiles.log`, `g3-kernel-clippy.log`, `g3-cpp.log`, `g3-parity.log`.
The full regression run in `target/g3-kernel-check.log` passed (230 native tests, WASM lint,
C++ checks, parity, existing XR/editor/worker browser tests and repository rules). A final constructor
guard was added while its browser phase ran: oversized polygons are rejected before copying them;
the updated profile tests and native clippy were rerun and passed. G3 is not committed or declared
complete; the final whole-G3 strict run is still required after the remaining integration work.
