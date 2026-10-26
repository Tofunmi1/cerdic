# Handoff: compact text (pipes, loops, compact printer), ADR 0069

Read `docs/adr/0069-compact-text-and-batched-tools.md` first. Crate: `crates/ve-graph` only (text layer under `src/text/`).

## Tests (already written, do not weaken)
`crates/ve-graph/tests/g70_compact_text.rs` (22 cases). They compare documents built from compact text and from long-hand text, node for node.
If a test looks logically wrong, say so in the Status section below instead of changing it silently; the owner or Claude fixes tests.

## What to build
1. **Pipes** `a = f(...) | g(...) | h(...)`: stages feed the previous node into their first input; extra positional inputs follow it; the first stage may be a node name; a line ending in `|` continues; stage names `a_1`, `a_2`, last stage is `a`; taken names give `parse.name_taken` naming the clash; a stage with no input port to take the previous node gives `parse.syntax` mentioning "input".
   Suggested place: `parse.rs` produces `Stmt::Node` for each stage so `build.rs` is unchanged (lexer needs a `Pipe` token and newline handling after it).
2. **Loops** `for i in A..B` / `for i in [list]` / `for [a, b] in [[..]]` ... `end`, expanded BEFORE parsing like the existing `expr::expand` (`let`, `{expression}`): bind the loop variables, expand the body once per item, substitute `{expression}`. Whole numbers inside a name print without decimals; negatives print as `m1`. Nest to depth 3; limit 2,000 statements after expansion (`parse.too_big`, suggestion mentions `inst.array`); `group` inside a body is `parse.syntax`; a missing `end` is `parse.syntax` mentioning `end`. Line numbers of later statements must not move (blank lines for the loop header/`end`; expanded copies keep the body line number so an error span points at the body line) and an error from an expanded body must say `in iteration i=0` (all variables).
   Existing code: `src/text/expr.rs` (+ `expr/eval.rs`) already folds `let` and `{expr}`; extend it or add `src/text/repeat.rs` (files under 400 lines).
3. **`to_text_compact(registry, doc) -> Result<String, Box<Diagnostic>>`**, exported from `ve_graph` (lib.rs) next to `to_text`: no `@n` when the operator's latest version is used; omit parameters equal to the default; angles as degrees when a value of at most six significant digits reads back to the identical f64, else `rad`; a node with exactly one consumer that is the next statement and whose label is `<consumer label>_<k>` prints as a pipe stage (k counts stages from 1, in order). Canonical `to_text` must not change (existing goldens and hashes).
   Place: `src/text/compact.rs`, reusing `print.rs` helpers.

## Rules
- AGENTS.md rules: no unwrap/expect/panic in library code, no `#[allow]`, files under 400 lines, public items documented, a test for every behaviour.
- Do not run `cargo xtask` or `cargo fmt`. Format with `rustfmt --edition 2024 --config-path rustfmt.toml <file>` for the files you touched.
- Use your own `CARGO_TARGET_DIR` (given in the command). Run: `cargo test -p ve-graph` (all existing tests must still pass, including g3_text, g4_text, expr tests and the fuzz target compile `cargo test -p ve-graph --test g3_fuzz`).
- Do not commit or stage anything.
- Add a fuzz-style test that random loop and pipe text never panics (the existing `g3_fuzz.rs` shows how).

## Status
Implemented pipes, loop expansion, and `to_text_compact`, exported beside `to_text`. Pipes lower to ordinary node statements, including continuation lines, existing-node starts, positional input insertion, and generated-name collision diagnostics. Loops support ranges, lists, pair/triple bindings, local constants, name interpolation, three nesting levels, and the 2,000-statement limit. Source maps preserve original lines and iteration bindings for parser, builder, and later application diagnostics. Compact printing reuses the canonical printer, omits latest-version suffixes and defaults, selects degrees only when a literal of at most six significant digits reproduces the exact f64, and folds eligible consecutive single-use chains. Group results/exposes and document outputs protect referenced stage names. Canonical output remains unchanged.

Added `crates/ve-graph/tests/g71_compact_edges.rs`: 16 tests covering boundaries, diagnostics, exact round trips, groups, versions, and a 4,000-case random loop/pipe parser property. Library files touched remain below 400 lines; formatted with direct `rustfmt`, without `cargo fmt` or `cargo xtask`. Nothing committed or staged.

Validation, all with `CARGO_TARGET_DIR=/tmp/codex-target-compact`:
- `cargo test -p ve-graph --no-fail-fast`: **174 passed, 1 failed**, with no filtered tests. All existing expression, g3_text, g4_text, canonical/hash, and g3_fuzz tests pass; all 16 added tests pass.
- `cargo test -p ve-graph --test g3_fuzz`: **5 passed**, including its existing random-text, damaged-program, and loader properties.
- `cargo clippy -p ve-graph --lib --message-format=short`: succeeds; 18 existing warnings in builtin/group code, none in the text layer.
- `git diff --check` passes for the changed tracked text-layer files. The strict gate remains pending, per the handoff's instruction not to run xtask.

**Supplied test issue (left unchanged):** `g70_compact_text.rs` contains 21 tests (the introduction says 22); 20 pass. `a_range_loop_writes_one_statement_per_whole_number` compares `{0.1 + i * 0.05}` at `i=1` against the literal `0.15`, using exact Debug-string document equality. Existing f64 arithmetic gives `0.15000000000000002`, whereas the literal is `0.15`. This expectation is logically inconsistent with the existing expression semantics. The owner can use `{0.1 + 1 * 0.05}` or `0.15000000000000002` in the long-hand fixture. No rounding was added to hide the difference, and the supplied test was not changed or weakened. The added `loops_preserve_the_existing_f64_expression_semantics` test verifies that loop and ordinary expression evaluation agree. This supplied fixture is the sole remaining test failure.
