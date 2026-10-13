# 0013: Groups are subgraphs inside the document, expanded before evaluation

- Status: accepted (G4 design; the owner asked for the next plan item to be built, review welcome)
- Date: 2026-10-13

## Context
G4 needs reusable assemblies: a wheel defined once and used four times, one instance changed without
touching the others, a definition edited so every instance follows, an explicit upgrade, and
rejection of recursive groups and of expansion that is too large. The document, commands, history,
text form, outliner and inspector already work on nodes with stable IDs. Operator specs are static
(`&'static str` names), so a per-group operator spec cannot be registered.

## Decision
- A **group** is `{ id (g_...), name, version, output, params, interface }` stored in `Document.groups`.
  Its **members** are ordinary nodes in `Document.nodes` that carry `group: Some(id)`. They are
  authored with the ordinary commands (`add_node` with a `group`, `set_param`, `connect`, `remove_node`),
  keep their stable IDs, and are never evaluated or shown as top-level parts. A member may only read
  other members of the same group.
- The group's **interface** is data: the single `output` member; **exposed parameters**
  `{ name, node, param }` (the instance may override these by name, with the member's own type and
  range); and **inputs**, which are members of operator `group.input@1` (name and kind), bound by
  position to the links of the instance.
- An **instance** is an ordinary node of operator `group.use@1` carrying `instance: { group, version,
  overrides }`. It is a top-level node: it can be connected, transformed, made an output, muted.
  Its output type is the group's output member's type. Overrides are keyed by exposed parameter name.
- **Versions are pinned.** Editing a member's non-interface parameters is a live definition edit that
  every instance sees at once. A change to the interface (exposing or unexposing a parameter, changing
  the output, adding or removing an input) increases `group.version`. An instance whose pinned version
  is behind is refused at evaluation with `group.outdated` naming it, until `upgrade_group` moves it:
  that command keeps the instance ID, keeps overrides that still match an exposed parameter, drops the
  rest and says which. Nothing else in the document changes.
- **Recursion is refused when the edit is made** (`group.recursive`, with the groups in the loop) and
  again when a loaded document is evaluated. Depth is bounded (`MAX_GROUP_DEPTH`).
- **Evaluation expands first.** `expand` turns a document with groups into a flat document without
  them: each instance becomes copies of its members with overrides applied, inputs bound and
  synthetic internal IDs, plus a table from each copy to its instance and member. Everything after
  that (plan, budgets, kernel calls) is unchanged. Diagnostics from copies are reported against the
  instance with the member as `related`. Expanded node count is budgeted before anything runs.
- **Sharing comes from content keys.** Cache keys already ignore node IDs (operator, parameters,
  flags, input keys, quality, seed), so instances with equal effective parameters share every inner
  result. The evaluator now also looks in the current job's staged results, so sharing happens inside
  one job, not only on the next.
- Instance sets, explicit realization, sockets and attachment frames follow in later slices of G4 and
  get their own ADR if they change these rules.

## Consequences
- Nothing about group authoring needs a second editor: the outliner and inspector show members like
  any node, and "edit definition versus edit instance" is which node you select.
- A group's members live in the main node list, so IDs are unique across the document and undo,
  persistence and conflict detection need no new cases.
- The text form must learn `group` blocks and instance overrides before a model can use groups; until
  then `to_text` refuses a document that has groups (it says so) rather than printing something that
  does not read back.
- Expanded size can be much larger than the document, so the budgets count expanded nodes, depth and
  triangles, and refuse with the limit and the observed value.
