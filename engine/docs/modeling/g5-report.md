# G5 surfaces progress report

**Status: incomplete; G5-14 still lacks configured-provider evidence.** Updated 2026-10-14. The
`cargo xtask check --strict` evidence is `target/next-after-g5-strict-03.log` (458 tests; 20 kernel and 16
modeling parity lines match between native and WASI; browser suites ran on SwiftShader). ADR 0017 remains
Draft pending maintainer acceptance. Two manual provider-backed model runs (pipe and vase, DeepSeek V4
Flash through Claude CLI and AgentRouter) were reported in chat; their transcripts are not checked in,
so this report does not claim reproducible provider acceptance coverage.

## Implemented

- Authored open/closed polylines, sampled cubic Béziers (closed cubics require matching endpoints), and
  sampled circles. Sweeps support open paths and closed sampled paths with a single convex profile loop;
  lofts accept 2 to 64 compatible convex sections.
- `curve.hang@1(from, to, slack, points)` samples a catenary rope between two tie points under gravity.
- Double-reflection sweep frames, closed seam holonomy correction, sampled curvature checks, and two
  self-intersection layers: the original sufficient path-distance rule (unchanged), then a bounded uniform
  triangle grid that catches cap/tube, bend-vertex and parallel overlap fixtures. A separated near-miss builds.
  Preflight reserves 24 MiB per sweep; runtime caps cell references and candidate pairs. This uses floating-point
  triangle predicates with tolerances, not exact arithmetic.
- Hollow six-section lofted vase plus straight and curved hollow sweep compositions. For matching corresponding
  vertices with linear section interpolation, outer-minus-inner separation is linear per span and its minimum is
  at a section. The new thin-wall case has a 0.005 m section gap between old 0.035 m samples. This assumes equal
  point counts, valid correspondence, inner inside outer at every section, and a raised inner loft; it is not a
  universal runtime check through arbitrary floor/rim transitions.
- Twist, taper and bend with edge midpoint and face-centroid samples. Shared-edge local splitting stays conforming
  without T-junctions; at most two rounds preserve the 16x triangle-growth preflight. Sampled face normals and
  deformation orientation are checked, then `Solid::from_mesh` validates the solid. Thin hard-bend and twisted-loft
  fixtures pass; impossible sampled tolerance refuses. This is not a whole-face deviation bound.
- Save/reload/history/group/instance sharing plus tests distinguishing one instance override from a group
  definition edit.
- Five chained unions of overlapping box inputs pass Preview preflight and realize a 12-triangle solid. The
  union estimate uses the sum of its estimated inputs plus a capped 16,384-triangle intersection allowance;
  this is a bounded planning heuristic, not a bound on every possible boolean arrangement.
- Native/WASI parity corpus: 16 modeling output lines match byte-for-byte.
- Thirteen checked-in gallery programs cover pipe elbow, lofted vase, curved panel, authored-polyline coil
  spring, convex twisted-loft blade, staircase, lamp, goblet, bridge, sword, toy car, smooth-vase comparison
  and fused multi-colour car. The expanded gallery is covered by native evaluation and six fixed SwiftShader
  views per program; generated images are in the ignored `target/reports/gallery/` directory.
- Desktop SVG curve display and camera-plane Bézier/polyline control-point dragging, with preview,
  commit and undo coverage. This overlay is not rendered per eye.
- The scripted MCP browser client builds a pipe and loft and edits both through stable IDs. It does not
  exercise an external natural-language model or provider.

## Measurements

| Artifact | G4c recorded raw | Current raw | Delta | G4c recorded Brotli | Current Brotli | Delta |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Runtime WASM | 34,464,260 | 34,499,928 | +35,668 | 4,153,657 | 4,116,598 | -37,059 |
| Evaluator WASM | 1,241,084 | 1,405,870 | +164,786 | 345,361 | 394,881 | +49,520 |

The comparison baseline is the recorded G4c release. It includes intervening G4d, V3 and G5 work, so these
deltas cannot be attributed to G5 alone. Current sizes are from a fresh `cargo xtask web` release build on
2026-10-14; Brotli quality 9 was measured on the resulting WASM files with custom
sections retained. The decrease in runtime Brotli size despite a raw increase reflects compression changes,
not a measured feature-level saving.

`g5_performance.rs` evaluated a 64-section loft with 32-point circular sections at 4,096 output triangles
in Preview and Final. One local native release run measured 10.55 ms (Preview) and 7.85 ms (Final). This is
a single Apple Silicon measurement; it is not a latency guarantee. Loft tessellation is unchanged between
these quality tiers.

## Validation run

- `cargo fmt --all -- --check`
- `cargo test -p ve-graph -p ve-modeling -p ve-interact`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo clippy -p ve-runtime --target wasm32-unknown-unknown -- -D warnings`
- `cargo xtask parity` (20 kernel lines, 12 modeling lines, identical)
- `cargo xtask rules` (file-size and dependency-boundary gate)
- `cargo xtask check --strict` (all gates passed; `target/next-after-g5-strict-03.log`; 458 tests; SwiftShader browser/XR suites; 20 kernel and 16 modeling parity lines)
- `cargo xtask web` (fresh 2026-10-14 release WASM build; raw and Brotli sizes above)
- `cargo test -p ve-modeling --test g5_mesh_overlap --test g5_hollow --test g5_deformers`
- `cargo xtask web` (release WASM build)
- `bun run typecheck`
- `cargo test -p ve-modeling --test gallery`
- `cargo test -p ve-modeling --test g5_performance g5_31_five_chained_unions_of_overlapping_boxes_fit_preview`
- `cargo xtask rules` (after the union estimate change)
- `bun web/tests/gallery.mjs` (six fixed SwiftShader views per gallery program)
- `bun web/tests/mcp.mjs`
- `bun web/tests/v3-curve-points.mjs`
- `bun web/tests/v3-drag-guards.mjs` (also included in the strict browser suite)

No headset or physical GPU was tested.

## Remaining blockers

- G5-14's checked-in MCP test uses scripted tool calls. Two manual model runs were reported, but no provider
  is configured in the repo and those runs are not reproducible from this tree.
- Extend checks across difficult near-coplanar/touching sweep meshes and larger grid workloads; current focused
  cases do not establish exact-predicate robustness for degenerate inputs.
- Apply the section-gap wall argument only under its documented assumptions; arbitrary hollow compositions and
  floor/rim transitions remain outside that bound.
- Multi-colour regions still collapse to the first input's surface through twist, taper, bend, `inst.realize`,
  `xform.attach` and attached-frame handling. Extending labels through those flattening operations needs dedicated
  cases before it can be called supported; see ADR 0018.
- Configure a real provider/key and exercise natural-language follow-up for G5-14.
- The strict gate passed on the G5 code recorded above. Rerun it after any later G5 code change and capture/review
  remaining acceptance evidence before changing ADR 0017 from Draft.
