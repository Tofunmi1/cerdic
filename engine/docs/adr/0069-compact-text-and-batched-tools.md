# ADR 0069: Compact text (pipes, loops, a compact printer) and batched tools (`batch`, `run`)

- Status: accepted (2026-10-26, owner: make the language and the tool calls much cheaper for a model, implement the known fixes, no measuring first)
- Date: 2026-10-26
- Amends: ADR 0011 (text form), ADR 0061 (model tool surface)

## Context

A model builds a world with one line per node and about twenty tools. A game took 119 to 131 calls. Each call is a round trip that carries the whole conversation, so calls cost more than text. The text itself repeats: a chain such as box, move, array, mirror needs four names and four lines, many near-identical nodes are written out by hand (fourteen pads, six trees), and `get_text` prints angles as seventeen-digit radians.

The language already has constants and arithmetic (`let w = 1.5`, `{w * 2}`, `ve-graph/src/text/expr.rs`), operator versions are optional (`solid.box(...)` is the latest), and groups are the reusable parametric part (`group`, `expose`, `result`). This ADR does not redo those.

Machine-style binary was considered and rejected for the model-facing form: a model produces tokens, a binary form costs more tokens per bit of meaning and the model has never seen it. Inside the engine the document already has a canonical typed form (commands, content hashes); that stays.

## Decision

Every language feature below is expansion in the text layer. It produces the same typed commands as long-hand text, so the evaluator, caches, history and canonical form do not change.

### 1. Pipes

```
post = solid.box(size=[0.08, 1.2, 0.08]) | xform.move(by=[0, 0.6, 0]) | xform.array(count_x=3, spacing=[0.75, 0, 0])
```
- `a | b | c`: each later stage reads the stage before it as its first input; inputs written in a later stage follow that one (`| solid.union(other)` is `union(previous, other)`).
- The first stage may be a call or the name of an existing node (`b = a | xform.move(by=[0, 1, 0])`).
- The last stage takes the statement's name (`post`); the stages before it are named `post_1`, `post_2`, in order. A name that is already taken is an error (`parse.name_taken`) that says which.
- A pipe is one logical line; a line that ends in `|` continues on the next line.

### 2. Loops

```
for i in 0..4
  pad_{i} = terrain.flatten(ground, at=[{i * 20}, 40], radius=5)
end
for [x, z] in [[10, 4], [22, 9], [31, 2]]
  tree_{x} = scatter.one(...)
end
```
- `for NAME in A..B` runs for the whole numbers A up to but not including B; `for NAME in [list]` runs over the items; `for [a, b] in [[..], ..]` takes a pair or triple per item.
- Inside the body, `{expression}` may use the loop variables (it already may use constants), and `{...}` is also allowed inside a node or output name (`pad_{i}`, `{i}` must give a whole number, shown without a decimal point; negative numbers are shown as `m1` for -1 so the name stays valid).
- The body is node statements, `let` and `output` lines; loops nest to depth 3. A body may not hold a `group`.
- Expansion happens before parsing and keeps line numbers: an error inside a loop body names the loop iteration (`in iteration i=2`) and the body line.
- Limit: 2,000 statements after expansion, otherwise `parse.too_big` with the suggestion to use `inst.array` or `inst.radial`, which repeat without extra nodes.

### 3. Compact printer

`to_text_compact(registry, doc)` prints the document for a model to read:
- no `@n` version suffix when the operator's latest version is the one used, and no parameters that equal their default;
- angles in degrees when a degree value (up to six significant digits) reads back to the same number, otherwise `rad`;
- a node used once, directly by the next statement, and named `<consumer>_<k>`, is printed as a pipe stage.

It must read back to the same document: `parse_text(to_text_compact(doc))` equals `parse_text(to_text(doc))`, node for node. The canonical `to_text` is unchanged (hashes, history and goldens keep using it).

### 4. Batched tools (MCP server)

- `batch {steps: [{tool, args}], stop_on_error?, max_chars?}`: runs up to 32 tool calls in one request, in order, and answers with one compact entry per step (`ok` or the error, the step's text cut to `max_chars`, default 3,000, with a `truncated` mark). Only the last step may return a picture. It stops at the first error unless `stop_on_error` is false. `batch` and `run` may not be called inside a batch.
- `run {code}`: the body of an async function with `tools.<name>(args)` for every tool (returns the tool's text, or its JSON when it returned JSON), `log(...)`, and a result: what `return` gives, as JSON cut to 8,000 characters. A `for` loop over `tools.apply_text` and `tools.check` runs inside one call, and only what the code returns enters the model's context. Limits: 120 seconds, 200 tool calls, no network and no file or process access from the code. It is for local, trusted use through the same MCP connection, not a security boundary.
- `get_text {format: "compact"}` (compact is the default; `"canonical"` gives the stored form).
- Tool descriptions get short: the runtime-world methods move out of the `world` description into `get_guide(topic="world_api")`.

### Not in this ADR
Positional parameters (`solid.box([1, 2, 3])`: ambiguous with inputs, little saving), a new binary form for the model, constrained decoding (API models do not offer it), user-defined functions beyond groups.

## Consequences
- A chain of four nodes is one line, many similar nodes are a loop, and a whole build-measure-fix cycle can be one `run` call.
- Documents stored or hashed are unchanged; old text still parses.
- Tests: `ve-graph/tests/g70_compact_text.rs` (pipes, loops, compact round trip, limits, errors) and `tools/batch-run-test.mjs` (the two tools).
- Cost: loop and pipe expansion adds a pre-parse step and error positions inside expansions must map back to the source.
