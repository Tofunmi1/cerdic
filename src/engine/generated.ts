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
alias: Alias | null, 
/**
 * Makes the node a member of this group's definition instead of a top-level node.
 */
group: GroupRef | null, } } | { "remove_node": { 
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
collapsed: boolean, } } | { "create_group": { 
/**
 * Display name.
 */
name: string, 
/**
 * The nodes that become members.
 */
members: Array<NodeRef>, 
/**
 * The member whose result is the group's result (a solid).
 */
output: NodeRef, 
/**
 * A name later commands in this batch can use for the new group.
 */
alias: Alias | null, } } | { "remove_group": { 
/**
 * The group.
 */
group: GroupRef, } } | { "set_group_name": { 
/**
 * The group.
 */
group: GroupRef, 
/**
 * The new name.
 */
name: string, } } | { "expose_param": { 
/**
 * The group.
 */
group: GroupRef, 
/**
 * The name instances use.
 */
name: string, 
/**
 * The member holding the parameter.
 */
node: NodeRef, 
/**
 * The member's parameter.
 */
param: string, } } | { "unexpose_param": { 
/**
 * The group.
 */
group: GroupRef, 
/**
 * The exposed name.
 */
name: string, } } | { "set_group_output": { 
/**
 * The group.
 */
group: GroupRef, 
/**
 * The member.
 */
node: NodeRef, } } | { "add_instance": { 
/**
 * The group.
 */
group: GroupRef, 
/**
 * Display name.
 */
label: string | null, 
/**
 * A name later commands in this batch can use for the new node.
 */
alias: Alias | null, 
/**
 * Makes the instance a member of this group's definition (a nested group) instead of a
 * top-level node.
 */
into: GroupRef | null, } } | { "set_instance_param": { 
/**
 * The instance.
 */
node: NodeRef, 
/**
 * The exposed name.
 */
param: string, 
/**
 * The value, checked against the member parameter's type and range.
 */
value: Value, } } | { "clear_instance_param": { 
/**
 * The instance.
 */
node: NodeRef, 
/**
 * The exposed name.
 */
param: string, } } | { "upgrade_group": { 
/**
 * The instance.
 */
node: NodeRef, } };
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
 * Where in the text source the problem is, for diagnostics from text input.
 */
span?: Span | null, 
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
 * Group definitions; their members are nodes above that carry the group's ID.
 */
groups?: Array<Group>, 
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
export type Group = { 
/**
 * Stable identity.
 */
id: GroupId, 
/**
 * A display name; never part of identity or evaluation.
 */
name: string, 
/**
 * Increases whenever the interface changes; instances pin the version they were made against.
 */
version: number, 
/**
 * The member whose result is the group's result.
 */
output: NodeId, 
/**
 * What instances may override.
 */
params: Array<GroupParam>, };
export type GroupId = string;
export type GroupParam = { 
/**
 * The name instances use.
 */
name: string, 
/**
 * The member holding the parameter.
 */
node: NodeId, 
/**
 * The member's parameter.
 */
param: string, };
export type GroupRef = string;
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
export type Instance = { 
/**
 * The group.
 */
group: GroupId, 
/**
 * The version of the interface this instance was made against.
 */
version: number, 
/**
 * Overridden parameter values by exposed name.
 */
overrides: { [key in string]: Value }, };
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
flags: Flags, 
/**
 * The group whose definition this node belongs to; such nodes are not top-level.
 */
group?: GroupId | null, 
/**
 * Set on `group.use@1` nodes: the group, pinned version and overrides.
 */
instance?: Instance | null, };
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
export type Span = { 
/**
 * Line, starting at 1.
 */
line: number, 
/**
 * Column of the first character, starting at 1.
 */
col: number, 
/**
 * Number of characters covered.
 */
len: number, };
export type TransactionId = string;
export type Value = boolean | number | number | string | Array<Value>;
