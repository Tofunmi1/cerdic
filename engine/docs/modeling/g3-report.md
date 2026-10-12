# G3 evidence (2026-10-12)

Status: G3 implemented; `cargo xtask check --strict` passed (277 native tests, wasm lint, C++ checks,
native/WASI parity, every browser suite including the new `web/tests/gallery.mjs`, repository rules).
No headset was used; nothing here is a hardware result.

## What was built
- **Kernel** (earlier checkpoint `5ae829b`): checked profile, offset, difference, extrude (slabs, twist,
  top scale) and revolve through the C ABI and `ve_kernel::Profile`; numerical corrections in ADR 0010.
- **Operators**: `profile.offset`, `profile.difference`, `solid.extrude` and `check.min_wall` join
  `profile.polygon` and `solid.revolve`. The primitives, booleans and transforms passed analytic cases
  (`ve-modeling/tests/g3_ready.rs`) and are `ready` too, so all 15 shipped operators are ready.
  Transforms accept solids only; they used to list profiles, curves, SDFs and instances in the spec
  while evaluating them to nothing.
- **Evaluator**: node values are meshes, canonical f64 profile loops, or nothing (a check). The wall
  check opens the wall cross-section (outer minus inner) with a disc of the required diameter and
  reports the largest passing thickness by bisection in `observed`. It works on any pair of profiles
  (not only axisymmetric ones); an inner profile that covers the outer one is a wall of thickness 0.
  `profile.offset` takes `join="miter"` (default) or `"round"` (64 segments per full circle), through a
  new `Choice` parameter type.
- **Text form** (`ve-graph::text`, ADR 0011): lexer, parser, command builder, canonical printer.
  `Diagnostic` gained a `span`.
- **Provider boundary** (`ve-provider`, ADR 0011): bounded `Patch`, labelled mock, one repair attempt.
- **Page**: `doc_parse_text`, `doc_text`, `viewer_look_at` (fixed review views); bridge methods
  `sendText` and `text`; TypeScript types regenerated.

## Cases
| Case | Evidence |
|---|---|
| G3-01, 02 | `ve-graph/tests/g3_text.rs`: F1 text reads to the six-node document (labels, radians, metres); printing twice is identical and text, document, text is a fixed point |
| G3-03 to 06 | same file: unknown operator with suggestion and span, missing/unknown/mistyped/out-of-range parameters, wrong port type with its source line, forward references and cycles with names |
| G3-07 to 12 | `ve-modeling/tests/g3.rs`: analytic offset area, twisted volume and bounds, faceted and circular revolve volumes, half revolve, self-intersection and zero area (`kernel.invalid_input`), zero height (`param.range`) |
| G3-13, 14 | same file: widening both profiles keeps all IDs; inner only gives `constraint.min_wall` naming `wall`, `outer`, `inner` with the measured thickness and the document unchanged; outer only passes; a larger minimum fails; on a hollow extruded tube a 5 mm wall measures 5 mm, a 1 mm wall on one side measures 1 mm and a cavity that swallows the outline measures 0 |
| G3-15 to 19 | `ve-provider/tests/g3_provider.rs`: mock vase equals the text document; widening is one batch that keeps IDs; one repair for a stale revision then `revision.conflict`; a constraint refusal is fed back once; code, extra fields, empty and non-JSON replies are `provider.invalid_patch` with nothing applied; no key name in `engine/web`, `src` or `dist` |
| G3-20 | changing `segments` re-runs 2 nodes and hits the cache for 4 |
| G3-21 | `docs/modeling/spike-classification.md`: 12 supported, 9 changed, 32 deferred with reasons; the 21 runnable programs run and match the spike evaluator's volume (1.5 percent) and size (2 mm) |
| G3-22 | `ve-graph/tests/g3_fuzz.rs`: parser on random text and on damaged programs (accepted programs must print and read back identically), loader on damaged and random files; 4,000 cases each in the gate, 500,000 each in a release run (99 s), no failure |
| G3-23 | `web/tests/gallery.mjs`: vase, twisted tower and offset plate go text, staged commands, worker, scene; six fixed views each in `target/reports/gallery/`; reviewed by eye (below) |

Browser sequence (same test): create the vase from text, widen its opening (all six IDs kept), edit
`segments`, undo, save, reload (same nodes, IDs and parameters); an edit that thins the wall to 1 mm
is refused with `constraint.min_wall` and the document is unchanged.

## Review of the gallery views
The vase reads as a flared vase with a visible rim opening and wall ring from above; the twisted
tower shows a continuous five-lobed twist and taper; the offset plate shows the plate, the thin raised
rim and four square holes. Shading is flat (no shadows, one sun), so interior walls are dark. Faces of
the tower show faint banding from 64 flat slabs. The detail view of the plate crops the far corner.

## Measurements (release, Brotli q9, custom sections stripped in memory)
| Module | Stripped Brotli | Change |
|---|---:|---|
| Runtime | 3,426,869 B | +28,109 B over G2 (3,398,760 B) |
| Evaluator | 262,700 B | +44,764 B over G2 (217,936 B) |

## Limits and honest notes
- **Spike coverage**: 32 of 53 programs are deferred because they use mirror, arrays, sweeps, SDFs or
  other operators that belong to later gates; G3 does not claim them.
- **Fuzzing** is property testing, not coverage-guided; 500,000 cases per target found no failure.
- **Provider**: only the offline mock exists; `server/adapters.ts` is not connected. The key search is a
  static scan of browser sources, not proof about a deployed server.
- **Round holes** come from growing a 0.2 mm square by the radius, since there is no circle operator
  yet; the arcs are 64-gons.
- **Wall check**: the opening test uses mitred offsets and a small area tolerance (1 percent of the
  disc area), calibrated on the vase and a tube, not proven for every shape.
- **Text form**: flags (`muted`, `bypass`) are written as parameters; groups, materials and the editor
  layer have no text form yet.
- **Control surfaces**: measured in experiment X10 (`experiments.md`): text, typed commands and a JS
  builder perform alike for Sonnet and Haiku on 16 harder tasks and 8 edits, so no code surface is
  built. One run per cell, strong model near ceiling, no critique loop or query tools tested.
- **Vase fixture**: ADR 0010's corrections (six nodes, inner radius, which edit fails) stand; the
  owner has not yet confirmed them.
- Headset frame time, worker CPU cost on a Quest and headed pointer lock remain untested.
