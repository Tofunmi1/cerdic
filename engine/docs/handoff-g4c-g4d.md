# Handoff: what to build next (G4c instance sets, G4d sockets and attachment)

Written 2026-10-13 at commit `83d33a7`. Scope: the two remaining slices of gate G4 (reuse and assembly).
G4a, G4b and G4e are done; V1 is done on the viewer page. Everything else in the plan is not yours yet.
Read this file, then `docs/plans/track2-modeling.md` (gate G4, section 3 on geometry types and 4 on
evaluation), `docs/modeling/cases.md` (the G4c and G4d tables are your acceptance lists),
`docs/modeling/g4a-report.md`, `g4b-report.md`, `limits.md`, and ADRs 0012 and 0013.

## 1. The product and where you are
An AI-native browser engine for authoring interactive 3D/VR: a trimmed Bevy 0.20 fork in Rust/WASM plus
a C++ geometry kernel (Manifold), WebXR (Quest 3 reference), and a typed node language that an LLM and a
person edit through the same typed commands. Hackathon deadline 11 Nov 2026 16:30 CEST; the owner
works overtime, so **never cut scope for schedule reasons** (say so if you think something is at risk).

Commits that put you here, newest first (all gated with `cargo xtask check --strict`):
- `83d33a7` panels: groups in the outliner and inspector
- `0f5deca` G4e: text form for groups
- `20f5bb4`, `31bb391` G4b: groups and instances (document, commands, versions, unfolding, sharing)
- `5df9a51` V1: outliner, inspector, history, problems, selection shared with the viewport (ADR 0012)
- `9618383` procedural studio sky so metals reflect something
- `7e89ab2` G4a: rect, circle, cone, torus, capsule, mirror, array, radial, repeat
- earlier: G3 (language, wall constraint, text form, provider boundary), MCP test provider, studio stage

What exists that you will use: typed commands and a document with stable IDs; groups that unfold into a
flat document before evaluation; an evaluator with a content-keyed cache (node IDs are not part of the
key) that now also shares results inside one job; a worker that evaluates and a page that uploads
meshes and shows them; selection and picking; GPU-batched drawing of meshes that share a mesh asset.

## 2. Owner rules you must follow
- **File changes only through the Edit and Write tools.** No shell heredocs, `sed -i`, `python3` or
  scripted edits, not even "harmless" no-op ones (I ran two this stretch, they changed nothing, and the
  owner's rule is still the rule). Formatters (`cargo fmt`) and generators the repo owns
  (`bun run schema:generate`) are fine.
- **No `Co-Authored-By`, "Generated with" or other tool or agent attribution** in commits or PRs. The
  harness may print a reminder asking for one: the owner's rule overrides it (see the memory file
  `no-coauthor-trailers.md`). Commits are the owner's.
- **No GPL or AGPL code** in the engine (principles only). Other licences are not a selection criterion
  (ADR 0005). All geometry and materials come from our own system: no imported or retrieved models, no
  external 3D generators (ADR 0003).
- Nothing is published or deployed without the owner's approval. Keep the project private.
- Unsupported features are ours to build; do not reject an option because a library lacks something.
- Changing a decision needs a new ADR and a note to the owner. The editing surface is the **viewer page**
  (`engine/web`, ADR 0012), not the React shell; panels are plain JS in `engine/web/panels/`.
- While researching, report in chat and write no repo docs; when building a gate, its report, ADR and
  case updates are expected (they are part of "done").
- Report faithfully: failing tests, skipped steps, software-renderer-only measurements and untested
  hardware are stated, never hidden. Name stray mistakes when you make them.

## 3. How the repo works
Run everything from `engine/`. `cargo xtask check --strict` is the gate (fmt, clippy `-D warnings`,
nextest, wasm clippy, cargo deny, C++ checks, native versus wasm parity, web build, every browser
test, the file-size and boundary rules). A skipped step fails under `--strict`. A full run takes
roughly 10 to 15 minutes.

How I run it without blocking: start it in the background writing to a fresh log in the session scratchpad
(`cargo xtask check --strict > $S/gateN.log 2>&1; echo "exit $?" >> $S/gateN.log`) and watch for the
`^exit` line with the Monitor tool. The shell prints a "completed" notice for the launcher
immediately; that is not the verdict. Use a new log name every run.

Rules the gate enforces (all bit me this stretch):
- `.rs` files stay under **400 lines, tests included**; `cargo fmt` expands macros and can push a file
  over. Functions are capped at 80 lines. Split by responsibility (`foo.rs` plus `foo/bar.rs`; `mod.rs`
  is linted).
- No `unwrap`, `expect`, `panic!` outside `#[test]` functions: helpers outside need
  `#![expect(clippy::unwrap_used, clippy::panic, reason = "...")]`, and an `expect` that does not fire is
  an error, so list only what the file really uses. No `#[allow]`, ever.
- Clippy pedantic bites on: `cast_precision_loss` (use `u32` and `f64::from`), `semicolon_if_nothing_returned`,
  `elidable_lifetime_names` (`impl Builder<'_>`), `match_same_arms` (merge identical arms),
  `format_push_string` (use `write!`), `assigning_clones` (`clone_into`), `float_cmp` in tests.
- Test helpers shared by several files live in `tests/support/<name>.rs` and are included with
  `#[path = "support/<name>.rs"] mod support;`. Every helper must be used by **every** file that
  includes it, or `dead_code` fails the gate.
- Borrow checker pattern in tests: `commit(&mut s, json!([... named(&s, "x") ...]))` does not compile;
  bind the ID first (`let x = named(&s, "x");`).
- Pure crates (`ve-core`, `ve-geometry`, `ve-graph`, `ve-kernel`, `ve-interact`, `ve-xr-math`) never
  depend on Bevy, wgpu or web crates. `ve-runtime` is wasm only (so its logic is tested in the browser,
  or moved into a pure crate).
- After changing a Rust serde type in `ve-graph`: add it to the export list in
  `crates/ve-graph/examples/typescript.rs`, run `bun run schema:generate` from the repo root, and fix
  `bun run typecheck` (for example `Editor.tsx` builds commands by hand).
- Browser tests run against `web/dist`: rebuild with `cargo xtask web dev` first. The root editor
  tests need `bun scripts/build-engine.ts --copy-only` (the gate does both).
- `fish` is the shell: a glob that matches nothing is an error (`grep --include=*.rs` fails with "no
  matches found"; quote it or use `grep -rn pattern dir | grep '\.rs:'`).
- Write the acceptance tests before the code, name them after their case id (`g4b_12_...`), and
  record each gate's evidence in a report under `docs/modeling/` and a status paragraph in the plan.

Useful commands: `cargo test -p ve-modeling --test g4_eval` (one test file), `bun web/tests/<name>.mjs`
(one browser test), `cargo clippy --workspace --all-targets -- -D warnings`, `cargo clippy -p ve-runtime
--target wasm32-unknown-unknown -- -D warnings`, `bun web/tests/look.mjs <gallery-name> out.png`
(screenshot a gallery program in the studio stage).

## 4. Architecture you build on (verified against the code)
**Document and commands** (`crates/ve-graph`). `Document { nodes, outputs, materials, groups, editor }`;
every edit is a `CommandBatch` applied to a candidate copy, validated, then committed
(`apply.rs`, `session.rs`). `group_check.rs` runs after every command; `links.rs` validates links
and types (`PortType` in `spec.rs` already has `Inst` and `Curve` and `Sdf` that nothing uses yet).
Operator specs are static (`&'static str` names): `builtin/*.rs` register them, `READY` in `builtin.rs`
lists the ones that evaluate. Text form: `text/{lex,parse,build,node,group,instance,print}.rs`.

**Groups** (ADR 0013). Members are ordinary nodes with `group: Some(id)`; an instance is a
`group.use@1` node with `instance: { group, version, overrides }`; `expand.rs` unfolds them into a flat
document before planning (the instance's own ID goes to the copy of the group's result, so parts and
selection keep their identity).

**Evaluation** (`crates/ve-modeling`). `evaluate.rs`: `Evaluator::begin` expands, plans, then `step`
runs one node at a time (the worker yields between steps). `cache.rs`: `Value { mesh, volume, loops,
material }` (a solid's mesh, a profile's loops, or neither); the cache key is
`hash(op, params, flags, input keys, quality, seed)`, so equal nodes anywhere share a result. `plan.rs`:
`estimate()` predicts triangles per operator before any kernel call (a missing arm is
`geometry.unsupported_case`), `validate_outputs` requires every output to be a **solid**.
`operations.rs`: `execute` routes by operator family; `shapes.rs` holds G4a's primitives and
**patterns** (`xform.array/radial/repeat/mirror`), which *union their copies into one solid*.
Material is carried on `Value` and inherited by `xform.*` and booleans from the first input: this is the
pattern to copy for anything that rides along with a solid.
Budgets: `Quality::triangles()` is 200,000 (preview) and 2,000,000 (final); a job's live bytes cap is
256 MiB (`evaluate.rs`); `plan.rs` checks both before running.

**Result to screen.** `Evaluation { parts, evaluated_nodes, cache_hits, cache_bytes }`, a `Part` is one
named output (`node`, `name`, `positions`, `indices`, `volume`, `material`). The worker
(`eval-worker.js`, `ve-evaluator` wasm) posts `MeshPart`s with transferable typed arrays
(`src/engine/worker-types.ts`); `src/engine/mesh-upload.ts` computes smooth normals and sends chunks to
the runtime (`eval_projection_begin / material / chunk / finish`); `ve-runtime/src/projection.rs`
swaps a complete candidate in and spawns **one entity per chunk** with `Mesh3d`, a `VeMaterial`,
`Authored { node, revision }` and `Interactable::selection_only(bounds)`.

**Rendering** (`crates/ve-render`). A forward pass that **batches meshes by mesh asset** with a shared
per-instance buffer (model matrix and material per instance): entities that share a `Mesh3d` handle are
drawn together (`prepare.rs` `InstanceBatches`, `geometry.rs`). `extract.rs` rebuilds the visible mesh
list every frame from the main world (O(entities)) and **skips instances whose determinant is not
positive** (`drawable`): mirrored geometry is refused, never drawn inside out. That is the rule the plan
asks you to keep ("support mirrored geometry through a conversion or change the runtime deliberately;
never an unchecked negative scale").

**Picking and selection.** `ve-runtime/src/interact.rs` picks by ray against each `Interactable`'s
box with a linear scan per frame; a click emits `SelectEnd { click: true }` which
`selection.rs` turns into the selected node (`selection_get` returns `{ node, parts, generation }`).
Hover and selection glow are in `interact/hover.rs`. The page decides which parts a node builds
(`panels/graph.js` `partsFor`).

**Kernel** (`crates/ve-kernel`, C ABI in `ve-kernel-sys` and `cpp/`). `Solid` offers `translate`,
`rotate` (Euler degrees about x, y, z), `scale` (no zeros; a negative factor mirrors), booleans and
`union_all_with_context`; there is **no general matrix transform**. Meshes are f32. Native versus wasm
output is compared by `cargo xtask parity` (a corpus under the kernel tests); add cases there for any
new kernel function.

**Limits**: `docs/modeling/limits.md`. Mismatch to settle: it says "Expanded nodes after groups
50,000" and "Predicted realization 4,000,000 triangles and 256 MiB", while the code uses
`MAX_EXPANDED_NODES = 20_000` (`ve-graph/src/limits.rs`). Either measure and align the doc, or the
constant, and say which in your report.

## 5. G4c: instance sets and explicit realization
**Goal.** A solid (or a group instance) can be repeated many times *without* being unioned: the set
shares one prototype mesh, is drawn with one mesh upload and GPU batching, is picked per instance, and
only an explicit `inst.realize` turns it into one solid, refused by predicted triangles and bytes before
any union runs. Ten thousand cheap instances are valid.

**Acceptance (copy into your tests; from `cases.md`).**
| ID | Expected |
|---|---|
| G4c-01 | `inst.array`, `inst.radial`, `inst.repeat` of a solid or an instance make a set sharing one prototype; nothing is unioned |
| G4c-02 | 10,000 cheap instances (a 12-triangle box) are valid as a set and shown with shared geometry |
| G4c-03 | `inst.realize` of the 10,000 is accepted or refused by predicted triangles and bytes (`budget.triangles` / `budget.memory`) before any union |
| G4c-04 | `inst.realize` of a few instances equals the union of the placed copies |
| G4c-05 | A set with a mirrored transform is supported (winding flipped, positive volume), never an unchecked negative scale |
| G4c-06 | Render and pick 10,000: one mesh upload, many placements, picking returns the instance index |
| G4c-07 | Count products near overflow are refused with the limit, no wraparound |

**Proposed design (write it as ADR 0014 before coding; it changes the "Instances" row of plan section 3).**
1. *Value.* Add `instances: Option<InstanceSet>` to `cache::Value`, with `InstanceSet { prototype:
   Rc<Value> (the solid), transforms: Vec<Transform> }`. A `Transform` is f64 translation, rotation and
   scale composed once (`[f64; 12]` or a 3x4), stored f32 only at the render boundary. The set's
   `mesh` is empty; `bytes()` counts the prototype once plus the transforms.
2. *Operators* (`builtin/instances.rs`, evaluated in `ve-modeling/src/instances.rs`, estimates in
   `plan.rs`): `inst.array@1`, `inst.radial@1`, `inst.repeat@1` take a Solid or an Inst and the same
   parameters as the `xform.*` patterns, output `Inst` (a set of a set composes the transforms:
   cap the product); `inst.realize@1` takes an Inst, outputs a Solid. `PortType::Inst` already exists.
   Add `Inst` to `XFORMABLE` only if you also give `xform.move/rotate/scale` an instance-set behaviour
   (apply to every transform); otherwise leave transforms off sets and say so.
3. *Budgets.* Add `budget.instances` (a cap per set; choose with evidence, 100,000 is a starting guess)
   and refuse before building: the count is a product, so use `checked_mul` and report the limit and
   the observed value, never saturate silently (G4c-07). Realization predicts
   `prototype_triangles x count` against `Quality::triangles()` and the 4,000,000 / 256 MiB line in
   `limits.md`, **in `plan.rs`**, so the refusal happens in preflight with zero kernel calls
   (assert `evaluated_nodes() == 0` as `g4_eval_more.rs` does for expansion).
4. *Outputs.* `validate_outputs` currently requires a Solid. Allow `Inst` outputs. A `Part` for a set
   carries the prototype mesh once plus `transforms: Vec<[f32; 16]>` (column-major); the volume is the
   prototype's times the count (state that it ignores overlaps).
5. *Mirrored transforms (G4c-05).* Recommended: for each set, build the prototype's winding-flipped
   twin lazily (once, in the evaluator) and give every transform with a negative determinant to the
   twin, with the sign folded so the matrix the renderer sees has a positive determinant. The renderer
   rule in `extract.rs` stays as it is. Test: a mirrored set's parts have positive volume and the same
   bounds as `xform.mirror` of the same solid. If you choose something else (for example refusing with
   a diagnostic), record it in the ADR.
6. *Transfer and upload.* `MeshPart` gets `transforms?: Float32Array` (and a flag for the twin); extend
   `mesh-upload.ts` and the `eval_projection_*` exports (one new export for the transforms, or an
   extra field on the chunk) so the prototype mesh crosses once. `projection.rs` then spawns N
   entities that **share one `Mesh3d` handle** (the renderer already batches them) with `Authored`
   plus a new `InstanceIndex(u32)` component. Measure first: Bevy keeps a main-world entity per
   instance, `extract_meshes` walks all of them every frame, and the pick loop is linear. If 10,000
   entities cost too much in the software renderer or in wasm memory, the fallback is one entity with an
   instance-transforms component and a change in `ve-render/extract.rs`. Decide with a number, not a
   guess: write a small spike first and record its result in the report (`web/tests/perf.mjs` and
   `bench-session.mjs` show how frame time is measured here).
7. *Picking and selection.* Per-instance `Interactable::selection_only(prototype bounds)`;
   `selection.rs` stores the instance index next to the node (`selection_get` gains `instance`), the
   outliner keeps selecting the node; the inspector can show "10,000 instances". A click on instance
   k must report `(node, k)` (G4c-06).
8. *Text and MCP.* The new operators have dotted names, so the text form needs no change; add them to the
   guide in `tools/mcp/server.mjs` with a fence example (a row of posts with `inst.array`, then
   `inst.realize` only when a single solid is needed). The studio stage and `profile.smooth` guide
   paragraphs show the house style.

**Tests to write first.** `ve-modeling/tests/g4c_sets.rs` (cases 01, 03 to 05, 07; a `tests/support`
helper if two files need it), a native check that 10,000 instances plan with zero kernel calls and
evaluate quickly, `web/tests/instances.mjs` (cases 02 and 06: one upload, pick returns the index,
selection reaches the panels). Add `inst.*` gallery pieces (a fence, a bookshelf of 200 books) to
`crates/ve-modeling/tests/gallery/` and to the list in `web/tests/gallery.mjs`.
**Evidence to record**: `docs/modeling/g4c-report.md` (what exists, numbers for 10,000 instances in the
software renderer, the ADR, what is not claimed), the plan's status paragraph, `limits.md` updates.

**Unknowns to resolve early.** Whether per-instance entities are fast enough (the spike above);
whether materials may differ per instance (not in G4c: one material per set); how a set of group
instances shares work (it does through the cache: the prototype solid is already evaluated once).

## 6. G4d: named parts, sockets and attachment
**Goal.** Parts can declare named attachment frames (sockets); a part can be attached to another part's
socket so the two frames coincide; moving the parent moves what is attached; `xform.align` rotates one
direction onto another with defined pivot and handedness. Named parts (wheel, body, hub) are already
selectable as named outputs and group members; the new thing is frames and what uses them.

**Acceptance (from `cases.md`).**
| ID | Expected |
|---|---|
| G4d-01 | A socket (named frame with position and orientation) on a member or part, exposed by name through the group, selectable |
| G4d-02 | `xform.attach` of a part's socket to another socket: the part moves so the frames coincide; pivot is the socket; handedness preserved |
| G4d-03 | Moving the parent moves attached parts; unrelated parts do not move |
| G4d-04 | `xform.align` of an axis or normal: defined pivot and result for parallel and anti-parallel inputs |
| G4d-05 | A missing socket or a type mismatch is refused with the socket named |
The wheel assembly fixture (`docs/modeling/fixtures.md`) also asks for "attachment sockets honoured".

**Proposed design (ADR 0015).**
1. *Frames ride on the value, like material.* Add `frames: BTreeMap<String, Frame>` to `cache::Value`,
   with `Frame` a rigid 4x4 (rotation and translation, f64, no scale). Every `xform.*` applies its
   transform to the frames too; booleans keep the first input's frames; `solid.material` keeps them;
   a group instance's output keeps its result member's frames automatically (values flow), so no group
   changes are needed for frames to pass through groups.
2. *Operators.* `frame.add@1(in: Solid, name, position, rotation)`: output the same solid with one more
   named frame (name unique per value; frames are in the solid's own coordinates). `xform.attach@1(in:
   Solid, to: Solid, socket, to_socket)`: output `in` moved by the rigid transform
   `T = F_to * inverse(F_in)` so its `socket` frame coincides with `to`'s `to_socket` frame (an optional
   `flip` for a back-to-back mount is worth considering). `xform.align@1(in, from, to, pivot)`:
   rotation taking direction `from` onto `to` about `pivot` (default the origin); define parallel as
   identity and anti-parallel as a half turn about a deterministic perpendicular (say which, e.g. the
   axis whose component of `from` is smallest) and test both.
3. *Moving the parent moves the child.* Because attach reads the parent's frame *value*, an upstream
   transform of the parent changes the frame and the attach node re-runs; nothing else is needed. Test
   G4d-03 as: change a move upstream of the parent and assert the attached part's bounds move by the
   same offset while an unrelated part's cache entry is a hit.
4. *Kernel.* Add a general rigid transform to the kernel (`Solid::transform([f64; 12])` over a
   Manifold `Transform(mat3x4)`, a C ABI function in `cpp/`, the wrapper in `ve-kernel/src/solid.rs`,
   cases in the parity corpus) instead of composing `rotate` Euler angles, which is lossy and hard to
   test. Keep scale out of it; a mirrored transform stays `scale(-1)`, which Manifold handles.
5. *Mirror and frames.* `xform.mirror` (and the future instance mirror) must reflect the frames:
   a frame `T` becomes `M T M` for the reflection `M`, which is a proper rotation again (determinant
   +1), so handedness is preserved. Test it (the mirror of a socket on the right leg is the socket on
   the left leg with the matching orientation).
6. *Errors.* A missing socket is a runtime `Fail` (frames are known only at evaluation): code
   `socket.unknown`, `node` the attach node, `related` the nodes involved, `suggestion` listing the
   frames the input does have. A non-solid input is already `graph.port_type` at link time.
7. *Showing sockets.* Evaluate frames into the result (`Part.frames`) and show them in the inspector for
   a selected part (name and position); drawing them as gizmos is V3's job, not yours.
8. *Text and MCP.* Dotted names again, so no parser change; extend the guide with a table: legs attached
   to a top by named frames. Add a gallery piece (a table whose legs are attached by sockets, a door
   with a hinge frame) and a `web/tests/gallery.mjs` entry.

**Tests to write first.** `ve-modeling/tests/g4d_frames.rs` (cases 01 to 05, mirror of frames, rigid
transform round trip), a kernel test for `Solid::transform` (volume and bounds preserved, parity
corpus), and a browser check that the inspector lists a part's sockets (`panels.mjs` style).
**Evidence to record**: `docs/modeling/g4d-report.md`, then mark gate **G4 complete** in the plan only
when G4a to G4e all stand (say what "complete" does not include: V3 gizmos, V4 group authoring UI).

**Decisions that are the owner's** (ask, with a recommendation, before the ADR is final): whether a
socket belongs to a group's *interface* (so adding one raises the version and forces upgrades, like an
exposed parameter) or is simply carried by the result value (recommended: carried, no interface
change); and whether attach should rotate as well as translate by default (recommended: yes, both,
because "frames coincide" means position and orientation).

## 7. Order of work
1. G4c: ADR 0014 and `limits.md` reconciliation, spike (10,000 entities), then tests, `Value.instances`,
   operators and budgets, realization, output and transfer, projection and picking, gallery, MCP
   guide, report, gate, commit. Commit after each of: engine and tests, render path, docs.
2. G4d: ADR 0015, kernel transform (with parity) first, then `Value.frames`, `frame.add`, attach and
   align, mirror of frames, errors, inspector listing, gallery, MCP guide, report, gate, commit.
3. Update the plan's status paragraph and `cases.md` after each slice; keep ADRs short and dated.

## 8. Known debt, risks and open questions
- Everything visual was checked on the headless software renderer (SwiftShader, about 9 fps), never on
  a real GPU or a headset. Frame-time claims need real hardware; say so.
- `limits.md` and the code disagree on expanded nodes (see section 4).
- Metal shows a procedural studio sky, not a real environment map: no reflection of the room or the
  model itself (`crates/ve-render/src/shaders/forward.wgsl`).
- Group inputs and results are solids only; profile groups wait for later gates. Group names must be
  plain words (letters, digits, `_`, at most 32), a rule chosen so text round-trips.
- A bypassed group instance passes its first input (by name order) through; a group with several
  inputs has no better rule.
- V4 still owes: making a group from selected nodes in the UI, entering a group with a breadcrumb, a
  library panel, previewing the upgrade conflict for all instances at once.
- V2 (node graph canvas) must be a custom canvas, not `@xyflow/react` (React only), per ADR 0012.
- Open from earlier, unanswered by the owner: the ADR 0010 vase fixture corrections; whether to drop the
  "no GPL in the engine" rule (it stands until the owner says otherwise); the hosted provider
  (`server/adapters.ts`) is not built; 32 of the 53 spike programs wait for later gates
  (`docs/modeling/spike-classification.md`).
- The mock and MCP test providers are for testing only; the real provider boundary is ADR 0011.

## 9. Where things are
| What | Where |
|---|---|
| Document, commands, groups, text form | `crates/ve-graph/src/` (`apply.rs`, `groups.rs`, `groups/instances.rs`, `group_check.rs`, `expand.rs`, `text/`) |
| Operator specs | `crates/ve-graph/src/builtin.rs` and `builtin/{modeling,shapes,group_ops}.rs`; the `READY` list |
| Evaluator, plan, cache, operators | `crates/ve-modeling/src/{evaluate,plan,cache,operations,shapes,profile_ops}.rs` |
| Kernel API and C ABI | `crates/ve-kernel/src/solid.rs`, `crates/ve-kernel-sys`, `cpp/` |
| Meshes to the screen | `src/engine/{evaluation,mesh-upload,worker-types,viewer-bridge}.ts`, `crates/ve-runtime/src/projection.rs`, `crates/ve-render/src/{extract,prepare,geometry,pass}.rs` |
| Selection, picking, hover | `crates/ve-runtime/src/{selection,interact}.rs`, `interact/hover.rs`, `engine/web/panels/` |
| Viewer page and panels | `engine/web/{index.html,viewer.js,docpanel.js}`, `engine/web/panels/{graph,selection,outliner,inspector,groups,widgets,history,dock}.js` |
| MCP test provider | `engine/tools/mcp/server.mjs` (guide text), `src/engine/mcp-bridge.ts`, `docs/modeling/mcp-test-provider.md` |
| Tests | `crates/*/tests/`, `engine/web/tests/*.mjs`; gate order in `engine/xtask/src/web.rs` |
| Gallery programs | `crates/ve-modeling/tests/gallery/*.txt` (volumes in `gallery.rs`, six views in `web/tests/gallery.mjs`) |
| Plan, cases, limits, reports, ADRs | `docs/plans/track2-modeling.md`, `docs/modeling/`, `docs/adr/` |

Try a model through MCP: `bun engine/tools/mcp/try-with-claude.mjs --headed "make a table with four legs"`
(see `docs/modeling/mcp-test-provider.md`); open the viewer at `http://127.0.0.1:8123/?viewer=1&eval=1`
after `cd engine && node tools/serve-lan.mjs --http --host 127.0.0.1 --port 8123`.
