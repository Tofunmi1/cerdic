# Handoff: what to work on after G5 (written 2026-10-14)

Read `handoff-common.md` first (owner rules, gate mechanics, architecture). This file replaces the "what is
next" part of `handoff-g5-surfaces.md`; that file stays as history for the G5 design.

## 1. Where things stand

**Gate:** `cargo xtask check --strict` passed on the current tree (`target/next-after-g5-strict-03.log`, 458 tests,
parity 20 kernel plus 16 modeling lines identical natively and in WASI, all browser suites on SwiftShader).

**Nothing is committed.** The tree mixes V3 (viewport editing), G5 (curves, sweep, loft, deformers) and the
2026-10-13/14 follow-ups below, about 110 changed or new paths. Per-hunk splitting is not practical; one
combined commit is the owner's call.

**G5 remains incomplete:** a real model has built and edited a pipe
and a vase through the MCP path (`deepseek-v4-flash` via the Claude CLI and AgentRouter, two runs, results
recorded in chat only), but there is no configured provider in the repo. Say "scripted" for the checked-in
test and "two manual runs" for the model evidence. Do not claim more.

**Added since the G5 handoff (all tested, all in the strict gate):**
- `solid.material` takes placement sets (G5-29). `solid.loft` has `steps` 1 to 8 for smooth blends (G5-26).
- **Material regions (ADR 0018, Proposed):** booleans, moves, mirrors and patterns keep each input's colour
  on its own faces via Manifold origin ids. A fused solid is drawn as one part per colour (`Part.region`);
  the renderer keys surfaces by (node, region) (`eval_projection_material_region`,
  `eval_projection_chunk_part`). Files: `ve-modeling/src/regions.rs`, `operations.rs`, `instances/output.rs`,
  `ve-kernel/src/solid/origin.rs`, `cpp/src/kernel.cpp` (`ve_reserve_ids`, `ve_solid_from_mesh_runs`,
  `ve_solid_copy_runs`), `ve-runtime/src/projection.rs`, `src/engine/mesh-upload.ts` (G5-27, G5-28).
- Deformer fold check fixed for slivers and small solids (G5-30); the triangle-overlap check no longer refuses
  dense valid tubes (G5-25).
- Eight scratch programs are now checked in under `crates/ve-modeling/tests/gallery/` and run by the six-view
  SwiftShader gallery. G5-31 covers five chained unions; its planner estimate is input sum plus a capped
  16,384-triangle allowance. This allowance is a heuristic, not a bound for arbitrary boolean arrangements.
- Material-region propagation through twist, taper, bend, `inst.realize` and `xform.attach`/frames remains open.

The ignored `engine/target/reports/big-test/` still contains supplementary renders and original programs. The
programs are now copied into the checked-in gallery; generated images remain ignored. All browser render evidence
uses SwiftShader.

## 2. Decisions the owner still has to make (do not decide for them)
1. Commit the combined tree (and when).
2. Accept ADR 0017 (G5 adapter) and ADR 0018 (material regions, loft blending). Both say Proposed or Draft.
3. Rotate the AgentRouter key that was pasted into a chat; it is not stored anywhere in the repo.
4. Yes or no: **glTF export** (proposed in `handoff-g5-surfaces.md` section 8; needs an ADR).
5. Yes or no: a faster `cargo xtask quick` command (fmt, clippy, native tests only) for inner loops.
6. Open since earlier: the ADR 0010 vase corrections, and the question about reading GPL code for principles.
7. Credit for a model run: `claude-opus-4-8` and `gpt-6-astra` exist on the router but the key's balance
   is spent. With credit, rerun the six prompts (below) with a strong model; budget about 0.25 USD per
   request with the full guide in the prompt.

## 3. Recommended order of work

**The plan's next gate is G6 (polygon tools); X3 is complete.** A, B and D below are complete and
documented. Continue with C if preserving material regions is the immediate priority; otherwise review
ADR 0019 and the G6 operator contracts before starting G6.

### A. Close G5 documentation (completed; maintainer decisions remain)
The report records G5-26 to G5-31, the two handoff-reported manual model runs, fresh release WASM sizes,
limits, and strict evidence. ADR 0017 documents the bounded deformer and hollow cases; its conditional wall
argument and missing provider evidence remain explicit. The ADRs remain Draft/Proposed for maintainer review.
The owner decides ADR acceptance and commits.

### B. Promote the scratch programs into the repo gallery (completed)
The programs are in `engine/target/reports/big-test/programs/*.txt` (`staircase`, `lamp`, `goblet`, `bridge`,
`sword`, `car`, `vase_smooth`, `car_fused`). Move them to `crates/ve-modeling/tests/gallery/`, add them to
the `pieces` table in `web/tests/gallery.mjs` (centre, size, detail point) and to the list in
`tests/gallery.rs`, and the G5-13 cases row. Native gallery evaluation and strict-gate six-view SwiftShader
coverage pass. Keep the staircase camera framing; the studio room is finite and the eye stays within about 5 m.

### C. Carry material regions through the operations that flatten them (medium)
Twist, taper, bend (`deform.rs`), `inst.realize`, `xform.attach` and `frames` keep only the first input's colour.
- Deformers refine by uniform or local midpoint splitting: each source triangle becomes 4, 16 or a local
  split. Track the source triangle of every output triangle and rebuild regions from it (regions are ordered
  triangle runs, `Value.regions`); the kernel validation step afterwards uses `Solid::from_mesh`, so call
  `Labels::solid` there and re-derive regions the same way as booleans.
- `inst.realize` unions copies of one prototype: label with the prototype's regions (same id for all copies,
  as `shapes::pattern` already does).
- Add a "recolour one region" operator only if the owner wants it; today a material on top recolours all.
- Cases to write first: a two-colour union twisted, tapered, bent; a two-colour set realized.

### D. Fix the union triangle estimate (completed)
`plan/estimate.rs` estimates a union as its input sum plus a capped 16,384-triangle intersection allowance.
The five-union regression passes Preview preflight and realizes 12 triangles. This is a planning heuristic,
not a formal bound for every boolean arrangement; the caveat is recorded in ADR 0018 and `g5-report.md`.

### E. Make models better at writing this language (medium, needs credit for the model part)
- Rerun `engine/tools/mcp/try-with-claude.mjs` with a strong model on the six prompts that are in the scratch
  suite: staircase with swept helical handrail, lamp with lofted twisted shade, goblet with revolve and stud
  ring, arch bridge, sword with fuller and wrapped grip, toy car with instanced wheels. Record the result as a
  table in a new `docs/modeling/` report only if the owner asks for repo docs; otherwise report in chat.
- Known engine-side friction the earlier run showed: straight lofts (fixed with `steps`), colour on sets
  (fixed), union compounding (D), the guide text is large (about 0.24 USD per request): consider trimming
  the guide in `tools/mcp/server.mjs` and keeping operator details behind `list_operators`.
- AgentRouter notes: it rejects generic API clients ("unauthorized client detected"); the Claude CLI is
  accepted (`ANTHROPIC_BASE_URL=https://agentrouter.org`, `ANTHROPIC_AUTH_TOKEN=<key from a scratch file
  outside the repo>`). `deepseek-v4-flash` answered; most other model names returned "no available channel".
  Never write the key into the repo, a log or a commit.

### F. Bigger tracks (each has its own handoff)
- **V2/V4 graph editor and cross-document library:** `handoff-v2-v4-graph-editor.md`. This is the strongest
  product story (person and model edit the same graph).
- **Extensibility ("create anything"):** data-driven operators (a group as a first-class operator with a
  declared interface), a pack format and loader, then the cross-document group library. Today a new primitive
  needs Rust (about 11 files; see how `xform.align` and `solid.loft` were added). Scope it as its own
  handoff before building; it needs an ADR.
- **glTF export and materials** (owner yes, ADR): the way models leave the editor.
- **L0/L1 logic:** `handoff-l0-l1-logic.md`.
- **G6 polygon tools (the plan's next gate).** X3 now has a probe report in `docs/modeling/experiments.md`,
  proposed ADR 0019, the operator plans in `docs/plans/g6-polygon-operators.md`, and 18 acceptance rows
  in `docs/modeling/cases.md`. Review ADR 0019 and the contracts before beginning G6; its topology layer,
  operators, schema and renderer work remain unimplemented. G7 subdivision and V5 polygon edit mode wait
  on the G6 gate. G6 is the route to hand-editable organic meshes and, with G7, to things like a rabbit.
- **Per-eye overlays (V6):** curve, gizmo and socket overlays are desktop SVG only today.

## 4. Traps learned in this stretch
- Every evaluation of a boolean can contain zero-area or sliver triangles; never assume a clean mesh from
  Manifold output. Use thresholds relative to the mesh's float resolution, not absolute numbers.
- Pair or cell budgets must count work that was actually done, not candidates in a bucket.
- A kernel run can be empty (coincident copies); always derive a run's end from the next run or the total.
- The runtime keyed surfaces by node; anything that makes several parts per node must set `Part.region`.
- Studio floor is y = 0: anything modelled at negative y is invisible (the sword's grip was).
- The union estimate is a heuristic (input sum plus a capped 16,384-triangle allowance, item D). It fixed the
  compounding, but it can undercount a union of two dense meshes that interleave heavily (a comb pattern),
  so a heavy boolean may start that the old estimate would have refused. The post-evaluation triangle check,
  cancellation and the time budget still apply. Measure that case before relying on the estimate.
- clang-format and clang-tidy gate the C++ (`unchecked operator[]` is an error; use `std::copy_n`,
  `std::transform` on `.data()` ranges like the existing code). Run `clang-format -i --style=file:cpp/.clang-format`.
- Shell-based file edits are forbidden even for a quick test; use the Edit tool.

## 5. How to run things
- Fast loop: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
  then `cargo xtask rules` and `cargo xtask parity`.
- Full gate, in the background with a fresh log, and wait for the real `^exit=` line:
  `cargo xtask check --strict > target/<name>.log 2>&1; echo "exit=$?" >> target/<name>.log`.
- Rebuild the browser bundle before any page test or render: `cargo xtask web dev`.
- Native probe of a text program: copy the loader pattern in `crates/ve-modeling/tests/gallery.rs`
  (`parse_text`, `Session::apply`, `Evaluator::evaluate`).
- Real-model run through the viewer: `bun engine/tools/mcp/try-with-claude.mjs --model <name> "<request>"`.
