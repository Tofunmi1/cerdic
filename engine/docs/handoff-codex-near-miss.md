# Handoff: accept near-misses and answer with the fix (text layer, `crates/ve-graph`)

A model's retries come from small slips. Accept the ones with one obvious meaning, and for the rest say which name was meant. Crate: `crates/ve-graph` only (`src/text/`: `coerce.rs`, `node.rs`, `build.rs`, `parse.rs` areas). Do not touch the MCP server or the page.

## Tests (already written, do not weaken)
`crates/ve-graph/tests/g73_near_miss.rs` (10 cases). If one looks logically wrong, say so in Status; the owner or Claude fixes tests.

## Behaviour
1. **One number for a vector**: where a parameter is a fixed-length list of numbers (`Vec2`, `Vec3`; length 2 or 3), a single number fills every component (`size=2` is `[2, 2, 2]`, `by=50cm` is `[0.5, 0.5, 0.5]`, units apply to the number). Ranges still apply to each component. A list of the wrong length is still an error that says how many components are needed. This is text-layer coercion only (typed commands and canonical text keep full lists).
2. **Wrong-case enum values** are accepted when exactly one choice matches ignoring case (`axis="X"`). An unknown value is an error that lists the choices and names the parameter.
3. **Did you mean**: unknown operator, unknown parameter (`parse.unknown_param` keeps its code and its "takes: ..." suggestion; add "Did you mean `size`?" first when a name is within edit distance 2, or a prefix/substring match), unknown input port, and an unknown node name in an input position all suggest the closest candidates (at most three) in `suggestion`, starting with `Did you mean`. When nothing is close (distance > 2 and no substring match) say no "Did you mean" (the existing suggestion stays). Candidates for node names are the labels defined so far in the text and the document (the `Known` map in `node.rs`), for operators the registry's ready operators (family-qualified: `solid.bx` suggests `solid.box`).
4. **`fix` on diagnostics where unambiguous**: look at how `Diagnostic` carries `fix` (ADR 0061 rule 4 says a ready typed command when one is unambiguous; check `src/diag.rs`). If the field exists, fill it for the misspelled-parameter and misspelled-node cases when there is exactly one candidate within distance 2 (a command or text replacement as the type allows); if the field does not exist, do not add a new public field in this job; say so in Status.
5. Edit-distance helper in one small module (`src/text/near.rs`), unit-tested, no new crates.

## Rules
- AGENTS.md: no unwrap/expect/panic in library code, no `#[allow]` (use `#[expect(lint, reason)]`), public items documented, files under 400 lines. Do not run `cargo xtask` or `cargo fmt` (use `rustfmt --edition 2024 --config-path rustfmt.toml <file>` on touched files). Own `CARGO_TARGET_DIR`. No commits or staging.
- Run `cargo test -p ve-graph` (everything, including g3_text, g4_text, g70/g71 compact text, fuzz) and `cargo test -p ve-evaluator`. If an existing test expects the old strict behaviour (for example a scalar for a vector being an error), list it in Status with the test name instead of weakening it silently.
- Do NOT rebuild the evaluator wasm; Claude does after review.

## Status

Implemented (2026-10-26), confined to `crates/ve-graph` and this Status section:

- Text coercion broadcasts a scalar (including its unit) to `Vec2`/`Vec3`, then checks every component's bounds. Wrong-length lists remain errors naming the required length; typed values remain strict and canonical text prints full lists.
- Choice values accept a unique case-insensitive match and store the canonical spelling. Unknown choices name the parameter and list its options; ambiguous case matches are refused.
- Added `src/text/near.rs` with four unit tests. Operator, parameter, input-port and input-node diagnostics suggest at most three distinct candidates within two edits or matching a prefix/substring, starting with `Did you mean`. Existing `takes:`/group-interface hints remain. Operator candidates are ready, family-qualified names; input-node candidates are preceding text labels and usable document labels. A uniquely misspelled parameter avoids a redundant missing-parameter error taking precedence. Group instance parameters/ports and output references also receive hints.
- Added six regression tests in `g73_near_miss_edges.rs`; left the supplied `g73_near_miss.rs` and all existing tests unchanged.

Validation, all using `CARGO_TARGET_DIR=/tmp/codex-target-nearmiss`:

- `cargo test -p ve-graph`: **199 passed**, including all 10 supplied near-miss cases, g3/g4 text, g70/g71 compact text, the new regressions and the default 4,000-case fuzz properties. No existing test conflicts with the new behavior.
- `cargo test -p ve-evaluator`: **13 passed**; doc-tests also passed for both crates.
- `cargo clippy -p ve-graph --lib -- -D warnings`: blocked by **18 pre-existing errors in untouched files** (17 `too_many_lines` errors in `src/builtin/` and one `match_same_arms` error in `src/groups.rs:117`). No diagnostic points to the near-miss changes. Log: `/tmp/codex-nearmiss-clippy.log`.
- Formatted only touched Rust files with `rustfmt --edition 2024 --config-path rustfmt.toml` (using `skip_children=true` to preserve other files); touched files remain under 400 lines. Whitespace checks passed.

`Diagnostic` in `src/diag.rs` has **no `fix` field**, so no machine-applicable fix or public field was added, as instructed. No `cargo xtask`/`cargo fmt`, staging, commits or evaluator wasm rebuild were performed. The strict gate remains pending; the owner/Claude can review and rebuild wasm.
