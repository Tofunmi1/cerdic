# G8 spatial fields: implementation status

Date: 2026-10-14. G8 is in progress; this report deliberately does not claim full handoff
acceptance.

## Shipped in this slice

- Rust expression values and typed `Field` socket, with sphere, ellipsoid, rounded box, torus, capsule,
  cylinder, plane, hard booleans, polynomial smooth union, move, uniform scale, mirror, offset and
  shell instructions.
- Rust tape lowering/reference evaluator and C++ C-ABI tape execution through Manifold `LevelSet`.
- Typed refusal for invalid parameters, clipping, empty output, cancellation and bounded cell/tape
  work. A field cannot connect directly to a solid socket.
- The text graph exposes sphere, ellipsoid, plane, box, torus, capsule, cylinder, bounded tube and bounded repeat primitives; union, smooth union,
  intersection and subtraction; move, uniform scale, mirror, offset and shell; and explicit
  `field.to_solid`. A meshed sphere can feed the existing solid boolean path. The cache key already includes source
  operator parameters, dependency keys, bounds, edge length and quality through the authored node
  parameters.
- Eight geometry/kernel tests cover analytic sphere convergence, torus volume and genus, every
opcode's Rust/C++ sample parity, ellipsoid guarantees, bounded failure modes and propagation. Six graph tests
  cover primitive/combine/transform graph use, meshing into a boolean, clipping and socket type
  rejection, preflight limits and Preview scaling.
- `field.tube` now builds a bounded polyline tube from validated points. Its tape lowers to typed
  endpoint records, exact segment-distance instructions and unions; the 128-point input cap is
  included in planner work estimates. Two-point tubes report `Exact`; multi-segment unions report
  `LowerBound`.
- `field.repeat` folds coordinates into a bounded periodic domain, with a maximum of 16 copies per
  axis. It uses one tape instruction and preserves the child's distance guarantee. The Rust and C++
  evaluators agree bit-for-bit; repeat is included in native/WASI corpus parity.
- `field.offset` and `field.shell` now emit `field.distance_inexact` warnings when their input
  guarantee is not `Exact`; exact sphere offset remains warning-free. These warnings carry the
  authored node IDs through the existing evaluation warning channel.
- Field groups and instances now have a persistence regression: canonical save/reload preserves
  output meshes, and an instance radius edit keeps stable IDs with exact mesh restoration through
  undo and redo.
- Two field gallery fixtures now exercise repeated tubes and a procedural creature assembled from
  ellipsoids, tube limbs/tail, smooth unions and separately meshed eyes. Rust evaluation confirms
  positive-volume mesh outputs. All six fixed SwiftShader views passed the gallery visibility
  checks. At the 0.026 m authored body spacing, the Preview render at 0.052 m removes the earlier
  sawtooth limb faceting. Front and three-quarter views read as a small stylized four-legged
  creature; the right view shows the rear tail; top view shows the broad torso and paired eyes;
  underside shows four legs and the tail connection. It remains a simple procedural regression
  fixture, not a finished asset. Repeat-tube views show two bent tubes; top and three-quarter views
  make the spacing easiest to judge.
- A quality regression test enforces the 200,000-triangle Preview cap and compares the Preview result
  with Final: total volume differs by at most 12%, and each bounds face by at most 0.06 m. The body
  bounds yield 70 × 74 × 58 (300,440) Final samples at 0.026 m and 35 × 37 × 29 (37,555) Preview
  samples at 0.052 m. The interactive page requests Preview by default and does not upgrade idle
  documents to Final; Final is available only through an explicit evaluator request. Automatic idle
  refinement is not implemented.

## P0 measurements and limits

See [ADR 0022](../adr/0022-bounded-spatial-fields.md) for methods and detail. The standalone
Manifold release sphere took 0.267 s native / 0.430 s WASI at 1M samples and 1.938 s / 3.031 s at
8M. With a 64-instruction tape: 1.120 s / 1.291 s and 9.042 s / 10.133 s. WASI linear memory was
about 89 MiB at 1M and 386 MiB at 8M. The production Rust-to-kernel 1M/64-op run took 1.506 s,
producing 91,728 triangles and 1.767233886518835 m³ volume.

### Production adapter release measurements

`cargo run --release -p ve-modeling --example field_measure -- <sphere|creature>` was run three
times natively and as `wasm32-wasip1` under Node 26.7.0 on an Apple M1 arm64 development machine.
The sphere fixture samples 1,030,301 points at 0.02 m; the creature has 300,440 body-grid points
and 2,475 eye-grid points at Final. Native median evaluator time was 344.7 ms for the sphere
(323.4–850.5 ms range) and 286.9 ms for the creature (269.9–300.9 ms). WASI median evaluator time
was 355.0 ms for the sphere (330.8–451.4 ms) and 349.1 ms for the creature (328.9–350.4 ms).
WASI linear memory after evaluation was 54,722,560 bytes (52.2 MiB) for the sphere and 23,134,208
bytes (22.1 MiB) for the creature. The WASI host elapsed time also includes module startup and was
368.8 ms median for sphere and 364.0 ms for creature. These are local Node WASI runs, not browser
or headset results. The sphere produced 40,536 triangles; the creature produced 29,112.

Enforced limits: 2M sampled grid points, 64M point-instruction work, 4,096 instructions, and
2,097,152 padded voxel slots. Eight million cells are not accepted. `LevelSet` returns a closed
clipped solid for a sphere crossing the bounds, so the adapter samples the six bounds faces; the
check can miss a feature thinner than the grid. Native/WASI `sin`, `cos`, `exp`,
`pow`, `atan2` and `tanh` differed over one
million inputs; these functions are absent from the current tape. Seven production adapter cases
(sphere, torus, ellipsoid, smooth blend, creature-like capsule blend, polyline tube and bounded
repeat) run in `ve-modeling/examples/corpus.rs`; `cargo xtask parity` matched 37 lines.

## Remaining G8 acceptance

Current graph operators (19) are `field.sphere`, `field.ellipsoid`, `field.plane`, `field.box`, `field.tube`,
`field.torus`, `field.capsule`, `field.cylinder`, `field.repeat`, `field.union`, `field.smooth_union`,
`field.intersect`, `field.subtract`, `field.move`, `field.scale`, `field.mirror`, `field.offset`,
`field.shell` and `field.to_solid`. A plane by itself is unbounded and will normally be refused as
clipped when meshed. Graph preflight checks field samples, tape instructions, sample-instruction work
and padded voxel slots. It estimates triangles from primitive count and longest grid axis; the actual
post-mesh triangle check remains authoritative. Preview doubles field edge length, and tests cover
Preview/Final cap behavior and sphere volume within 0.08 m³. The sampling callback checks for
cancellation every 4,096 points; the native regression case asserts return within 500 ms for a
64-instruction tape and small grid (observed 0.7–3.3 ms in recent runs). Accepted-cap and WASI
latency are not measured. Native/WASI parity covers seven field cases. Deterministic noise,
displacement, twist, bend, inspector display and MCP text path remain open. Named field regions are
deferred for G8 because the current `LevelSet` adapter does not preserve primitive source labels;
separate `field.to_solid` outputs are the supported workaround. The
creature gallery fixture passes browser rendering and has a six-view silhouette review recorded above.
Wasm size delta is not measured. There is no
headset run.

## Validation run

- `cargo test -p ve-geometry --test g8_fields`: 8 tests passed in the latest quick gate.
- `cargo test -p ve-modeling --test g8_graph`: 6 passed, including distance-inexact warning coverage.
- `cargo xtask parity`: native/WASI output identical for 37 corpus lines, including seven field cases, the polyline tube and bounded repeat.
- `cargo clippy -p ve-modeling --all-targets -- -D warnings` and `cargo clippy -p ve-geometry -p ve-modeling --all-targets -- -D warnings` passed during the planner/module work.
- `cargo xtask cpp`: passed clang-format, native ASan/UBSan tests, and WASI clang-tidy after splitting C++ meshing (`field_mesh.cpp`) from tape validation/interpreting (`field_tape.cpp`). The in-sampling cancellation regression measured 0.7–3.3 ms across runs and asserts under 500 ms for its 64-instruction, 0.021 m grid case. This is not an accepted-cap or WASI latency measurement.
- `cargo test -p ve-modeling --test gallery`: repeat and creature field outputs passed with positive finite volumes and nonempty meshes.
- `cargo xtask web-test dev gallery`: all three shards passed, including six-view SwiftShader captures for both new fixtures and the existing vase edit/undo/reload sequence.
- `GALLERY_ONLY=field_creature GALLERY_SHARD=1/3 GALLERY_PORT=5191 bun web/tests/gallery.mjs`: passed after refining the creature grid and moving its tail behind the body.
- `cargo test -p ve-modeling --test g8_persistence`: field group save/reload, instance edit and undo/redo test passed.
- `cargo test -p ve-modeling --test g8_graph inexact_offset_and_shell_report_distance_warnings_but_exact_offset_does_not`: passed.
- `cargo xtask quick ve-geometry ve-graph ve-kernel ve-modeling`: passed after all current G8 code changes (456 tests, Clippy, formatting and rules).
- `cargo run --release -p ve-modeling --example field_measure` and `tools/run-wasi-measure.mjs`: three-run native and Node WASI production-adapter measurements for sphere and creature; results and limits are recorded above.
- `cargo xtask parity`: passed after the final field evaluator refactor; `ve-kernel` identical (20 lines), `ve-modeling` identical (37 lines).
- The full strict gate passed all fresh stages on 2026-10-14 (602 Rust tests and all browser scripts: 32 pool jobs plus 3 serialized timing checks, SwiftShader only; `target/reports/g9-fidelity-pass1.log`, `exit=0`). G8 acceptance remains open. Browser release-WASM size and accepted-cap/WASI cancellation latency remain unverified. Production-adapter WASI time and memory are measured above for two fixtures.
