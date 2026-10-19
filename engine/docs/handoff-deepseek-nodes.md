# Handoff to DeepSeek: finish the error suggestions in `ve-graph/src/nodes.rs`, and split the file

Read `engine/AGENTS.md` first and follow it (no `unwrap`/`expect`/`panic!`/`println!` in library code, no `#[allow]`, Rust files under 400 lines, docs on public items, tests, no commits, no `Co-Authored-By` or tool
attribution). **Never run `cargo fmt`/`cargo fmt --all`; format only your Rust files with `rustfmt --edition 2024 --config-path rustfmt.toml <file>` from `engine`.** Use the edit tools. Do not build the browser bundle.
Another engineer works in `cpp/`, `crates/ve-kernel*`, `crates/ve-geometry`, `poly_ops/blender.rs` and `builtin/poly_output.rs`: do not touch those. At the end write the Status below **into this file** as well as saying it.

## What you left last time, now finished
You reported nine error sites in `crates/ve-graph/src/nodes.rs` (lines near 47, 110, 250, 257, 286, 296, 327, 334, 374: `param.range` string length, `budget.nodes` for nodes/outputs/materials, `param.range`
for material and position, `graph.unknown_port` twice) left without a suggestion because the file is 395 lines. Do it properly:
1. **Split `nodes.rs` first** into two files by responsibility (for example, node creation and editing in `nodes.rs`, parameter and budget validation in `nodes/checks.rs` or similar), keeping every public path unchanged
   (`pub use` the moved items); change no behaviour. Run `cargo test -p ve-graph -p ve-modeling` after the split before touching any message.
2. **Add the nine suggestions**, short and true, naming the real parameter, limit or operator with the real numbers where the site has them (for `budget.nodes` say the limit and to delete or group nodes; for
   `graph.unknown_port` list the node's real port names, which the site can read from the operator spec; for `param.range` say the allowed range, which the site has).
3. **Tests** in a new `crates/ve-graph/tests/suggestions_nodes.rs` (look at how `crates/ve-modeling/tests/g11_suggestions_more.rs` builds a refusing program and reads `code` and `suggestion`): one test per site you
   changed that provokes it through the public API (a text program or a command batch) and asserts the code, that `suggestion` is `Some`, and that it contains the parameter, limit or port name. A site that cannot be
   provoked from outside must be listed in the Status with the reason, not dropped.

## Acceptance
`cargo test -p ve-graph -p ve-modeling`, `cargo clippy -p ve-graph --tests` shows nothing new from your files, `cargo xtask rules` (no file over 400 lines, including both halves of the split). In the Status give
the before and after count of `Diagnostic::error(` sites without a suggestion in `crates/ve-graph/src/nodes*`.

## Status (DeepSeek: fill in, then stop)
- State: done (uncommitted; the strict gate is pending, and the browser bundle was not built).
  `nodes.rs` was split first with no behaviour change, `cargo test -p ve-graph -p ve-modeling`
  passed on the split before any message was touched, then the nine suggestions were added.

- Split (what moved where):
  - `crates/ve-graph/src/nodes.rs` (289 lines) keeps node creation and editing: `add_node`,
    `remove_node`, `set_param`, `set_label`, `set_flags`, `set_op_version`, with the helpers they
    use (`unknown_node`, `unknown_op`, `spec_of`, `existing`, `text_ok`, `build_params`).
  - `crates/ve-graph/src/nodes/checks.rs` (152 lines, new) holds the edits checked against budget
    and range limits: `set_output`, `define_material`, `remove_material`, `editor_set_position`,
    `editor_set_collapsed`, plus `plain_value_ok` (it uses `super::existing`).
  - Every path is unchanged: `nodes.rs` declares `mod checks;` and
    `pub(crate) use checks::{define_material, editor_set_collapsed, editor_set_position,
    remove_material, set_output};`, so `crate::nodes::*` and `crate::nodes::unknown_node` (imported
    by `links.rs`, `groups.rs`, `groups/instances.rs`) resolve exactly as before. No message, code
    or behaviour changed in the split.
  - Both halves are under 400 lines (`cargo xtask rules` passes).

- Per site (all provoked through the public API by a command batch in the new
  `crates/ve-graph/tests/suggestions_nodes.rs`, 9 tests, all passing):
  1. `nodes.rs` `text_ok` (`param.range`). Test `a_label_over_the_limit_names_the_byte_limit`:
     add a `solid.union@1`, then `set_label` with a 300-byte label. Code `param.range`, suggestion
     `shorten the label to at most 256 bytes`.
  2. `nodes.rs` `add_node` (`budget.nodes`). Test `too_many_nodes_names_the_node_limit`: five
     batches of 1000 `add_node solid.union@1`, then one more (6th batch). Code `budget.nodes`,
     suggestion `delete or group nodes to stay within 5000 nodes`.
  3. `nodes.rs` `set_op_version` (`parse.unknown_op`, from `with_version`). Test
     `a_zero_op_version_names_the_version_floor`: add a node, then `set_op_version` version `0`.
     Code `parse.unknown_op`, suggestion `use a version of at least 1, not 0`.
  4. `nodes.rs` `set_op_version` (`graph.unknown_port`). Test
     `dropping_a_linked_input_port_lists_the_real_ports`: a test registry with `test.thing@1`
     (input `in`) and `test.thing@2` (input `other`); add `test.src@1`, add `test.thing@1`, link
     `in`, then `set_op_version` to 2. Code `graph.unknown_port`, suggestion
     `test.thing@2 inputs: other` (lists the target spec's real port names, or `none` when it has
     none).
  5. `nodes/checks.rs` `set_output` (`graph.unknown_port`). Test
     `an_output_on_an_input_port_names_the_output_port`: add a node, then `set_output` from port
     `in`. Code `graph.unknown_port`, suggestion `use the output port "out"`.
  6. `nodes/checks.rs` `set_output` (`budget.nodes`). Test
     `too_many_outputs_names_the_output_limit`: add a node, then 65 `set_output` with distinct
     names. Code `budget.nodes`, suggestion `remove outputs to stay within 64 named outputs`.
  7. `nodes/checks.rs` `define_material` (`budget.nodes`). Test
     `too_many_materials_names_the_material_limit`: 257 `define_material` batches. Code
     `budget.nodes`, suggestion `remove materials to stay within 256 definitions`.
  8. `nodes/checks.rs` `define_material` (`param.range`). Test
     `an_oversized_material_definition_names_the_entry_limit`: one `define_material` with 65
     entries. Code `param.range`, suggestion
     `use at most 64 entries named 1 to 64 bytes, with finite values`.
  9. `nodes/checks.rs` `editor_set_position` (`param.range`). Test
     `a_position_past_the_editor_extent_names_the_coordinate_limit`: add a node, then
     `editor_set_position` with `x = 2.0e6`. Code `param.range`, suggestion
     `keep each coordinate within ±1000000`.
  Sites that cannot be provoked from outside: none of the nine.

- Before and after count of `Diagnostic::error(` sites without a suggestion in
  `crates/ve-graph/src/nodes*`: **9 before → 0 after**. Measured the same way as the previous
  Status (each `Diagnostic::error(` occurrence, looking up to 16 lines ahead and stopping at the
  next occurrence, counting it as having a suggestion when `.suggest(` appears). Note: under a
  strict per-builder-chain reading, `graph.unknown_node` (`nodes.rs`) is also a `Diagnostic::error`
  with no `.suggest`; the window method counts it as covered because the following `unknown_op`
  body contains `suggest(`. It was not in the nine the handoff listed, so I left it alone; say the
  word and I will give it one too.

- Commands run and results:
  - `cargo test -p ve-graph -p ve-modeling` after the split — passed (no behaviour change).
  - `cargo test -p ve-graph -p ve-modeling --no-fail-fast` after the messages — passed, no FAILED.
    `cargo test -p ve-graph --test suggestions_nodes` is `9 passed; 0 failed`.
  - `cargo clippy -p ve-graph --tests` — nothing new from my files (`nodes.rs`,
    `nodes/checks.rs`, `suggestions_nodes.rs`); the only warnings are the pre-existing
    `too_many_lines` ones in `builtin/curve_tools.rs`, `builtin/modeling/*`, `builtin/poly_network*`
    and `builtin/terrain.rs`.
  - `cargo xtask rules` — passes (no file over 400 lines, including both halves).
  - `rustfmt --edition 2024 --config-path rustfmt.toml --check` on my three files — clean.
  - `cargo xtask quick ve-graph` — the `fmt` stage fails on pre-existing unformatted files
    `crates/ve-measure/tests/{query,symmetry,uv}.rs` (not mine; left untouched). My files pass
    `rustfmt --check`.
  - Not run (owner-only): `cargo xtask check --strict` / `--no-cache`. The browser bundle was not
    built. Nothing was committed.

- Sites left alone, with reasons: none of the nine; the only related text-less site is
  `graph.unknown_node`, explained above.

- Note for the owner: this worktree is shared with other running agents; twice during this task
  `nodes.rs` was restored to HEAD and the untracked `nodes/` directory was deleted out from under
  me (a `git checkout`/`git clean` from another session). I re-applied the files; the final
  working tree has 289 + 152 + 245 lines in the three files and all tests passing.
