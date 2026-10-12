# 0011: The text form is a projection of the commands; providers return bounded patches

- Status: accepted (G3)
- Date: 2026-10-12

## Context
The plan needs a way for a person or a model to write geometry as text, and a way for an assistant
to change a document. The question raised during G3 was whether a text language should be the only
control surface. Research (CAD-Coder and CADCodeVerify on code as a modeling medium; SceneCraft and
LL3M on render-and-critique loops; Aider's edit-format results on how much the edit mechanism
matters) argues for several surfaces over one authority, not for a single one. No independent
benchmark compares a custom language, a restricted code surface and typed tool calls on the same
tasks, so experiment X10 (`experiments.md`) ran one: text, typed commands and a JavaScript builder
gave the same results within noise for Sonnet and Haiku, so no code surface is built.

## Decision
- The typed `CommandBatch` stays the only way to change a document. Text is a projection of it:
  `parse_text` turns text into commands (with source spans), `to_text` prints a document
  canonically, and a document read from either is the same document. Nothing in the text form can
  express anything the commands cannot.
- The form is one node per line, `name = family.op@N(inputs, key=value)`. Inputs are node names,
  positional in port order or `port=name`; lengths are metres with `mm`/`cm` suffixes; angles are
  degrees unless written `rad`, and canonical text prints radians so stored floats round-trip. Only
  `ready` operators are accepted; defaults are omitted from canonical text. The text names nodes, the
  document keeps IDs (labels carry the names).
- A provider returns exactly one `Patch` (summary, base revision, commands) with unknown fields
  refused, at most 200 commands and 256 KiB. The host applies it as an ordinary batch with a key it
  chooses. A conflict or a refusal (including a geometry constraint evaluated before commit) is fed
  back once; the second failure is reported and the document stays as it was. An invalid patch is
  never repaired. Providers see the document and request, never a handle to either.
- Edits by an assistant are typed commands addressed by node ID, as with the mock's "widen its
  opening". Query tools and render critique keyed by node ID are later work. A restricted code
  surface is not planned (X10 found no gain).

## Consequences
Any later surface (node graph, inspector, a model's tool calls, a code-like syntax) lowers to the
same commands, so the evaluator, history, undo and constraints do not change. The text parser has
parse and round-trip properties under fuzzing. Draft operators cannot be written in text until they
pass their conformance cases, which keeps the provider schema honest.
