# Diagnostic codes (contract v1, draft for G1a)

One shape everywhere (commands, parser, planner, evaluator, store, worker, editor):
`{ code, severity, message, command?, node?, port?, param?, related?, feature?, span?, limit?, observed?, suggestion? }`.
`command` is the index of the failing command in its batch; `related` lists other nodes that took
part (the members of a cycle, the source of a mistyped link).
`code` is a stable dotted string (generated into a TS union); `message` is for people and may change;
`span` is `{ line, col, len }` into the text source; `limit` and `observed` are numbers with a unit
string. Severity: `error` blocks a commit, `warning` does not, `info` is progress or a no-op.

| Code | Severity | Meaning | Typical suggestion |
|---|---|---|---|
| `parse.syntax` | error | Text does not match the grammar | Show the expected form |
| `parse.unknown_op` | error | Operator not registered or not `ready` | Closest names by edit distance |
| `parse.unknown_param` | error | Parameter not declared by the operator | List valid parameters |
| `parse.missing_param` | error | Required parameter absent | Name it with its type and unit |
| `parse.duplicate_name` | error | Two statements define the same node name | The second definition's span |
| `parse.forward_reference` | error | A statement reads a node defined later | Define inputs first |
| `parse.unknown_name` | error | A statement reads a name no statement defines | |
| `provider.invalid_patch` | error | A provider reply is not exactly one bounded patch (prose, code, extra fields, empty or oversized); nothing was applied, no repair is attempted | |
| `provider.unsupported` | error | The provider does not handle the request | What it does handle |
| `provider.failed` | error | The provider failed to answer | |
| `param.type` | error | Wrong type | Expected type |
| `param.range` | error | Outside declared range | Nearest valid value and the range |
| `param.unit` | error | Unit mismatch or missing unit | Expected unit |
| `param.non_finite` | error | NaN or infinity | |
| `graph.unknown_node` | error | Reference to a missing node | |
| `graph.duplicate_id` | error | ID already used | |
| `graph.port_type` | error | Output type does not fit the input port | Types accepted by the port |
| `graph.port_arity` | error | Too few or too many inputs | Required count |
| `graph.cycle` | error | Link would create a cycle | The nodes on the cycle |
| `graph.group_recursion` | error | Group contains itself, directly or not | |
| `graph.group_version` | error | Group version missing or incompatible | Available versions |
| `graph.unknown_port` | error | The operator has no such input or output port | Valid ports |
| `graph.in_use` | error | Removing a node that other nodes or outputs still use | The readers; or use cascade |
| `graph.unknown_material` | error | Reference to a missing material | |
| `graph.unused` | warning | Node not reachable from any output | |
| `budget.source` | error | Source text over the byte limit | `limit`, `observed` |
| `budget.nodes` | error | Node count over limit (parsed or expanded); also output, material and ID limits | |
| `budget.commands` | error | More than 200 commands in a batch | Split the batch |
| `budget.expansion` | error | Predicted geometry or instance expansion over limit | Predicted size |
| `budget.geometry` | error | Vertex, face, grid-cell or point count over limit | |
| `budget.memory` | error | Predicted temporary or cached bytes over limit | |
| `budget.time` | error | Job exceeded its time budget | Preview vs final policy |
| `kernel.invalid_input` | error | Kernel rejected an argument | |
| `kernel.not_manifold` | error | Input mesh is not a valid closed solid | Which node produced it |
| `kernel.too_large` | error | Result too large for the kernel | |
| `result.empty` | info | Valid but empty (for example a difference that removes everything) | Not an error; distinct from failure |
| `constraint.min_wall` | error | A declared thickness constraint is violated | Which pair of nodes |
| `geometry.floating_part` | warning | Disconnected pieces with no declared socket relation | The piece IDs |
| `geometry.unsupported_case` | error | Valid input outside the operator's published contract | The documented limits |
| `eval.cancelled` | info | Job superseded or cancelled | |
| `eval.worker_crash` | error | Worker died; restarted; last scene kept | |
| `eval.worker_timeout` | error | Worker terminated after the hard limit | |
| `eval.stale_result` | info | Result for an older revision was discarded | |
| `revision.conflict` | error | `base_revision` is not current | `current_revision` and changed nodes |
| `revision.already_applied` | info | Transaction key seen before | The original revision |
| `revision.document` | error | The batch is for another document | |
| `selection.lost` | warning | Authored feature no longer exists | |
| `selection.ambiguous` | warning | Several candidates; none chosen | The candidates |
| `store.corrupt` | error | Saved data failed validation | Previous save kept |
| `store.quota` | error | Storage quota exceeded | Bytes needed |
| `store.newer_version` | error | File written by a newer schema | File kept untouched |
| `store.unknown_op` | error | File uses an operator this build lacks | File kept, shown read-only, node ids listed |
| `store.unavailable` | error | Storage cannot be read or written (blocked, disabled) | Previous save kept |
| `store.read_only` | error | Edit or save refused on a document that is read-only (unknown operators, or it failed to load) | Open with a build that can edit it |
| `store.migration_failed` | error | A migration step failed | Previous save kept |
| `provider.invalid_patch` | error | Provider output did not validate | Counts toward the repair cap |
| `provider.budget` | error | Calls, tokens or time cap reached | |

Logic codes (`logic.*`, defined with the logic track; see `logic.md` section 11): `unknown_event`,
`event_args`, `unknown_state`, `state_type`, `flow_cycle`, `flow_fan_in`, `unreachable_rule`,
`unreachable_objective`, `cascade_depth`, `step_budget`, `queue_overflow`, `target_missing`,
`rebuild_hot`, `write_conflict`, `retrigger`.

Rules: a diagnostic always names the smallest responsible node, port or feature it can; a failure
that cannot be located says so rather than guessing; adding a code is additive, changing a meaning is
a contract version bump.
