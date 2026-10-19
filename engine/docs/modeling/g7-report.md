# G7 subdivision — implementation status and review follow-up

Date: 2026-10-14

## Implemented

- Vendored only OpenSubdiv 3.7.0 Far, Vtr and Sdc at commit
  `9dab8a47bfbb1388ec8388fe61f5f916e6123f38`, with its license and a vendor record.
- Added a static CPU library with exceptions and RTTI disabled and a narrow C ABI for uniform Catmull-Clark.
- Added checked input sizes, manifold/winding edge preflight, exact uniform growth recurrence, and a two-million
  output vertex/face cap (eight million output corners, four million cumulative level vertices). The kernel polls cancellation before topology refine
  and between each level's position interpolation.
- Added safe `ve-kernel` and `ve-geometry` entry points, including edge creases, vertex corner sharpness and
  boundary rules. Output includes immediate vertex provenance and cage-face provenance. Face attributes copy
  to child faces; integer point attributes are returned by name in `Subdivided::dropped_attributes`; edge
  attributes and unsupported continuous/corner interpolation refuse explicitly.
- Added deterministic shorter-diagonal triangulation for mildly curved subdivision quads. A fold whose two
  triangle normals meet at 90 degrees or more refuses. Curved quads convert to solid triangles; G6 v1 polygon
  editors have narrower contracts and subdivision is documented as the final editing step.
- Registered `poly.subdivide@1` for text and evaluation with levels 1–4, edges / edges-and-corners boundaries,
  and conservative triangle-budget preflight. Added a bevelled-block gallery fixture and MCP guide/test.
- Preserved color-region runs through `poly.from_solid` and subdivision by mapping refined face-region labels
  to child triangle spans; a two-color union test confirms both materials survive.
- Added a versioned C ABI input descriptor for interleaved point channels and one face-varying channel.
  UV values are welded by exact `(u, v)` bits in first-corner order and use OpenSubdiv linear face-varying
  boundaries; up to eight F32 point channels are refined with the same refiner and crease rules as positions.
  F64 point attributes keep their own typed refusal. Hole tags remain explicitly unsupported after checking
  the pinned OpenSubdiv source: only base faces are marked, with no child-hole inheritance contract exposed.
- `poly.grid` now emits continuous normalized corner `u`/`v` coordinates, so the gallery's `uv_subdivided_grid` fixture
  exercises channel refinement through the browser evaluator and viewport.
- Added the generated-UV grid to the browser gallery path; the geometry regression checks identity UVs after two
  subdivision levels, while the full gallery renders it through the viewport.
- Added native C++ sanitizer coverage and Rust cube/refusal tests with an independent Catmull-Clark reference.

## Validation

- `cmake --build engine/target/g7-cmake --target ve_kernel`: passed.
- Native ASan/UBSan `kernel_test` and `subdivision_test`: passed.
- `cargo xtask quick ve-kernel`: passed after edge provenance was changed to sorted undirected cage-edge rank.
  This stronger parent-by-parent comparison caught and fixed the former mismatch between OpenSubdiv edge
  ordinals and the independent reference's sorted edges.
- `cargo test -p ve-kernel --test subdivision --test subdivision_creases`: 8 passed. The cube and valence 3/5
  reference comparisons now match each child to its own immediate parent. Edge and corner sharpness are checked
  per parent at 0, 0.5, 1, 2 and 10 for one level.
- `cargo xtask quick ve-geometry ve-graph ve-modeling`: passed; the suite includes UV identity/island interpolation,
  named float-channel propagation, channel caps and the typed hole-tag refusal,
  warning aggregation test, and level-1/2 post-subdivision extrusion/bevel refusals.
- `cargo test -p ve-modeling --test g7_graph`: text graph subdivision, closed-solid conversion, persistence,
  and the terminal-edit refusal probes passed.
- `cargo test -p ve-modeling --test gallery g7_subdivided_bevelled_block_is_a_closed_smooth_solid`: passed.
- `bun run schema:generate`: passed; ts-rs emitted its existing serde-attribute parse warnings.
- `cargo clippy -p ve-kernel -p ve-geometry --all-targets -- -D warnings`: passed.
- `cargo check -p ve-kernel --target wasm32-wasip1`: passed.
- `cargo check -p ve-kernel --target wasm32-unknown-unknown`: passed.
- WASI subdivision test binary run with `node tools/run-wasi.mjs target/wasm32-wasip1/debug/deps/subdivision-c99372a3db4327b4.wasm`: 3 passed.
- `cargo xtask cpp` passed formatting, native ASan/UBSan tests, and clang-tidy against the WASI sysroot. The
  subdivision translation unit excludes two analyzer checks that report false positives inside OpenSubdiv's
  own template headers; all other project checks remain enabled.
- `cargo xtask web` built the release browser artifacts after adding the generated-UV gallery path. Evaluator WASM:
  2,026,733 raw / 561,495 Brotli-9 bytes (+72,083 / +10,993 from the prior G7 measurement); runtime WASM:
  34,514,066 raw / 4,122,233 Brotli-9 bytes (+94 / +1,658). The measurement uses `web/dist/*_bg.wasm` after a
  release build. The prior G7 evaluator measurement was 1,954,650 / 550,502; its runtime was 34,513,972 / 4,120,575.
- Release timing (`cargo run --release -p ve-kernel --example subdivision_bench`): 99,856 quad faces at level 1
  produced 399,424 quads in 103 ms natively and 190 ms under Node WASI. A four-face level-4 cage produced 1,024
  quads in under 1 ms on both. Near-cap outputs contain 1,982,464 quads and 1,985,281 vertices: level 4 took
  311 ms natively / 710 ms WASI; level 3 took 251 ms / 409 ms. These end-to-end times include adapter setup
  and output copying; they do not isolate the uninterruptible `RefineUniform` call.
- The near-cap native run peaked at 514,441,216 bytes maximum resident set size (`/usr/bin/time -l`). The WASI
  benchmark reported 419,364,864 bytes (400 MiB) linear memory at both near-cap cases. Keep the existing
  2,000,000 output caps; the measured WASI call stays below one second, while its roughly 400 MiB memory peak
  remains a material resource cost.
- Attribute memory was measured in isolated near-cap release runs (`grid(88,88)`, level 4; output 1,985,281
  vertices / 1,982,464 faces). One point channel returned 1,985,281 floats in 368 ms native / 582 ms WASI;
  peak was 515,211,264 bytes native RSS / 434,044,928 bytes WASI linear memory. Eight channels returned
  15,882,248 floats in 396 ms / 953 ms, peaking at 646,774,784 / 563,347,456 bytes. Continuous UVs returned
  1,985,281 unique refined values in 398 ms / 722 ms, peaking at 702,676,992 / 650,444,800 bytes. Keep the
  8-channel and 32-million-float caps: the eight-channel observed output is below 16 million floats due to the
  existing 2-million-vertex ceiling; the larger UV peak includes its corner-index remap storage.
- The modeling parity corpus includes the five edge/corner sharpness values, open-boundary grid, dropped-attribute
  warning payload, and a creased cube with two F32 point channels and six UV islands; native and wasm32-wasip1
  output must match byte-for-byte.
- Full strict no-cache gate: final run is recorded in `engine/target/reports/g7-attributes-final-gate.log`; it includes C++
  format, ASan/UBSan, clang-tidy, native/WASI parity, full web build and browser suite. It passed 582 Rust test cases and
  all 32 browser scripts; the browser stage took 620.8s. Browser rendering uses SwiftShader; it has not been tested on a
  headset. The log ends with `exit=0`.

## Explicit limits

These paths fail explicitly instead of silently losing data:

- Paired F32 corner `u`/`v` and up to eight F32 point channels are supported, capped at 32 million refined
  floats. F64 stays refused with `poly.subdivide.f64_unsupported`; incomplete or malformed UV pairs use
  `poly.subdivide.uv_incomplete`.
- Edge and non-UV corner attributes return their typed unsupported codes. Integer point attributes are dropped
  by policy, listed by name by the geometry adapter, and emitted as `poly.subdivide.attribute_dropped` evaluation
  warnings visible in the Problems panel.
- A non-zero face `hole` tag returns `poly.subdivide.holes_unsupported`. The pinned OpenSubdiv source marks base
  faces, but does not define the child-face propagation/output contract needed to keep provenance stable.
  `PolyMesh` still has no first-class hole-face representation. Open boundaries are separately supported.
- Extraordinary valence 3/5 refined points match their own parent against an independent one-level
  Catmull-Clark reference. Exact infinite-level limit positions and fractional/multilevel crease-decay
  references remain future numerical coverage.
- Pre-cancellation is tested and the adapter polls between levels; cancellation during the library topology call
  cannot be observed because `RefineUniform` is uninterruptible. Keep preflight/caps as the bound.
- `RefineUniform` has no internal cancellation callback. The largest accepted call has not been isolated in a
  benchmark; keep the two-million face/vertex caps conservative rather than retuning without that evidence.
- The subdivision graph now has cage immutability, save/reload, undo/redo, and grouped two-instance parameter
  history coverage.

Graph quality preflight uses the Catmull-Clark corner recurrence bounded by at most three input corners per
triangle; kernel growth preflight remains exact. The previous measured WASM size baseline predates the graph
node. ADRs 0019/0021 remain Proposed pending owner review.
