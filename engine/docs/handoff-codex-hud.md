# Handoff to Codex: HUD lines and a procedural pixel font (pure logic)

Read `engine/AGENTS.md` first and follow it (no `unwrap`/`expect`/`panic!`/`println!` in library code, no `#[allow]`: `#[expect(lint, reason)]` on the
smallest item only, files under 400 lines, docs on public items, no commits, no `Co-Authored-By` or tool attribution). **Never run `cargo fmt`; format
only your files with `rustfmt --edition 2024 --config-path rustfmt.toml <file>` from `engine`.** Own `CARGO_TARGET_DIR=target-codex-hud`; delete it when done.
Context: ADR 0062 section 6. A game needs on-screen feedback in VR. The decision (this task is its pure-logic half): the HUD is plain geometry. Text is drawn from a
procedural 5x7 pixel font (no font files), so a panel is a handful of rectangles the runtime turns into a mesh; the text is bound to game variables.
The runtime drawing comes in a later task.

## Part 1: `ve-game` (`crates/ve-game/tests/hud.rs`)
- New statement `hud text="Score {score} / 4" anchor=gaze size=0.05`. The parser must support a double-quoted value containing spaces for `text`
  (and keep working for every existing statement). `anchor` is one of `gaze`, `left`, `right`, `top`, `bottom` (default `gaze`), `size` is the edge of
  one font pixel in metres (default 0.05... see the test for the exact expectation, 0.0 to 1.0 allowed).
- `GameDef.huds: Vec<HudDef>` with public `text: String`, `anchor: String`, `size: f64`; `to_text` prints them back (`GameDef` equality must hold after a round trip).
- `World::hud_lines(&self) -> Vec<String>`: each hud's text with every `{name}` replaced by that variable's current value (whole numbers without a decimal point,
  others with one decimal), in definition order.
- Validation (parse diagnostics with the hud statement's line and a suggestion that lists real variable names): a placeholder for an unknown variable, text longer than
  64 characters, more than 8 hud statements, an unknown anchor. Messages must contain the words the tests look for.
- Snapshots are unaffected (huds are definition data); budgets and all existing tests stay green; clippy `--all-targets` clean.

## Part 2: `ve-world::pixel_text` (`crates/ve-world/tests/pixel_text.rs`)
`pub fn layout(text: &str, pixel: f32) -> Layout` with `Layout { rects: Vec<[f32; 4]>, width: f32, height: f32 }` (`Clone, Debug, PartialEq`): every lit pixel (or merged
horizontal run) of a 5x7 glyph per printable ASCII character 32 to 126 (a standard 5x7 dot-matrix face; author it yourself, do not copy a font file), glyphs
spaced by one pixel, origin at the top-left, rect = `[x, y, w, h]` in metres (y grows downward), `width`/`height` the extent of the text (empty text: 0 and 0).
Characters outside 32..=126 draw `?`. No overlapping rectangles. Deterministic. New file `crates/ve-world/src/pixel_text.rs` (split the font table into its own file
so each stays under 400 lines), `pub mod pixel_text;` in `lib.rs`.

## Part 3: the node (`crates/ve-modeling/tests/g14_game_nodes.rs`, test `hud_nodes_show_variables_and_name_the_node_when_one_is_missing`)
`game.hud@1(text, anchor="gaze", size=0.05)` in `ve-graph/src/builtin/game.rs` and `ve-modeling/src/game_nodes.rs` (a new item section `Hud`; keep the
collect order: vars, kinds, spawns, inputs, rules, goals, huds). A bad hud is refused at `game.collect` with code `game.hud`, naming the node by its label,
with the diagnostic from part 1. Add the operator to the ready list and give it a spec doc and example.

## Contract (written first, do not weaken)
The three test files above. Report a test that looks wrong in Status; do not edit it.

## Verify
```
cd engine
CARGO_TARGET_DIR=target-codex-hud cargo test -p ve-game -p ve-world -p ve-graph -p ve-modeling --no-fail-fast    # only g12_field_warp may fail (known debt)
CARGO_TARGET_DIR=target-codex-hud cargo clippy -p ve-game -p ve-world --all-targets -- -D warnings
CARGO_TARGET_DIR=target-codex-hud cargo check -p ve-game -p ve-world -p ve-modeling -p ve-evaluator --target wasm32-unknown-unknown
```

## Status
Implemented the quoted HUD text parser, HUD definition round-tripping, live variable substitution, validation diagnostics, the procedural 5x7 pixel layout, and `game.hud@1` registration/collection. Added focused helpers to keep changed Rust files within the file-size limit. The existing `hud.rs`, `pixel_text.rs`, and `g14_game_nodes.rs` contract tests were kept intact apart from clippy-only test-expression cleanup.

Checks run with `CARGO_TARGET_DIR=target-codex-hud`:
- `cargo test -p ve-game -p ve-world -p ve-graph -p ve-modeling --no-fail-fast`: all tests passed except the already known `ve-modeling/tests/g12_field_warp.rs` (8 failures, 1 pass). Its expectations appear stale: it expects operators no longer present in the registry and uses syntax/parameter ranges rejected by the current DSL. The HUD and pixel text tests passed.
- Focused rerun after the final file split: `cargo test -p ve-game --test hud -p ve-world --test pixel_text -p ve-modeling --test g14_game_nodes` passed (28 tests).
- `cargo clippy -p ve-game -p ve-world --all-targets -- -D warnings`: passed.
- `cargo check -p ve-game -p ve-world -p ve-modeling -p ve-evaluator --target wasm32-unknown-unknown`: passed; unrelated existing warnings appeared in `ve-geometry` and `ve-modeling`.
- `cargo xtask quick ve-game ve-world ve-graph ve-modeling`: stopped at its workspace-wide fmt check due existing diffs in unrelated engine and vendored files. No workspace-wide formatting was run.
- `cargo xtask rules`: the changed files pass; it still reports pre-existing size violations in `ve-graph/src/builtin/fields.rs`, `ve-graph/src/builtin/world.rs`, and `ve-modeling/src/human_hair.rs`.

The runtime/browser HUD drawing is outside this pure-logic handoff. The strict milestone gate remains pending for the owner.
