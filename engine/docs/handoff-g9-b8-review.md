# Handoff: B8 visual review package and the open G9b leftovers

Date: 2026-10-15. Read [`handoff-common.md`](handoff-common.md), [`handoff-g9-remaining.md`](handoff-g9-remaining.md) (section 5 is B8),
[`docs/modeling/g9-report.md`](modeling/g9-report.md) and Proposed [`adr/0023-g9-fidelity-before-ai.md`](adr/0023-g9-fidelity-before-ai.md).
B1, B3, B4 grading, B6 and B7 are implemented; B8 is the first thing to finish because every owner judgement about fidelity depends on it.
Do not commit. Do not invent owner scores.

## State to start from

- Done with evidence: lights and emissive emitters (B1), L1 sky/ground ambient with optional bounce (B3), viewer grading presets (B4, renderer only),
  `profile.fillet`, `solid.round`, `solid.chamfer` (B6), `inst.scatter`, `inst.jitter` and the `desk`/`shelf`/`window` kits (B7).
- Partial: B2 baked per-vertex AO (partial), B5 procedural materials (ten recipes, partial). Deferred with a reason in ADR 0023: bloom and depth of
  field (no clean isolation in the straight-copy present path).
- Captures that already exist under `engine/target/reports/g9/`: `environment-*`, `grading-*`, `chamfer-*`, `kits-*`, study-room and material gallery
  outputs. The new B8 review page links the focused B3/B4/B6 pairs and adds common-runner, same-camera pairs for B1/B5/B7.
- The B6 work regenerated the `engine/xr` pixel goldens (they predated B1/B3). The owner has not confirmed that; list it in the review package.

## Tasks

1. **One review gallery script** (`web/tests/g9-review.mjs`, added to the xtask web list on port `5203`). Models: pistol
   (`crates/ve-modeling/tests/gallery/visual/pistol_prop.txt`), furnished study room
   (`crates/ve-modeling/tests/gallery/visual/study_room.txt`), field creature (`crates/ve-modeling/tests/gallery/field_creature.txt`, render at
   `edge_length=0.026`; 0.02 is refused at Preview), ceramic vase, plus the desk/shelf/window kits and a scatter field.
   Six fixed named camera views per model (front, back, left, right, top, three-quarter) at Preview and Final where Final exists. Use the actual Engine
   Viewer (`?viewer=1&eval=1`), `viewer_look_at`, and wait for `committed.quality === 'final'` as `g9-chamfer.mjs` does. No second mock viewer.
2. **Before/after per B item.** Add a switch for each item that can be turned off without editing the document (grade preset neutral, environment
   off, AO off, rounding off) so the same scene and camera render with and without it. Save pairs, never overwrite.
3. **Machine-readable record** next to the images (`review-manifest.json`): fixture file path and content hash, view name, quality, renderer string
   (SwiftShader), git revision, rendered triangle count, evaluation wall time. Store scripts in the repo; images stay in ignored `target/reports/g9/`.
4. **Comparison sheet** (one HTML or PNG contact sheet per model) with the rubric columns: edge readability, material distinction, lighting and
   contact depth, silhouette, composition. Leave score cells empty. Do not call any image-similarity number an acceptance score.
5. **Fix what the sheet exposes before asking for scores.** Known weak spots to check first: flat single-colour books and plain box kits, AO partial
   on thin parts, emissive surfaces that light nothing unless an emitter is authored, no bloom, and the study room Preview-versus-Final difference.
   Each fix needs a case in `cases.md` and a before/after pair. Finish B5 recipes that the sheet shows are missing (wood grain, fabric, brushed
   metal, painted plaster are the likely ones) only if they are procedural and deterministic; no downloaded textures.
6. **Kits in the guide.** Make `ve_modeling::KITS` visible to a model: list them in `get_guide` and `list_operators` output of the MCP server and the
   provider prompt (name, doc, how to paste the source). Add a test that the guide names every kit.
7. Then ask the owner to score, once, with the whole package ready. Record scores and requested changes in `g9-report.md` (dated entry).

## Implementation status (2026-10-15)

- **Done:** `web/tests/g9-review.mjs` is registered in xtask on port 5203 and uses the real Engine Viewer/evaluator bridge. It creates the five
  fixture pages and 60 viewport-only Preview/Final images (six fixed views each). `review-manifest.json` records source hashes, camera poses,
  committed quality, triangle counts, evaluation wall time, SwiftShader, and revision/dirty state. Scores remain blank. The final direct capture
  run passed (`target/reports/g9/review-capture-final.log`, `exit=0`); `cargo xtask rules` passed. The full uncached strict gate passed earlier,
  before the extra comparison captures were added; those current capture changes have focused validation, not a new strict-gate run.
- **Comparison coverage:** the package includes same-camera B1 authored-light off/on, B5 recipes none/on, and B7 scatter off/on captures with
  source SHA-256 and identical camera pose per pair. Focused B3 environment, B4 grading and B6 chamfer pairs are linked. B2 AO remains unpaired:
  AO is baked into mesh vertex data and there is no runtime AO switch. Add a deliberate comparison control before claiming B2 visual isolation.
- **Visual follow-up still needed before owner scoring:** the books are flat-colour and the kits are simple box-like forms; in the room's
  three-quarter image the near wall hides part of the contents. These are visible in the sheets and should be fixed or explicitly accepted before
  requesting scores. Bloom and depth of field remain deferred in ADR 0023.
- **Owner-only pending:** score the rubric after those review follow-ups and confirm the refreshed engine/XR goldens. SwiftShader is not headset
  evidence; no physical headset or emulator was used here, and Quest cost remains unmeasured.

## Exit criteria

The capture package and common-runner B1/B5/B7 comparisons have focused evidence; B2 isolation, visual follow-ups, owner scores, and XR-golden
confirmation remain open. The full strict gate passed before the latest comparison-only script changes; current capture script and rules checks pass.
See `handoff-common.md` for milestone-gate policy. The package opens from `target/reports/g9/review/index.html`; its images are generated under the
ignored target directory. Never infer Quest performance from SwiftShader.
