# Handoff to Codex: `game.*` nodes (ADR 0062 phase 2, part a)

Read `engine/AGENTS.md` first and follow it (no `unwrap`/`expect`/`panic!`/`println!` in library code, no `#[allow]`: `#[expect(lint, reason)]` on the
smallest item only, files under 400 lines, docs on public items, no commits, no `Co-Authored-By` or tool attribution). **Never run `cargo fmt`; format
only your files with `rustfmt --edition 2024 --config-path rustfmt.toml <file>` from `engine`.** Use your own `CARGO_TARGET_DIR=target-codex-game-nodes`.
Read `docs/adr/0062-*.md` and `docs/plans/game-world-system.md` (7.3) for why. Pattern wiring from ADR 0060 (`field.expr`, `PortType::Pattern`,
`builtin/patterns.rs`, `ve-modeling/src/world_field.rs`) is the closest model of how to add a port type and its operators: copy its shape.

## Task
Make the game a set of nodes in the document. `ve-game` is finished and stays unchanged (if you need a small public addition, say so in Status instead
of editing it, unless it is `Clone`/`Debug`/`PartialEq` derives, which you may add).

1. `ve-graph`: new `PortType::Game`; operators in family `game` (new file `builtin/game.rs`, registered in `builtin.rs` and `ready.rs`), all output `Game`:
   - `game.var@1(name: str, value: number)`
   - `game.kind@1(name, size=0.5, speed=0, tags: list of str)`
   - `game.spawn@1(kind, at=[x,y,z], count=1, spread=0)`
   - `game.input@1(name, axis="", key="", target)` (axis like `x,z`, key like `press`; at most one of them)
   - `game.rule@1(on, a="", b="", when="", every=0, do)` (`do` is the `;`-separated action list of `ve-game`)
   - `game.goal@1(outcome="win"|"lose", when)`
   - `game.collect@1(variadic Game inputs, min 1; name="game")`: joins fragments in link order into one `Game`.
   Every operator has a doc line and an example in its spec like the existing ones; diagnostics follow ADR 0061 (stable id, message names the node).
2. `ve-modeling`: `Value.game: Option<Rc<GameFragment>>` (a fragment is `Vec<Item>` where an item is var/kind/spawn/input/rule/goal plus the node id that
   wrote it); `game.collect` builds the whole definition **through `ve_game::GameDef::parse`-compatible text or direct struct construction**
   (your choice; keep order: vars, kinds, spawns, inputs, rules, goals as linked), runs `GameDef::validate`, and maps every `ve_game::Diagnostic`
   back to the node that wrote the offending item (the model edited nodes, not the internal text). Errors: `game.rule` / `game.kind` / `game.var`
   / `game.goal` / `game.input` for a bad item, `game.duplicate` naming **both** nodes for a repeated var or kind name, `budget.*` through
   `GameError` budgets (64 kinds etc.; check at estimate time too if cheap). `Evaluation` gains `pub game: Option<ve_game::GameDef>` (add `ve-game`
   as a dependency of `ve-modeling`; it is pure Rust). A `Game` value is not geometry: the existing `graph.port_type` machinery must refuse it where geometry
   is expected (give it a message in the style of the pattern one), but `output rules = <game.collect node>` is accepted and produces no part.
   Several collects may be linked into another collect (kits join); the last name wins.
3. Evaluate-time cache and determinism like every other node.

## Contract (written first, do not weaken)
`crates/ve-modeling/tests/g14_game_nodes.rs`. If a test looks wrong, report it in Status; do not edit it. Also keep all existing `ve-graph` and
`ve-modeling` tests green (`g12_field_warp` already fails; ignore it).

## Verify
```
cd engine
CARGO_TARGET_DIR=target-codex-game-nodes cargo test -p ve-graph -p ve-modeling -p ve-game
CARGO_TARGET_DIR=target-codex-game-nodes cargo clippy -p ve-graph -p ve-modeling -p ve-game --all-targets -- -D warnings   # your files must add nothing
CARGO_TARGET_DIR=target-codex-game-nodes cargo check -p ve-modeling --target wasm32-unknown-unknown
```

## Status
Done for the requested phase 2a slice. Added the `Game` port and seven ready `game.*` operators, including string-list tags; wired fragments through cached modeling values; `game.collect` assembles and validates definitions with node-based diagnostics, duplicate-name reporting and kind/rule/entity budgets. `output rules = <game.collect>` returns `Evaluation.game` without creating a geometry part. `ve-game` itself and the contract test were left unchanged.

Verification (using `CARGO_TARGET_DIR=target-codex-game-nodes`): native and wasm `cargo check -p ve-modeling` passed; `cargo check -p ve-modeling --target wasm32-unknown-unknown` passed; `cargo test -p ve-graph -p ve-game` passed (all tests); the requested full test command cannot compile `g14_game_nodes.rs` because line 100 accesses `.status` directly on `World::step`, whose API returns `Result<Outcome, GameError>` (the test needs to handle the `Result`). I did not edit that contract test or `ve-game`, per the handoff. The requested full clippy command is blocked by existing warnings in `ve-human`, `ve-graph`, and other pre-existing workspace code; focused graph clippy passed with the existing `too_many_lines` baseline lint excluded. No new public additions to `ve-game` were needed.
