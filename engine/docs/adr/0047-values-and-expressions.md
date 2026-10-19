# ADR 0047: Number values, parameter sockets and text expressions

- Status: accepted
- Date: 2026-10-19

## Context

Every parameter of every operator was a typed literal. A model that wanted "a pillar as tall as the stairs", "twice this width" or "half of that" had to compute the number itself and write it out, and nothing could be
derived from another part's size. Blender's geometry nodes solve this with value sockets and fields; we need the same reach for text-authored graphs without the full field system.

## Decision

Two features, deliberately separate, both bounded and deterministic:

1. **Text constants and expressions, folded before the parser runs** (`crates/ve-graph/src/text/expr.rs`). `let name = expression` defines a constant; `{expression}` in any value is replaced by its number. Plain
   arithmetic (`+ - * /`, brackets), a few functions (`min max abs sqrt floor ceil round clamp sin cos radians`) and `pi`, `tau`. Numbers are unitless (a length is metres, an angle degrees, as the parameter reads it); a unit
   suffix goes after the closing brace. `let` lines become blank lines so error line numbers hold. Errors are `parse.expression` with a span and a suggestion. A constant used where a node name goes (`radius=w`) says to
   write `{w}`. This changes no document: the saved text prints the folded numbers.
2. **Number values and parameter sockets in the graph.** A new port type `Number`; operators `value.number`, `value.math` (add, sub, mul, div, min, max, pow, mod) and `value.size` (bounding-box width, height, depth,
   longest, shortest, volume, area of a solid or polygon mesh). A link into a port that is not a declared input but names a number parameter (`radius`) or one component of a vector (`size.y`) is a **socket**
   (`OperatorSpec::socket`). The evaluator fills each socket's number into the node's parameters before it runs (`crates/ve-modeling/src/values.rs`), checks it against the parameter's own range, refuses a fraction for a
   whole-number parameter (`value.not_whole`), and keeps socket links out of the operator's input list so geometry inputs keep their order. The text printer writes socket links by port name and omits the literal of a fully
   linked parameter. A parameter with no default that is filled by a link starts at a valid placeholder so the node can be created.

## Consequences

- One part's size can drive another's (`value.size` into `size.y`), and a model can name a dimension once and reuse it, either as a text constant (cheap, compile time) or as a value node (a real graph link that follows edits).
- Expressions cost nothing at evaluation and add no node; sockets add nodes and are the only way to use a measurement of evaluated geometry.
- A whole vector from one node is not possible yet: only components. A boolean or choice parameter cannot be driven.
- Number nodes make no triangles and cannot be scene outputs. The registry of specs is built once and shared (`OnceLock`) because sockets need a spec lookup per node per evaluation.
- Limits: values are within +/-1e9; `pow` and `div` results must stay finite.

## Alternatives considered

- Blender-style fields (per-element lazy evaluation with attributes and domains): the right long-term shape for "where height > 1", but a large change to the evaluator core. Sockets and expressions cover the common
  cases first; fields remain open.
- Expression strings as parameter values evaluated at run time: would hide dependencies from the graph and need a second evaluator. Folding at parse time and real links for measured values keeps one evaluator.
- A scripting language: refused by the project's rule that providers return typed commands, never code.

## Evidence

`crates/ve-graph/src/text/expr/tests.rs` (10 unit tests), `crates/ve-modeling/tests/g11_expressions.rs` (6 end-to-end tests), `crates/ve-modeling/tests/g11_values.rs` (12 tests with exact volumes, every measure, the
whole-number and range refusals, bad links rejected, and the text round trip). A scene using both, plus surface scatter and `poly.round_edges`, was evaluated in a browser through the MCP server and every part measured watertight.
