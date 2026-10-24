# Handoff: `check.attached@1`, a graph node that finds parts floating off their neighbours

Owner: Claude manages; Codex implements. **Crates you may edit, in this order, running that crate's tests before moving on: `ve-graph` (operator spec and the ready list), `ve-modeling` (evaluation).** Do not touch `ve-measure` (finished: `attachments(parts, tolerance)`, tests in `crates/ve-measure/tests/attach.rs`), `ve-bevy`, `ve-runtime`, `ve-evaluator`.
Rules: never run `git checkout`, `git clean`, `git stash` or `git restore`; never `cargo fmt` (use `rustfmt --edition 2024 --config-path rustfmt.toml <file>`); never `cargo xtask`; use exactly `CARGO_TARGET_DIR=/tmp/codex-target-checkatt`; do not commit or stage; edit with your edit tools, not scripts.
`ve-modeling`'s lib test target fails to compile because of a duplicate `mod tests` in `src/anim/drop.rs` and `src/anim/part_2.rs` (not yours, leave it): test with `--test <file>` integration targets there.

## Why

A model builds a creature from separate parts (horns, mane, wings) placed by typed coordinates, and where a guess is off a part floats. `ve-measure::attachments` measures the true gap of each part to its neighbours; the viewer already lists floating parts in Diagnostics. A model writing text needs the same check **inside the language**, so
it reads the result in the tool reply of the evaluation and fixes the placement: `check.attached` is a node it adds to the document, like `check.min_wall` (see `crates/ve-graph/src/builtin/modeling.rs` `constraints()` and `crates/ve-modeling/src/profile_ops.rs` `min_wall` for the pattern: output type `PortType::Check`, diagnostics with `at_node`, `with_related`, `with_limit`, `suggest`).

## What to build

`check.attached@1(parts..., tolerance=0.02, severity="warning")`:
- Inputs: a variadic list of at least two solids (look at how other nodes declare `Arity::Variadic { min: 2 }`, e.g. `crates/ve-graph/src/builtin/fields.rs:314`). Output `PortType::Check`. Add it to the ready list in `builtin/ready.rs` next to `check.min_wall@1`, and to `plan/estimate.rs` (`=> Ok(0)` like `check.min_wall`).
- Params: `tolerance` (length in metres, default 0.02, 0 to 1; doc: "Largest gap between a part and its nearest neighbour that still counts as attached") and `severity` (choice "warning" | "error", default "warning"; doc: "warning lists the floating parts in the evaluation's warnings; error fails the evaluation").
- Evaluation (ve-modeling): turn each input value into a `ve_measure::NamedMesh` in world space (a plain solid's mesh is already world space; an instance set with exactly one copy: apply its transform; skip sets with more than one copy and meshes with fewer than 4 triangles; convert f64 positions to f32), name it by the input node's name as the model wrote it (find how other diagnostics get a node's display name; if
  only ids exist, use the id and put the id in `related`), run `ve_measure::attachments(&parts, tolerance)`, and for each floating part emit a diagnostic with code `attach.floating`, at the floating part's node (`at_node`), `related` = the nearest part's node, `with_limit(tolerance, gap)`, a message such as
  "`horn_b` floats 0.63 m from `body` (nearest part); it should touch within 0.02 m" and a suggestion such as "move the root of `horn_b` 0.61 m toward `body`, or sink it inside `body`; build attached parts with their roots inside the body". With severity "warning" these are `EvaluationWarning`s of the evaluation (see how `value.warnings.push(crate::EvaluationWarning { .. })` is used in `world_scatter.rs` and
  that those reach `Evaluation.warnings`); with "error" the first floating part fails the evaluation as a `Fail`. A `ve-measure` budget error (more than 64 parts or 2,000,000 triangles) must become a clear diagnostic, not a panic. The check node's own value is `Value::nothing()`.

## Tests (written by Claude; never weaken; add more if you like): `crates/ve-modeling/tests/check_attached.rs`

Copy the `evaluate` helper (text -> `Evaluation`) from `tests/scatter_show_range.rs`; each world is text with boxes and spheres (`solid.box@1(size=[1,1,1])`, `xform.move@1(.., by=[x,y,z])`), the check node, and an `output` for every box.
1. `touching_boxes_pass`: a at the origin, b moved by [1,0,0], `chk = check.attached@1(a, b)`: evaluates, no warning with code `attach.floating`.
2. `a_floating_box_warns`: b moved by [2.5,0,0] (gap 1.5): the evaluation succeeds and has one `attach.floating` warning whose message contains `1.5` (and the part's name) and whose related list names the other box's node.
3. `severity_error_fails`: the same with `severity="error"`: `evaluate` returns an error containing `attach.floating`.
4. `tolerance_decides`: the gap 0.4 case warns at the default tolerance and does not at `tolerance=0.5`.
5. `a_part_planted_in_another_passes`: a sphere (radius 1) and a small box centred on its surface, half inside: no warning.
6. `three_parts_name_only_the_floating_one`: a and b touch, c is 0.4 m from b: exactly one warning, about c.
7. `needs_two_inputs`: `check.attached@1(a)` is refused at parse or planning with a typed diagnostic (not a panic).
8. In `ve-graph`: whatever lists or snapshots the operator set (run `cargo test -p ve-graph`; update only the entries for the new operator, never weaken a test).

## Check

`cargo test -p ve-graph` and the new `ve-modeling` test file pass, plus `--test scatter_show_range --test scatter_show_budget --test scatter_show_fade` still pass; `cargo build -p ve-evaluator --target wasm32-unknown-unknown --release` builds.

## Status (Codex: write this section yourself when you stop; your final chat message is not saved)

- [ ] done / [x] blocked: `ve-modeling` could not be compiled because the native `ve-kernel-sys` build exhausted available disk space (`No space left on device`; 300 MiB remained on `/tmp` afterward). The check implementation and integration tests are present, but remain unverified. The required modeling integration tests, scatter regression tests, and wasm evaluator build were not run.
- How input names are found: the evaluator's `Job.labels` map, populated from each authored node's `label`; node IDs are used as fallback names. Floating warnings point to the part node ID and list the nearest part's authored name in `related`; error diagnostics point to the part node ID and relate the nearest node ID.
- Tests run and their output: `CARGO_TARGET_DIR=/tmp/codex-target-checkatt cargo test -p ve-graph` passed (44 unit tests and all integration/doc test targets; 0 failures). `CARGO_TARGET_DIR=/tmp/codex-target-checkatt cargo check -p ve-modeling` failed in the `ve-kernel-sys` CMake build due to `No space left on device` before Rust compilation completed.
- Files changed: `crates/ve-graph/src/builtin/modeling.rs`, `crates/ve-graph/src/builtin/ready.rs`, `crates/ve-modeling/src/plan/estimate.rs`, `crates/ve-modeling/src/operations.rs`, `crates/ve-modeling/src/evaluate.rs`, `crates/ve-modeling/src/profile_ops.rs`, `crates/ve-modeling/src/result.rs`, `crates/ve-modeling/src/hulls.rs`, `crates/ve-modeling/src/hierarchy.rs`, `crates/ve-modeling/src/human.rs`, `crates/ve-modeling/src/budget.rs`, `crates/ve-modeling/src/field_ops.rs`, `crates/ve-modeling/src/rig/part_2.rs`, `crates/ve-modeling/src/physics/part_2.rs`, `crates/ve-modeling/src/poly_ops/uv_report.rs`, `crates/ve-modeling/src/world_scatter.rs`, `crates/ve-modeling/tests/check_attached.rs`, `docs/handoff-codex-check-attached.md`.
