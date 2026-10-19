# Handoff: L0 logic contracts, then L1 logic IR and validation

Read `handoff-common.md` first. Design source of truth: `docs/modeling/logic.md` (contracts v1, draft) and
ADR 0008 (logic embedded in the node language). Plan text: `docs/plans/track2-modeling.md` section 5
"L0" and "L1". Prerequisite for L0 is G4 (done). **Nothing evaluates in L0 and L1**; the play runtime is
L2 (which also needs experiments X8 and X9, not run). This file covers L0 and L1 only, plus what you must
decide for L2 so you do not paint it into a corner.

## 1. What must be delivered
**L0** (plan): `PortSpec` gains a kind (`Data` or `Flow`); `OperatorSpec` gains named output ports; the
document gains typed `variables` (initial values); flow links are stored in the same `inputs` map; schema
becomes **2** with a real migration step 1 to 2 (adds empty variables) tested against a frozen schema-1
fixture; `event.*`, `flow.*`, `act.*`, `state.*` operator families reserved as draft. *Accept:* old
documents load and migrate; a flow link into a data port and the reverse are refused with
`graph.port_type`; commands for variables and flow links follow the G1a case style; nothing evaluates.
`cases.md` end-of-file draft adds: an old build opening a schema-2 file reports `store.newer_version`;
variable commands (define, set initial value, remove) are atomic and undoable.

**L1** (plan): event, flow, action and state operators registered with typed payloads; flow validation in
`ve-graph` (acyclic flow, no fan-in without a merge, bounded `repeat`, typed event payloads, variable
types); the text `rule` form compiles to nodes and links and prints back identically; effect class and
param class (live or rebuild) on every action. *Accept:* the door and vending-machine fixtures round-trip
text, graph and commands; **every `logic.*` diagnostic has a failing case**; the parser fuzzes clean.
Diagnostic codes (logic.md section 11): `logic.unknown_event`, `logic.event_args`, `logic.unknown_state`,
`logic.state_type`, `logic.flow_cycle`, `logic.flow_fan_in`, `logic.unreachable_rule`,
`logic.unreachable_objective`, `logic.cascade_depth`, `logic.step_budget`, `logic.queue_overflow`,
`logic.target_missing`, `logic.rebuild_hot`, `logic.write_conflict`, `logic.retrigger` (some belong to
L2/L6 at run time or analysis time: assign each code to the gate that produces it in a table).

## 2. What exists (verified)
- `PortSpec { name, accepts: &[PortType], arity }`, `OutputSpec` (one fixed or "type of an input" output),
  `OperatorSpec { id, doc, inputs, output, params, readiness }` (`ve-graph/src/spec.rs`). No port kind, no
  multiple output ports.
- Links: `Node.inputs: BTreeMap<String, Vec<PortRef>>`; `PortRef` names a node (and output). Link and type
  validation is `links.rs` (the `graph.port_type` error at `links.rs:310`); group rules `group_check.rs`.
- Schema: `SCHEMA = 1` in `doc.rs:17`. Migrations are JSON-level steps in `store/migrate.rs`
  (`Migrator::builtin()` currently holds `schema0_to_1`; a step is `{ from, run: fn(&mut Value) }`).
  `store/envelope.rs` already refuses a newer schema with `store.newer_version`, before the checksum.
  The G1b cases (`ve-graph/tests/g1b.rs`) show how migration and the frozen fixtures are tested; the
  `tests/fixtures/` folder holds frozen documents. **Freeze a schema-1 fixture before you change `SCHEMA`.**
- Text form: `text/{lex,parse,build,node,group,instance,print}.rs`. Statements are `alias = op(args)`;
  groups are blocks; **an instance is a statement whose operator has no dot**. `rule` blocks are new syntax.
- `Document` evaluation hash (`doc.rs` around the `hash` fn) covers nodes, outputs, materials, seed and
  units; labels and the editor layer are excluded. Decide whether `variables` are inside the hash
  (initial values change play behaviour but not geometry): recommended **outside** the geometry
  evaluation hash but inside the saved checksum and history, with a separate "logic hash" for L2.
- Commands are in `command.rs` (about 25 variants); every command needs: apply, undo inverse, history
  label, invariant check, TypeScript export, `commands.md` row, MCP exposure.
- Names for things events target: **there is no `part` operator today.** `logic.md` section 4 writes
  `panel = part(door, name="door", pivot=[-0.5,0,0])`, but a "part" in the code is just a document
  *output* or a node the viewport shows. Frames/sockets (ADR 0015) exist and ride on values. Events and
  `act.animate` need named parts with a pivot. **Decide this before L1** (see 4.3).

## 3. L0 design (proposal; one ADR, 0016/0017 whichever is free)
- `PortKind { Data, Flow }` on `PortSpec`; default `Data`, so every existing operator is unchanged and the
  generated TypeScript only gains a field. Flow ports accept only flow links and vice versa; the check
  is in `links.rs` with `graph.port_type` and a message naming both kinds (G-case: both directions).
- Named outputs: `OperatorSpec.outputs: Vec<OutputPortSpec { name, kind, ty }>` while keeping the current
  single `output` meaning "the default data output" for old operators (do not rewrite 60 operators). A
  `PortRef` gains an optional port name (`#[serde(default)]`, omitted when default), so old links are
  byte-identical.
- Flow links live in `Node.inputs` under the flow input port's name (plan wording): a flow input accepts
  one link (chain) or many only for merge nodes; the validator for that is L1.
- `Document.variables: Vec<Variable { id, name, ty, initial }>` with types `Bool | Number | Text` (check
  logic.md and `limits.md`: 1,024 state variables, names rules like group names). New commands:
  `define_variable`, `set_variable_initial`, `remove_variable` (refused while referenced: define "referenced"
  as a node param naming the variable id; settle the param type for a variable reference, a `Variable`
  `ParamType`, with the dangling case as `logic.unknown_state`). Then `connect` already creates flow links.
- Migration step 1 to 2 adds `variables: []`. Test: frozen schema-1 fixture loads, migrates, equals the
  expected schema-2 document, and re-saving keeps `migrated_from`; an old build (simulate with a
  lower `SCHEMA` through the existing test injection like `Migrator::with_steps`) refuses with
  `store.newer_version`. Coordinate with V2: if V2 adds frames and reroutes to `EditorLayer`, put both in
  this one migration.
- Reserve the families as **draft** operators with specs only (`readiness: Draft`), as G1a did, so
  documents can hold them and commands validate against them, and providers cannot use them yet (the
  provider schema is generated from `Ready`, check `ve-provider`).
- Nothing evaluates: `evaluate.rs` must ignore flow-only nodes cleanly or refuse a document that routes a
  data input from a flow node (the validator already prevents it). Test that a document with logic nodes
  and geometry evaluates the geometry unchanged and the cache key is unchanged for geometry nodes.

## 4. L1 design
### 4.1 Operators with typed payloads
Register `event.*`, `flow.*`, `act.*`, `state.*` and `expr.*` as real specs with output port types for
payloads (`actor`, `entity`, `part`, `socket`: add `PortType`s or a `Payload` family; `PortType` today has
`Solid, Profile, Curve, Sdf, Inst, Check`). Each action carries an **effect class** (`state | emit | host`)
and each writable parameter a **param class** (`live | rebuild`) in the spec (a new enum on `OperatorSpec`
and `ParamSpec`), so the validator and the L6 simulator read them from data. Keep the table in one file per
family (400-line cap) and keep it data-driven, as `docs/modeling/logic.md` section 3 says packs will add
operators later.

### 4.2 Validation (pure, in `ve-graph`, run after every command like `group_check.rs`)
Acyclic flow; no flow fan-in without a merge; `repeat` bounded at 64; event payload fields typed; variable
types match writes; unknown event/state. Distinguish **errors that refuse a command** from **warnings
the L6 analyser reports** (`logic.write_conflict` is a warning; `logic.flow_cycle` refuses). Each code
needs one failing document in `ve-graph/tests/` and an entry in `diagnostics.md`.

### 4.3 Named parts and pivots (decision needed, recommended default)
Recommended: a `part@1` operator (solid in, solid out) that carries a **name and a pivot** on the value
exactly like frames do (ADR 0015: transforms the pivot, mirrors conjugate, patterns prefix copies, groups
carry it). Events then target `part` names and sockets by name, and `act.animate` rotates about the pivot.
Alternative: parts are document outputs; then pivots need another carrier. Choose by ADR, tell the owner,
and add `part.unknown` style diagnostics. This is the dependency L3 (interaction bridge) cannot do without.

### 4.4 The `rule` text form
`rule name: on event.x(...) [if cond] then act.a(...), act.b(...)` is **sugar over nodes and links**:
an event hat, an optional `flow.branch`, and `act.*` nodes chained with flow links. Printing must be
canonical so print(parse(text)) equals the canonical text, as G3 and G4e do for geometry and groups.
Two rules with one event are two hats. State variables are written `@name = state.bool(initial=true)` and
referenced as `@name`: extend `text/lex.rs` and `text/parse.rs` carefully, keep functions under 80 lines
(split like `parse/group.rs`), and fuzz (`ve-graph/tests/g3_fuzz.rs` is the pattern; the plan requires a
clean parser fuzz). Round-trip fixtures to freeze: the **door** (key insert unlocks, select opens, animates)
and the **vending machine** (coin, button, dispense). Write both as text, graph JSON and command batches
and assert all three agree (this is L1's headline acceptance).

### 4.5 Do not forget
Update the MCP guide (`tools/mcp/server.mjs`) only when the operators are `Ready` (the provider must not
see draft operators); update `commands.md`, `diagnostics.md`, `cases.md` (turn the L0 and L1 paragraphs into
tables first), the TypeScript export list, and `src/engine/Editor.tsx` if it builds commands by hand.

## 5. Cases to write first
`cases.md` holds only a paragraph each for L0 and L1. Expand to tables (L0-01...). Minimum for L0:

| ID | Input | Expected |
|---|---|---|
| L0-01 | Load a frozen schema-1 document | Migrates to 2, `variables` empty, geometry hash unchanged, `migrated_from` 1 |
| L0-02 | Open a schema-2 file with a schema-1 build | `store.newer_version`, nothing written |
| L0-03 | Flow link into a data port | `graph.port_type` naming both kinds; nothing applied |
| L0-04 | Data link into a flow port | `graph.port_type`; nothing applied |
| L0-05 | `define_variable`, `set_variable_initial`, `remove_variable` | Atomic, undoable, redoable; removal of a referenced variable refused |
| L0-06 | Property test: random valid command sequences including variables, undo all | Equals the empty document (G1a-19 style) |
| L0-07 | A document with draft logic nodes and geometry | Geometry evaluates identically; no logic runs; draft operators not offered to providers |
| L0-08 | Named output ports | `PortRef` with and without a port name round-trips; old links byte-identical |
| L0-09 | Save, reload, history of logic commands | Equal documents (G1b style) |
| L0-10 | TypeScript bridge | Regenerated types, `bun run typecheck` passes |

## 6. Order of work
1. Freeze the schema-1 fixture (commit it first, before any change).
2. L0 case table, ADR, then `PortKind`, named outputs, `PortRef` port name, `variables`, commands, migration,
   links check, draft families. Strict gate. `l0-report.md`, plan status.
3. Decide 4.3 (part and pivot) with the owner in chat via an ADR draft with the recommended default; do
   not stall on it, start with the rest.
4. L1 operators and specs, validation, diagnostics tests, `rule` text form, door and vending fixtures,
   parser fuzz. Strict gate. `l1-report.md`, plan status.
5. Hand L2 a short note: the budgets (`logic.md` section 5), and X8/X9 still to run.

## 7. Traps
- The 400-line cap hits `command.rs` (already split into `command/refs.rs`), `doc.rs` and `parse.rs`: plan
  the split before adding variants.
- `Document` equality and the history hash include new fields; `#[serde(default, skip_serializing_if)]`
  keeps old files byte-stable, which several G1b tests assert.
- A `PortType` addition changes the exhaustive matches in `links.rs` and the text printer; clippy
  `match_same_arms` will force merging identical arms.
- `ts-rs` exports need the new types in `examples/typescript.rs`; `bun run schema:generate` is part of the gate.
- Logic must never be able to execute arbitrary code (ADR 0002, logic.md section 12): no expression escape
  hatch, no recursion, every loop bounded.
- Hosted-LLM or host-effect calls are queued effects for the page, never made from the engine.

## 8. Done means
L0 and L1 tables pass, strict gate passes after each, frozen fixtures committed, ADRs written, reports and plan
status paragraphs, `diagnostics.md`/`commands.md` updated, no `#[allow]`, no attribution lines.
