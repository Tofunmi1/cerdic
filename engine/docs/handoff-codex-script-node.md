# Handoff: `script.js@1`, scripts saved in the document

Owner: Claude manages; Codex implements. **Crates you may edit, in this order, running that crate's tests before moving on: `ve-graph` (operator spec, ready list), `ve-modeling` (collection into the evaluation, a new `src/script_nodes.rs`, the `Evaluation` field).** Do not touch `ve-evaluator`, `ve-runtime`, `ve-render`, `ve-bevy`, `src/` (TypeScript) or the web files: Claude wires the page.
Rules: never run `git checkout`, `git clean`, `git stash` or `git restore`; never `cargo fmt` (use `rustfmt --edition 2024 --config-path rustfmt.toml <file>`); never `cargo xtask`; use exactly `CARGO_TARGET_DIR=/tmp/codex-target-scriptnode`; do not commit or stage; edit with your edit tools, not scripts; leave `[lints]` alone and fix clippy findings in your own code. In `ve-modeling` run integration tests with `--test <name>`.

## Why

The owner's goal (2026-10-25): an LLM builds and changes anything, including behaviour. Scripts (JavaScript now, replaced later; ADR 0065) run against the live world, but they only live while the page lives. A world must carry its behaviour: the script is a node in the document, so it is saved, undone, shared and re-run when the world loads. The page keeps the running scripts in step with the evaluation (start new, restart changed, stop removed): that part is Claude's.

## What to build

`script.js@1(source, name, enabled)`: no inputs. Params: `source` string, 1 to 65,536 bytes after the language's string unescaping, not blank (a `param.range` diagnostic that says so); `name` string, optional: when empty the script is named by the node's label (the alias the model wrote, as other nodes' diagnostics name nodes: find how), and when there is no label by the node id; a name must match `^[a-z][a-z0-9_]{0,31}$` (diagnostic code `script.name`, message such as `script name "Bad Name" must be lowercase letters, digits and underscores, starting with a letter, at most 32 characters`) and be unique among the document's scripts (a duplicate is refused naming the repeated name, code `script.name`); `enabled` bool, default true.
- Output type: the one the other side-effect-only nodes use (`PortType::Check`, as `check.motion@1` and `anim.machine@1` do; they are evaluated without being an `output`). Register it in `builtin/ready.rs` and the estimate (`plan/estimate.rs`, `Ok(0)`), operations, like `check.motion@1`.
- `Evaluation.scripts: Vec<ScriptDef>` (a new public field, default empty, `#[serde(default)]`; update every place that builds an `Evaluation`): `ScriptDef { pub name: String, pub source: String, pub enabled: bool, pub node: String }`, public, `Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize`, the fields serialised with exactly those names (the page reads them). Order: the document's node order (the order the model wrote them). The node id in `node` is the authored node's id.
- The node changes no shape and costs no geometry work. **No cache staleness**: a changed `source` must show up in the next evaluation (look at how the evaluator caches node results: the script node's value must depend on its parameters like every other node).
- Operator doc (one paragraph the model reads): what a script node is (behaviour that runs against the live world, saved with the document), the three parameters, that the source is the body of an async function with `world`, `on`, `every`, `log`, `time`, `random` (see the `script` tool's description in `engine/tools/mcp/server.mjs`: read it and keep your text consistent with it), that `\n` and `\"` escapes work in the string, and a two-line example: `bob = script.js@1(source="on('frame', (dt, t) => world.set('crate', 'Transform', {translation: [0, 0.2 * Math.sin(t * 3), 0], rotation: [0, 0, 0, 1], scale: [1, 1, 1]}));")`.

## Tests (written by Claude; never weaken): `crates/ve-modeling/tests/script_node.rs`

10 tests: listed with name, source and node; the name defaults to the label; disabled scripts are listed; order is the written order; escapes become real newlines and quotes; no node, no scripts; no shape changes; bad names (uppercase, leading digit, 33 characters), duplicates, empty, blank and over-long sources are refused with a message that says what to change; a changed source is not served from the cache; the evaluation JSON carries `scripts`. Also run `cargo test -q -p ve-graph` (update only entries for the new operator) and `--test check_motion --test anim_machine`.

## Check

`cargo test -q -p ve-graph`, `cargo test -q -p ve-modeling --test script_node --test check_motion --test anim_machine`, `cargo build -q -p ve-evaluator --target wasm32-unknown-unknown --release`.

## Status (Codex: write this section yourself when you stop; your final chat message is not saved)

- [x] done / [ ] blocked: implemented `script.js@1` as a ready, zero-cost `Check` node; added ordered, validated script collection to `Evaluation` and source-sensitive cached node execution.
- Tests run and their output:
  - `CARGO_TARGET_DIR=/tmp/codex-target-scriptnode cargo test -q -p ve-graph` — passed.
  - `CARGO_TARGET_DIR=/tmp/codex-target-scriptnode cargo test -q -p ve-modeling --test script_node --test check_motion --test anim_machine` — passed (6, 9 and 10 tests).
  - `CARGO_TARGET_DIR=/tmp/codex-target-scriptnode cargo build -q -p ve-evaluator --target wasm32-unknown-unknown --release` — passed.
- Files changed: `crates/ve-graph/src/builtin/modeling.rs`, `crates/ve-graph/src/builtin/ready.rs`, `crates/ve-graph/src/limits.rs`, `crates/ve-modeling/src/evaluate.rs`, `crates/ve-modeling/src/lib.rs`, `crates/ve-modeling/src/operations.rs`, `crates/ve-modeling/src/plan/estimate.rs`, `crates/ve-modeling/src/result.rs`, `crates/ve-modeling/src/script_nodes.rs`.
