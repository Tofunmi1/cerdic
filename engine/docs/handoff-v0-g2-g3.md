# Handoff: what to build next (V0, G2, G3)

Written 2026-10-12 at commit `7f4463e`. Scope: the next three gates only (editor shell, worker, first
language slice). Everything after them is in the plan and is not yours yet. Read this file, then
`docs/plans/track2-modeling.md` sections 2, 3, 5 (gates V0, G2, G3) and 8, and
`docs/modeling/` (`commands.md`, `diagnostics.md`, `limits.md`, `cases.md`, `experiments.md`).

## 1. The product and where you are
An AI-native browser engine for authoring interactive 3D/VR: a trimmed Bevy 0.20 fork in Rust/WASM plus a
C++ geometry kernel (Manifold), WebXR (Quest 3 reference), and a typed node language that an LLM and a
person edit through the same commands. Hackathon deadline 11 Nov 2026 16:30 CEST; the owner works
overtime, so **never cut scope for schedule reasons** (say so if you think something is at risk).

Done and gated: the runtime (render, XR, interaction, desktop viewer with lenses), the document core
(G1a: `ve-graph`, `ve-geometry`), persistence and a page bridge (G1b), experiments X1 (worker) and X6
(React shell), and the logic design (contracts only). Nothing evaluates a document into geometry yet.

## 2. Owner rules you must follow
- **File changes only through the Edit and Write tools.** No shell heredocs, `sed -i` or scripted edits
  (formatters like `cargo fmt` are fine). Prefer the dedicated search and read tools.
- **No GPL or AGPL code** in the engine (principles only). Other licences are not a selection
  criterion (ADR 0005). All geometry and materials come from our own system: no imported or retrieved
  models, no external 3D generators (ADR 0003).
- Nothing is published or deployed without the owner's approval. Keep the project private.
- Unsupported features are ours to build; do not reject an option because a library lacks something.
- Changing a decision needs a new ADR and a note to the owner. New ideas go to the plan's parking lot.
- Report faithfully: failing tests, skipped steps and untested hardware are stated, never hidden.

## 3. How the repo works
Run everything from `engine/`. `cargo xtask check --strict` is the gate (fmt, clippy `-D warnings`,
nextest, wasm clippy, cargo deny, C++ checks, native versus wasm parity, web build, the browser tests,
rules); a skipped step fails under `--strict`. `cargo xtask advisories` is separate. Browser tests run
with `cd web && node tests/<name>.mjs` against `web/dist`, which `cargo xtask web` (release) or
`cargo xtask web dev` builds; the full check leaves a **dev** build (62 MB wasm) in `dist`.

Rules the gate enforces, learned the hard way:
- `.rs` files stay under 400 lines (`cargo fmt` can push a file over: split by responsibility). Functions
  are capped at 80 lines. No `unwrap`, `expect`, `panic` in library code (allowed inside `#[test]`
  functions only: helpers outside them need `#![expect(clippy::unwrap_used, reason = "...")]`). No
  `#[allow]`: use `#[expect(lint, reason = "...")]`, and an expectation that does not fire is an error.
- Pure crates (`ve-core`, `ve-geometry`, `ve-graph`, `ve-kernel`, `ve-interact`, `ve-xr-math`) must not
  depend on Bevy, wgpu, winit or web crates (`xtask rules`). `ve-runtime` and `ve-platform-web` are wasm
  only.
- `Diagnostic` is over clippy's size threshold on 64-bit but not on wasm32, so functions returning a bare
  `Box<Diagnostic>` need the `cfg_attr(target_family = "wasm", expect(clippy::unnecessary_box_returns,
  ...))` pattern already used in `ve-graph` (copy it).
- Integration-test helpers shared between files live in `tests/support/helpers.rs` and are included with
  `#[path = "support/helpers.rs"] mod helpers;` (`mod.rs` is linted). Every helper must be used by every
  file that includes it, or `dead_code` fails the gate.
- Tests are named after their case id (`g1a_09_...`); write the acceptance tests before the code.
- Commit messages carry no `Co-Authored-By` or tool-attribution lines (owner rule). Commit only when the
  strict gate is green.

Gotchas that cost time: the wasm-bindgen glue imports `wasi_snapshot_preview1` several times (replace
**all** of them with the shim, as `xtask/src/web.rs` does) and imports a `snippets/` directory (ship it
next to the glue); `serde_json` must keep `float_roundtrip` or saved documents do not reload bit for
bit; the Edit tool trims trailing whitespace in a replacement, so replace whole lines or use `Write`; never
`pkill` by a broad name such as "chrom" (it can close the owner's own browser); headless Chromium refuses
pointer lock and uses SwiftShader, so timing there is not a performance reference (the baseline machine
is an Apple M1, 16 GB).

## 4. What exists that you build on (all in `engine/`)
- `crates/ve-graph`: `Document` (nodes, links, outputs, materials, editor layer, ID state), 13 typed
  `Command`s with batch-local aliases (`$name`), atomic `CommandBatch` with `base_revision` and a
  transaction key, `Session` (undo, redo, conflicts, idempotency), `OperatorRegistry` with 11 **draft**
  operators (`builtin.rs`), `Diagnostic`, and `store` (two-slot save, checksum, migrations, `Backend`).
  Contracts: `docs/modeling/commands.md`, `diagnostics.md`, `limits.md`.
- `crates/ve-geometry`: attribute domains, `PolyMesh`, adapters to `ve-kernel` that reject or report
  attribute loss.
- `crates/ve-kernel` (+ `-sys`, `cpp/`): safe `Solid` (cube, sphere, cylinder, booleans, transforms,
  queries, `from_mesh`, `to_mesh`, simplify). Manifold 3.5.4 also ships `CrossSection` (with offset),
  `Extrude` (twist, top scale), `Revolve`, `Warp`, `LevelSet`, `Smooth`/`Refine`, `Minkowski`, `Hull`,
  `ExecutionContext` (cancellation): **none of these is exposed through the C ABI yet**. `Compose` is
  deprecated upstream; use `BatchBoolean` with Add.
- `crates/ve-runtime/src/document.rs`: the document host (one `Session`, `localStorage` backend) behind
  `doc_open/reset/state/apply/undo/redo/save/operators` wasm functions; `web/document-api.js` wraps
  them as `__ve.doc` (`send(commands, label)` builds a batch for the current revision);
  `web/docpanel.js` is a small visible panel (`?doc=1` or `?viewer=1`); `web/tests/document.mjs` drives it.
- Experiment spikes (outside the repo, throwaway, never import them): `../spikes/x1-worker` (an
  evaluator-only wasm in a worker, with the measurement scripts) and `../spikes/x6-react-shell` (React
  19 + Vite shell around the real engine). Read them for the working patterns; rewrite, do not promote.
- The root app (`../src`, React 19, three.js, `../shared/commands.ts`) is the old vertical slice. It is
  replaced after V0, not extended.

## 5. Gate V0: editor shell (deliverable and acceptance)
React/TypeScript editor in the root app, hosting the engine canvas, a typed bridge, a panel frame and a
live read-only document view. Decisions already taken (ADR 0007, X6): Bun everywhere; the engine canvas
is created **once outside React** and moved into the pane; the engine boots once per page load behind a
guard; there is no `stop()`, so hide a panel instead of destroying it; a canvas React owns and
re-creates is blank (tested, do not use). Ship the engine as glue, wasm, shim and `snippets/`.

Cases (`docs/modeling/cases.md`): V0-01 to V0-08. In short: the editor loads and renders the gallery
without console errors; a command sent from the UI changes the engine document and the view; resize and
device-pixel-ratio changes follow; focus moves cleanly between a text field and the canvas (release
pointer lock on blur); two editor tabs do not interfere (storage keys are per document name); no leaked
workers or listeners after unload (count them); the **old three.js viewport matches the agreed rubric
before it is removed**; and **changing a Rust command schema field without regenerating makes the TS
build fail**. That last one needs a generated bridge: pick a mechanism (for example `schemars` to JSON
Schema to TS types, or a small generator over the serde types), keep it deterministic, record the choice
in a short ADR, and make `cargo xtask check` regenerate and diff it. Also in V0: move `xtask/src/web.rs`
from npm to Bun (lockfile and test chain together), which G0 left open.

## 6. Gate G2: responsive evaluation
Worker entry (ADR 0006: **two modules from one source**, evaluator-only, no renderer, no threads, no
SharedArrayBuffer), job protocol, bounded queue and cache, staged projection into the scene, per-result
selection map, and a visible status panel (worker state, queue length, last revision, last failure and
recovery). Start with the existing primitives and booleans as the first `gallery` scene entries.
`limits.md` has the starting caps (queue 8, preview job 2 s soft and 10 s hard, final 30 s and 60 s,
cache 128 MiB, GPU upload 8 MiB or 2 ms per frame, 3 restarts per 60 s).

Cases G2-01 to G2-15. Key behaviours: terminate a stuck job and restart (X1 measured 46 to 70 ms, results
identical); a crashed worker is a typed failure and the last committed scene stays; ten edits in 100 ms
end with the latest result and **no independent earlier edit lost** (latest-wins must accumulate
non-conflicting commands, never silently drop them); a late preview never replaces a newer final; stale
results are discarded by revision, request id and worker generation; over-budget graphs are refused
**before** allocating; cancelled kernel objects are never cached; the window loop stays responsive;
100 regenerations leave memory bounded. Still unmeasured from X1: result transfers above 0.1 MB, and the
CPU cost of a worker on the Quest (state it as untested unless the owner runs it). Add cooperative
cancellation to the C ABI (`ExecutionContext`; coverage varies by operation, expose only what you
verify) and keep termination as the fallback. Audit item B02 (worker and cancellation) closes here.

## 7. Gate G3: first language-to-model slice
Rewrite the language spike (`../spikes/geom-lang/lang.mjs`, 215 lines JS, about 50 node kinds; its 53
programs are in `runs/`) against the typed IR and registry: a text parser (one node per line, same IR as
the commands), generated help and provider schemas, deterministic serialisation, diagnostics with source
spans, planning with budget checks, incremental evaluation (change `segments` and only the dependents
re-run: instrument and assert node counts). Expose through the C ABI only what the vase slice needs:
`CrossSection` (polygon, offset, difference), extrude (height, divisions, twist, top scale) and revolve
(segments, angle), with the checked sizes and typed errors the existing calls have, and native versus
wasm parity cases (`cargo xtask parity`). Register the operators as `ready` **only after their
conformance cases pass** (the 11 built-ins are `draft`; nothing evaluates yet).

The frozen acceptance example is in `docs/modeling/commands.md` ("The vase transaction"): two polygon
profiles, two revolves, a difference and a `check.min_wall` constraint node; "widen its opening" is one
batch of two `set_param` commands that keeps every ID, and widening only the outer profile must be
rejected with `constraint.min_wall` naming the nodes. Cases G3-01 to G3-23, including: closed-form
checks (revolve volume `pi (r2^2 - r1^2) h` within 1e-3, twisted extrude volume, offset area), defined
errors for self-intersecting and zero-area profiles, an `ready`-only provider schema, a labelled **mock
provider** that returns bounded typed patches (never executable code; one capped repair attempt; a stale
revision is a conflict; the API key never reaches the browser bundle or logs; connect the existing
`server/adapters.ts` where available), a classification table of the 53 spike programs (supported,
deliberately changed, or deferred, each with a reason; do not import spike code or freeze its bugs),
parser and loader fuzz targets, and a gallery with a vase, a twisted extrusion and an offset plate
reviewed from the fixed views in `docs/modeling/fixtures.md`.

## 8. Known gaps and risks to keep in view
- Operators are all `draft`; history keeps full document snapshots (200 entries or 16 MiB); two tabs do
  not merge edits (last save wins, each save whole); the undo history is not saved across reload; the
  evaluation hash is a non-cryptographic 128-bit digest (fine for caches, swap before using it as a
  security boundary).
- The release wasm is 3.36 MB brotli stripped (baseline 3.24 MB); every gate records its delta (target:
  explain anything over 0.3 MB). `wasm-opt` has not been run.
- Not yet verified on a headset: viewport-scale support, worker CPU cost, hand tracking, pointer lock in
  a real browser, anything about frame time. Mark such claims untested until the owner runs them.
- Case lists from G4 onward are drafts in `cases.md`: expand and review them before starting those
  gates. Experiments X2 to X5 and X7 to X9 have not run; each gates the milestone that names it.

## 9. Process and definition of done
One active gate at a time, in this order: **V0, then G2, then G3** (G2 needs V0's bridge for its status
panel; G3 needs G2). For each: write its case tests first; build; `cargo xtask check --strict` green;
update the plan's progress line (`docs/plans/track2-modeling.md`, "Progress") and `cases.md` /
`experiments.md` / ADRs as decisions land; record measurements (sizes against the 3.36 MB baseline, job
times, restart times); look at the gallery or editor output yourself and say what you saw; commit; report
to the owner what passed, what was measured, what was surprising and what is next. Do not start V1, V2, G4
or any logic gate (L0 to L9) until the owner says so.
