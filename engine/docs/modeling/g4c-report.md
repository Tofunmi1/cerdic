# G4c instance sets (2026-10-13)

Status: G4c implemented and strict gate passed. G4d socket interface and attachment-default decisions
were requested from the owner as the handoff requires; no answer has arrived. G4 is not complete.

## Implementation

ADR 0014 introduces a shared `Rc<Value>` prototype and bounded ordered f64 affine placements.
`inst.array`, `inst.radial`, `inst.repeat` compose sets without geometry unions. `inst.mirror` adds
reflected placements. Move/rotate/scale operate on sets; existing unioning patterns stay solid-only.
`inst.realize` explicitly builds placed meshes and uses bounded 64-way kernel unions, with cancellation.
One material belongs to the prototype. Reported set volume sums placement volumes, ignoring overlaps.

Preflight checks every count product and result sum before kernel geometry. Sets/results are capped
at 16,384 placements. Count parameters admit larger integers so an excessive product is a typed
`budget.instances` refusal naming its limit and observed count. Realization has the quality triangle
limit, additional 4,000,000 cap and 256 MiB predicted storage cap. Output draw triangles include
placements, preventing huge raster workloads from bypassing the triangle limit through sharing.
The existing group expansion cap stays 20,000 nodes; the former 50,000 documentation is reconciled.

Reflection batches lazily construct an X-reflected, winding-corrected prototype. Its placement has
the X reflection folded in, so every renderer matrix has a positive determinant. Original logical
instance indices survive this split. No negative-scale renderer exception was introduced.

The worker transfers each prototype once, with transferable f32 matrices and u32 logical indices.
Projection validates and appends at most 512 placements per frame, then uploads prototype chunks.
Committed entities share mesh handles and carry their logical indices. Exact affine placements
override TRS in extraction; picking transforms rays into the prototype box and preserves world
distance, including shear. A viewport click returns `(node, instance)`; panels retain it, show the
instance count and selected index, and individual selection highlights that instance. Selecting a
node still highlights the whole set. Shortened sets invalidate stale index selection.

Status now samples at most 128 authored/interaction entries and exposes actual totals and sample
limits. Repeatedly serializing 10,000 complete entity records would defeat the shared render path.
This is status sampling; rendering and picking still process every placement.

## Representation experiment and observations

`web/tests/instance-probe.mjs` measures a real 10,000-entity runtime scene with one shared box mesh:
120,000 triangles, one mesh asset and one main-pass draw. The initial dev/SwiftShader run observed
5.78 fps, p50 164.7 ms, p95 329.4 ms. This includes software GPU work; no headset/real-GPU timing is
inferred. Retain per-instance entities at the measured scale; 16,384 placements keeps the packed
instance buffer below the 8 MiB upload ceiling. Individual ECS creation and O(N) extraction/picking
remain costs; this does not claim constant-time picking or a 60 fps headset result.

The first actual authoring run measured a 680,240 B result transfer, 12.7 ms worker evaluation,
5,636,096 B worker WASM memory and 1,281,120 B cache. One prototype upload was 864 B with 0.2 ms
observed CPU asset creation. Placement/prototype staging took 2,146.6 ms under SwiftShader. The final
run records runtime memory, reconciled draw counts and per-instance selection in
`target/reports/g4c-instances.json`. Initial report sampled renderer counts before its next frame;
the test now waits for the 120,000-instance triangles to be submitted before reporting.

The final authoring acceptance run recorded 17.4 ms worker evaluation, 110.3 ms transfer delivery,
2,125.7 ms staging and 44,957,696 B runtime WASM memory. Render submission was six main draws and
120,060 triangles: five stage draws plus one shared-instance draw. Selected logical index was 5049.
These timings are one software-rendered development run, not performance guarantees.

## Evidence and validation

- `ve-modeling/tests/g4c_sets.rs`: set nesting, 10,000 placements, group prototypes, radial pattern,
  explicit overlapping realization, winding-corrected reflection, checked products, draw-work and
  realization preflight with zero evaluated operators, warm cache reuse.
- Pure affine-picking test includes shear and invalid matrices.
- `web/tests/instances.mjs`: language/worker/projection with one mesh upload, 10,000 placements,
  real draw counts, central instance picking and retained panel selection. Included in strict gate.
- Gallery fixtures: `instance_fence.txt`, `instance_bookshelf.txt` (200 books), six-view browser list.
- MCP guide teaches shared fence placement and explicit realization.
- Native/WASI corpus includes instance matrices, indices and reflected prototypes.

The installed Playwright version needed its matching Chromium binary; it was installed with Bun.
The first click test released the mouse between slow software-rendered frames and never registered
a press; the test holds it across frames. A real selection refresh bug also cleared instance indices;
panel refresh now preserves unchanged viewport selection.

`cargo xtask check --strict` passed: 360 native tests, WASM lint, dependency checks, C++ sanitizer/tidy,
native/WASI parity (20 kernel lines and three evaluator lines), all browser suites and source rules.
Log: `target/g4c-strict-01.log`. Separate `cargo xtask advisories` passed. All six views of both new
gallery fixtures were visually reviewed: repeated posts/rails and 200 books remain visible, with
expected overlap in exact side views. Browser artifacts are under `target/reports/gallery/`.

Physical headset CPU/GPU/frame timings and controller testing remain untested. Matrix staging is
bounded per frame; creating all committed ECS entities is still one commit operation and is not
claimed to stay below 2 ms. Picking remains linear in the placement count. Per-instance material
overrides and stable individual identities across layout edits remain future work.

## Release build and commits

`bun run build` passed, including schema and TypeScript checks; log:
`target/g4c-release-build3.log`. Size measurements use Brotli quality 9 and remove WASM custom
sections in memory for comparison; the shipped artifacts retain those sections.

| Artifact | Raw bytes | Brotli bytes | Stripped raw bytes | Stripped Brotli bytes |
| --- | ---: | ---: | ---: | ---: |
| Runtime | 34,464,260 | 4,153,657 | 19,533,667 | 3,504,125 |
| Evaluator | 1,241,084 | 345,361 | 1,000,492 | 312,009 |

Compared with the published G3 stripped Brotli baseline, cumulative deltas are +77,256 runtime
bytes and +49,309 evaluator bytes. These include intervening G4 work; an immediate G4b baseline
was not available and the whole delta is not attributed to this slice.

Engine/tests: `4493ca0`. Render, projection and viewer selection: `a164632`. Documentation and
MCP examples are committed separately after validation. The independent rigid transform is
included in the engine commit; sockets and attachment remain unimplemented.

## G4d independent prerequisite

The kernel now has checked proper rigid `Solid::transform([f64;12])`, row-major 3x4 with translation
at entries 3, 7 and 11. Scale/reflection/non-finite matrices are refused; existing scale handles
reflection separately. Native tests cover volume/bounds preservation, inverse round trip and bad
matrices; C++ sanitizer cases and parity are added. This does not complete G4d: named frames,
attachment, align, inspector socket listing and its gallery still remain after the owner decisions.
