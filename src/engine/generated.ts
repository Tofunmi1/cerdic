// Generated from ve-graph serde types. Run bun run schema:generate.
export type Alias = string;
export type Command = { "add_node": { 
/**
 * Operator and version, `family.name@N`.
 */
op: string, 
/**
 * Parameter values; missing ones take their defaults.
 */
params: { [key in string]: Value }, 
/**
 * Display name.
 */
label: string | null, 
/**
 * A name later commands in this batch can use for the new node.
 */
alias: Alias | null, } } | { "remove_node": { 
/**
 * The node.
 */
node: NodeRef, 
/**
 * Also remove everything that reads it (and outputs that name it).
 */
cascade: boolean, } } | { "connect": { 
/**
 * Source.
 */
from: OutRef, 
/**
 * Destination.
 */
to: InputRef, } } | { "disconnect": { 
/**
 * The input to clear.
 */
to: InputRef, } } | { "set_param": { 
/**
 * The node.
 */
node: NodeRef, 
/**
 * The parameter.
 */
param: string, 
/**
 * The new value.
 */
value: Value, } } | { "set_label": { 
/**
 * The node.
 */
node: NodeRef, 
/**
 * The name, or `None` to clear.
 */
label: string | null, } } | { "set_flags": { 
/**
 * The node.
 */
node: NodeRef, 
/**
 * New mute state, if changing.
 */
muted: boolean | null, 
/**
 * New bypass state, if changing.
 */
bypass: boolean | null, } } | { "set_op_version": { 
/**
 * The node.
 */
node: NodeRef, 
/**
 * The version to use.
 */
version: number, } } | { "set_output": { 
/**
 * The output name.
 */
name: string, 
/**
 * The source, or `None` to remove the output.
 */
from: OutRef | null, } } | { "define_material": { 
/**
 * Named values.
 */
def: { [key in string]: Value }, } } | { "remove_material": { 
/**
 * The material.
 */
id: MaterialId, } } | { "editor_set_position": { 
/**
 * The node.
 */
node: NodeRef, 
/**
 * Horizontal position.
 */
x: number, 
/**
 * Vertical position.
 */
y: number, } } | { "editor_set_collapsed": { 
/**
 * The node.
 */
node: NodeRef, 
/**
 * Whether it is drawn collapsed.
 */
collapsed: boolean, } };
export type CommandBatch = { 
/**
 * The document this batch is for.
 */
doc: DocId, 
/**
 * The revision the author saw.
 */
base_revision: number, 
/**
 * Idempotency key: a repeated key applies nothing a second time.
 */
transaction: TransactionId, 
/**
 * What the batch does, for the history list.
 */
label: string | null, 
/**
 * The edits, in order.
 */
commands: Array<Command>, };
export type Diagnostic = { 
/**
 * Stable dotted code, for example `graph.cycle`.
 */
code: string, 
/**
 * How serious it is.
 */
severity: Severity, 
/**
 * What happened, in words (may change between versions; match on `code`).
 */
message: string, 
/**
 * Position of the command in its batch, when the problem came from one.
 */
command: number | null, 
/**
 * The smallest responsible node.
 */
node: NodeId | null, 
/**
 * The port involved.
 */
port: string | null, 
/**
 * The parameter involved.
 */
param: string | null, 
/**
 * Other nodes that took part (the members of a cycle, the source of a bad link).
 */
related?: Array<NodeId>, 
/**
 * The limit that was hit.
 */
limit: number | null, 
/**
 * The value that hit it.
 */
observed: number | null, 
/**
 * What to try instead.
 */
suggestion: string | null, };
export type DocId = string;
export type Document = { 
/**
 * Schema version of this data.
 */
schema: number, 
/**
 * Document identity.
 */
id: DocId, 
/**
 * Counts commits, undo and redo; the base of the conflict check.
 */
revision: number, 
/**
 * Always `m`; present so files say what their numbers mean.
 */
units: string, 
/**
 * Seed for anything random.
 */
seed: number, 
/**
 * ID issuing state.
 */
ids: IdState, 
/**
 * Nodes in creation order.
 */
nodes: Array<Node>, 
/**
 * Named results.
 */
outputs: Array<Output>, 
/**
 * Material definitions.
 */
materials: Array<Material>, 
/**
 * Editor layer.
 */
editor: EditorLayer, };
export type EditorLayer = { 
/**
 * Node positions `[x, y]`.
 */
positions: { [key in NodeId]: [number, number] }, 
/**
 * Nodes drawn collapsed.
 */
collapsed: Array<NodeId>, };
export type Flags = { 
/**
 * Skip the node (its output is empty).
 */
muted: boolean, 
/**
 * Pass the first input through unchanged where the types allow.
 */
bypass: boolean, };
export type IdState = { 
/**
 * The next value to encode.
 */
next: number, 
/**
 * Offset that differs between documents.
 */
salt: number, };
export type InputRef = { 
/**
 * The consuming node.
 */
node: NodeRef, 
/**
 * Its input port.
 */
port: string, 
/**
 * Position in a variadic port; `None` appends on connect and removes the only link on
 * disconnect of a single-link port.
 */
index?: number | null, };
export type Material = { 
/**
 * Stable identity.
 */
id: MaterialId, 
/**
 * Named values.
 */
def: { [key in string]: Value }, };
export type MaterialId = string;
export type Node = { 
/**
 * Stable identity.
 */
id: NodeId, 
/**
 * The operator and version.
 */
op: OpId, 
/**
 * A display name; never part of identity or evaluation.
 */
label?: string | null, 
/**
 * Parameter values in canonical form.
 */
params: { [key in string]: Value }, 
/**
 * Links by input port name.
 */
inputs: { [key in string]: Array<PortRef> }, 
/**
 * Mute and bypass.
 */
flags: Flags, };
export type NodeId = string;
export type NodeRef = string;
export type OpId = string;
export type OutRef = { 
/**
 * The producing node.
 */
node: NodeRef, 
/**
 * Its output port (always `out` for now).
 */
port: string, };
export type Outcome = { "committed": { 
/**
 * The new revision.
 */
revision: number, 
/**
 * The batch's key.
 */
transaction: TransactionId, 
/**
 * Nodes added, removed or changed.
 */
changed: Array<NodeId>, } } | { "rejected": { 
/**
 * Why.
 */
diagnostics: Array<Diagnostic>, } } | { "conflict": { 
/**
 * The revision the batch was based on.
 */
base_revision: number, 
/**
 * The current revision.
 */
current_revision: number, 
/**
 * Nodes changed since then, when the history still covers it.
 */
changed_since: Array<NodeId>, 
/**
 * Whether `changed_since` is complete (false when older history was dropped).
 */
complete: boolean, } } | { "already_applied": { 
/**
 * The revision the first application produced.
 */
revision: number, } };
export type Output = { 
/**
 * The output's name.
 */
name: string, 
/**
 * The node that provides it.
 */
from: PortRef, };
export type PortRef = { 
/**
 * The node.
 */
node: NodeId, 
/**
 * Its output port (always `out` for now).
 */
port: string, };
export type Severity = "error" | "warning" | "info";
export type TransactionId = string;
export type Value = boolean | number | number | string | Array<Value>;
