# Handoff to Codex: `inst.vary`, per-placement variation from patterns

Read `engine/AGENTS.md` first and follow it (no `unwrap`/`expect`/`panic!`/`println!` in library code, no `#[allow]`: `#[expect(lint, reason)]` on the
smallest item only, files under 400 lines, docs on public items, no commits, no `Co-Authored-By` or tool attribution). **Never run `cargo fmt`; format
only your files with `rustfmt --edition 2024 --config-path rustfmt.toml <file>` from `engine`.** Own `CARGO_TARGET_DIR=target-codex-vary`; delete it when done.
Context: ADR 0060. Patterns (`field.expr`, `field.mask_from`, `PortType::Pattern`, `ve-modeling/src/world_field.rs`, `world_scatter.rs` which already takes a
`where` pattern) exist. This task adds the node that lets patterns vary every placement of an instance set: the copy of the language that the audit found missing
("bigger on the right", "turned at random", "raised on the hill").

## Task
New operator `inst.vary@1` (spec in `ve-graph/src/builtin/` next to the patterns or instance specs, wherever a file has room; keep files under 400 lines, split if needed):
- input `in`: an instance set (`PortType::Inst`), required. Optional pattern inputs `scale`, `yaw`, `lift` (`PortType::Pattern`). No parameters. Output `Inst`.
- For every placement, sample each linked pattern at the placement's translation (normal `[0, 1, 0]`); each must give a float (`pattern.not_float` otherwise, naming
  the node, as `scatter.place` does for `where`).
  - `scale`: multiplies the placement's basis (all three axes) about the placement's own origin. Limits 0.01 to 100 (outside: `param.range` with a suggestion).
  - `yaw`: radians, rotates the placement about world up (+Y) *before* its existing rotation is kept: `new = Ry(yaw) * old` on the 3x3 part, translation unchanged. A quarter turn takes +x to -z.
  - `lift`: metres added to the translation's y.
- No pattern linked: the set is returned unchanged. Placement count and prototype are unchanged (the estimate is the input's).
- Wire it like the other instance operators (dispatch in `operations.rs` or a new module, `plan.rs` estimates, `ready.rs`), one doc line plus an example on the spec.

## Contract (written first, do not weaken)
`crates/ve-modeling/tests/g15_vary.rs`. Report a test that looks wrong in Status; do not edit it. Keep every other `ve-graph`/`ve-modeling` test green
(`g12_field_warp` already fails; ignore it).

## Verify
```
cd engine
CARGO_TARGET_DIR=target-codex-vary cargo test -p ve-graph -p ve-modeling --no-fail-fast
CARGO_TARGET_DIR=target-codex-vary cargo clippy -p ve-modeling --all-targets -- -D warnings   # your files must add nothing
CARGO_TARGET_DIR=target-codex-vary cargo check -p ve-modeling --target wasm32-unknown-unknown
```

## Status
Implemented and readied `inst.vary@1` with optional `scale`, `yaw` and `lift` pattern ports. Patterns are sampled at each original placement translation with world-up normal; scale is uniform about each placement origin and checked to `0.01..=100`, yaw is pre-multiplied about world Y while preserving translation, and lift changes only world Y. The operator preserves the prototype and placement count, and its estimate uses the input set. Added the spec example. Left `crates/ve-modeling/tests/g15_vary.rs` unchanged.

Checks run with `CARGO_TARGET_DIR=target-codex-vary`:
- `cargo test -p ve-graph -p ve-modeling --no-fail-fast`: all tests passed except the known `g12_field_warp` target (8 failures); all 9 `g15_vary` contract tests passed. `g12_field_warp` fails on its existing parser/registry fixtures (missing `field.taper`/`twist`/`bend` registration assumptions, parse syntax errors, required `rounding`, and negative `from` values).
- `cargo clippy -p ve-modeling --all-targets -- -D warnings`: blocked by existing workspace warnings promoted to errors, including `many_single_char_names` in `ve-human/src/measure.rs`, `too_many_lines` in existing graph specs (including the pre-existing `variation_ops`), and other graph files. No diagnostic was reported in the new `instances/vary.rs` implementation.
- `cargo check -p ve-modeling --target wasm32-unknown-unknown`: passed, with existing warnings in `ve-geometry` and `ve-modeling`.
- `cargo xtask quick ve-graph ve-modeling`: failed at workspace-wide `cargo fmt --all --check` due to existing formatting diffs across unrelated files; the edited files were formatted individually with `rustfmt` as required.
- `cargo xtask rules`: reports existing over-400-line files `ve-graph/src/builtin/fields.rs`, `ve-graph/src/builtin/world.rs` and `ve-modeling/src/human_hair.rs`; the files added/changed for this task remain under 400 lines.

No contract test appears wrong beyond the known `g12_field_warp` failures described above. The strict milestone gate remains pending.
