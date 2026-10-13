# G4b report: groups and instances

G4b is the second slice of G4 (reuse and assembly). A group is a reusable assembly defined once and
used many times; ADR 0013 has the design. Instance sets and realization (G4c), sockets and attachment
(G4d) and the text form for groups (G4e) are the rest of G4 and are not built.

## What exists
- **Document:** `Document.groups`; nodes carry `group` (member of a definition) or `instance` (group,
  pinned version, overrides). Old documents load unchanged; new fields are omitted when empty.
- **Commands:** `create_group`, `remove_group`, `set_group_name`, `expose_param`, `unexpose_param`,
  `set_group_output`, `add_instance`, `set_instance_param`, `clear_instance_param`, `upgrade_group`, and
  `add_node` with a `group` (`docs/modeling/commands.md`). Two operators, `group.use@1` and
  `group.input@1`, both `ready`.
- **Rules**, checked after every command (`group_check.rs`): no link crosses a group's edge, outputs
  never name a member, the result is a solid member, exposed parameters exist and are unique, input
  names are unique, no group contains itself, nesting is at most eight deep, and overrides of a
  current instance name exposed parameters and fit them.
- **Versions:** an interface change raises the group's version; an instance behind is refused at
  evaluation (`group.outdated`) until `upgrade_group`; `upgrade_preview` says what it would drop.
- **Evaluation:** `expand` unfolds groups before planning. The result of an instance takes the
  instance's own ID, so parts, selection and outputs keep their identity. Only members the result
  depends on are copied; a muted or bypassed instance is a placeholder; copies are budgeted
  (`MAX_EXPANDED_NODES`, 20,000). Failures inside a group are reported against the instance with the
  member in `related`.
- **Sharing:** cache keys ignore node IDs, and the evaluator now also looks in the current job's own
  results, so four instances with equal parameters run the prototype once.

## Evidence
- `ve-graph/tests/g4_groups.rs`, `g4_groups_lifecycle.rs`: cases G4b-01 to G4b-20 (21 tests), including
  recursion through a second group, the nesting bound, upgrade leaving every other node byte for byte
  unchanged, undo and redo of every group command, and the text form refusing groups.
- `ve-modeling/tests/g4_eval.rs`, `g4_eval_more.rs`: cases G4b-E1 to E8, G4b-11, 17, 18, 19. A wheel
  instanced four times executes 9 operators (five for the first wheel, four moves); an override on
  one wheel costs 2 more and changes that volume by the amount a plain torus with those facets gives;
  a definition edit re-runs 3 operators once for all four; nested groups share; unfolding 8^5 boxes is
  refused before any kernel call with the limit and the observed count.
- Fixture: `ve-graph/tests/fixtures/wheel_group.json` (tyre, hub, spokes, exposing `radius` and `width`).

## Not done, and known limits
- The text form cannot say groups (`to_text` refuses a document that has them, `text.unsupported`), so a
  model driving the engine through text cannot use groups yet (G4e).
- The viewer panels do not show groups well: members appear in the outliner among the other nodes and
  the inspector has no widgets for overrides or an upgrade button. That is the editor work of V4.
- Group inputs and results are solids only; profile and curve groups come with later gates.
- Instances are expanded to copies for evaluation; they are not yet shared in the renderer (G4c).
- A bypassed instance passes its first input (by name order) through; a group with several inputs has
  no better rule yet.
