# Embedded logic: contracts (v1, draft)

How behaviour ("blueprints") lives inside the same node language, document and command path as
geometry. Builds on `docs/architecture.md` section 10 (the rule runtime) and ADR 0008. Nothing here
is built yet.

## 1. The idea in one paragraph
A document is one typed graph. Geometry nodes pass *data* along data links and recompute when inputs
change. Logic nodes add *flow* links: "when this happens, then that". Both kinds are ordinary nodes
and links in the same document, edited by the same commands, undone by the same history, shown in the
same node editor (flow wires are drawn white, as in Blueprints) and written in the same text language.
The canonical form is the **rule**: `when` an event, `if` a condition, `then` actions. Everything else
(sequences, branches, waits, state machines) is structure inside a rule or sugar that compiles to
rules, so the validator can always analyse the whole behaviour.

## 2. Lessons that shaped this (sources in the plan, section 8)
| Source | What it showed | What we do |
|---|---|---|
| Godot VisualScript (removed in 4.0) | 0.5 percent of users; no high-level components, so it added nothing over text | The value is the high-level vocabulary (events, parts, sockets, objectives) and the LLM, not a generic low-level graph. Text is a first-class view of every graph |
| Unity script and state graphs | Separating "what happens" from "when behaviour switches" keeps both small | Rules and flow for what happens; state machines (L8) for modes |
| Unity custom events | Stringly-typed name and argument-count matching produced errors | Events are typed and registered; payload types are checked at edit time |
| Unreal Blueprints | Exec pins, pure nodes, latent actions; the debugger only steps forward | Same split of flow and data; latent actions are waits and animations with explicit yield points; debugging replays from a log, so it can step backward |
| Scratch | Hat-block events, cooperative scheduler, fixed yield points | Chains run until a yield point; no pre-emption; fixed tick |
| Verse (secondary sources) | Effect annotations: reversible versus permanent | Each action declares an effect class; the playthrough simulator uses only reversible ones |
| Blender simulation zones | State carried between frames, delta time, cache or bake | Variables and play-time state live in the play world and are replayable |
| Visual-program studies | Knots of wires cancel the advantage of graphics | Flow is structured (chains and trees, no arbitrary back edges); groups, auto layout and the text view keep graphs readable |
| Our architecture, section 10 | Rule, Event, Expr, Action, Objective; bounded cascade; host effects; scenario validator | Kept as the canonical IR; the node view is a view over it |

## 3. Port kinds and node families
`OperatorSpec` ports gain a kind: `Data` (a typed value) or `Flow` (an execution wire). A node has
named output ports (a branch has `then` and `else`). A flow link may only join flow ports; a data link
only data ports; the editor and validator say why a wire is refused.

| Family | Examples | Notes |
|---|---|---|
| `event.*` (hats: no flow input, one flow output, data outputs for the payload) | `event.select(part)`, `event.hover_begin/end`, `event.grab_begin/end`, `event.insert(item, socket)`, `event.enter/exit(zone)`, `event.timer(id)`, `event.state_changed(key)`, `event.start`, `event.custom(name)` | Payload fields are typed and checked: `actor`, `entity`, `part`, `socket` |
| `flow.*` | `branch(cond)`, `sequence(n)`, `wait(seconds)`, `once`, `repeat(count, max 64)`, `emit(event)`, `gate`, `all`/`race` (later) | `wait` is the only latent node; it ends the current step and resumes on a later tick |
| `act.*` | `set(state, value)`, `set_field`, `animate(part, to, seconds)`, `enable`, `attach`, `detach`, `spawn_instance`, `complete_objective`, `host(effect, args)` | Each has an effect class (section 5) and a param class (section 6) |
| `state.*` | `state.bool/number/text(initial)`, `state.get`, entity fields | Typed variables with an initial value stored in the document |
| `expr.*` (pure data) | comparisons, `and/or/not`, arithmetic, `holds(actor, item)`, `has_component`, `random(seed_stream)` | Reuses the geometry data graph; evaluated when a rule reads it |
| `state_machine.*` (L8) | `machine`, `state`, `transition` | Compiles to rules; for modes such as closed, opening, open |

A pack (architecture section 9) contributes events, conditions and actions as registered operators,
so the interaction vocabulary (door, key, lock, switch, socket) is data the AI can extend.

## 4. Text form and its equivalence to nodes
```text
door    = solid.box(size=[1, 2, 0.05])
panel   = part(door, name="door", pivot=[-0.5, 0, 0])
@locked = state.bool(initial=true)

rule unlock:
  on  event.insert(item="key", socket="lock")
  then act.set(@locked, false), act.host("sound.click")

rule open_when_unlocked:
  on  event.select(part="door")
  if  not @locked
  then act.animate(part="door", rotate_y=1.57, seconds=0.4)
```
A `rule` block is sugar. It compiles to nodes: an event hat, a `flow.branch` for the `if`, and `act.*`
nodes chained with flow links. The document stores only nodes and links, so the node editor shows
exactly the same logic with white wires, and either view's edits become the same commands. Two rules
that share a `when` are two hats on one event.

## 5. Execution semantics (Rust interpreter, pure crate)
- **Fixed tick.** Logic time is an integer tick count at 60 Hz. Durations in seconds round to ticks.
  No wall clock anywhere in the logic runtime.
- **Per tick:** (1) drain input events (queue of at most 1,024, first in first out; overflow drops the
  newest and raises `logic.queue_overflow`); (2) for each event, in queue order, start every matching
  rule in authored order; (3) run each started chain until it ends or reaches a yield point (`wait`,
  an awaited `animate`, or `flow.next_tick`); (4) advance timers and animations; (5) publish the
  outputs (transforms, state changes, host effects).
- **Bounds.** At most 10,000 node steps per tick (excess chains resume next tick and raise
  `logic.step_budget`), cascade depth at most 16 (`emit` chains), 256 timers, 1,024 state variables,
  64 iterations per `repeat`. Nothing allocates unboundedly; there is no recursion and no user code.
- **Re-trigger policy** per rule when its event arrives while its chain is still running: `ignore`
  (default), `restart`, or `queue` (at most 8). Never implicit.
- **State writes are immediate and ordered** (the order above), so the result is a pure function of
  the document, the seed and the event log. Two rules writing one variable in the same tick give a
  `logic.write_conflict` warning at edit time.
- **Failure is local.** An action whose target is missing aborts only its chain, emits a
  `logic.target_missing` diagnostic event, and play continues. Conditions are total: they cannot fail.
- **Effect classes.** `state` (reversible: set, animate, enable, attach), `emit` (internal events),
  `host` (permanent and external: sound, UI, LLM dialogue). Host effects are queued for the page and
  their results return as events. The playthrough simulator applies `state` and `emit` and treats
  `host` as a no-op.

## 6. Logic drives parameters: live versus rebuild
Per-frame animation must never run the geometry kernel. Every parameter an action can write has a
class declared in its `OperatorSpec`:
- **Live:** instance transform, visibility, material parameters, part pivot angle. Applied to the
  play world's entities directly, every frame, no evaluation.
- **Rebuild:** anything that changes geometry (a width, a segment count). Allowed as a discrete
  action; it starts an evaluation job in the worker (the same path as editing), the old geometry stays
  until the new result swaps in, and requests per node are coalesced to at most 4 per second.
  An `animate` over a rebuild parameter is refused with `logic.rebuild_hot`.

## 7. Edit and Play
Play clones the document into a world with runtime state and starts the interpreter; stopping
discards it. Logic never edits the authored document. The interpreter runs inside the engine module
on the window thread under a step budget (no SharedArrayBuffer, no second worker). The geometry worker
stays for rebuild jobs. This departs from architecture section 8, which put the logic loop in the
worker; ADR 0008 records why (geometry jobs can run for seconds and must not stall logic).

## 8. Determinism, replay and debugging
- **Inputs are logged.** A play session is `{ document revision, seed, events: [(tick, event)] }`.
  Replaying the log reproduces every tick; a state hash per tick detects divergence.
- **Deterministic ingredients:** fixed tick, seeded random streams split per rule, stable iteration
  order (authored order and IDs, never hash order), and a pure `libm` for trigonometry in expressions.
- **Debugger (editor):** breakpoints on nodes (pause at the tick when a chain reaches one), watches on
  data pins (last value, or "no data" if never evaluated), live highlight of flow wires that fired this
  tick, per-rule fire counts and step cost, an event log, and a variable watcher. Because the log
  replays, **stepping backward** works by replaying from the start to the previous tick.

## 9. Analysis (the scenario validator, in the editor)
Always on, cheap: unknown events, actions or state; payload type mismatches; flow cycles and fan-in
without a merge; rules whose event nothing can produce; unreachable objectives; cascade cycles over
`emit`; conflicting writers; rebuild actions in hot paths; unbounded wait chains. On demand: a bounded
**simulated playthrough** that tries every available interaction using reversible actions only, and
reports objectives it could not reach. A reached objective is evidence, not proof.

## 10. AI authoring
The provider's tools are generated from the registry: `describe_events`, `describe_conditions`,
`describe_actions`, `draft_rules` (text or commands), `validate`, `simulate_playthrough`. Output is
always typed commands against the document, validated like any batch, shown as a reviewable pending
change (graph diff plus play preview). The rubric for logic is behavioural: the validator is clean and
the simulated playthrough solves the stated objective. There is no published benchmark for LLMs
writing Blueprint-style graphs (the search found none), so the corpus and thresholds are ours.

## 11. Diagnostic codes (added to `diagnostics.md`)
`logic.unknown_event`, `logic.event_args`, `logic.unknown_state`, `logic.state_type`,
`logic.flow_cycle`, `logic.flow_fan_in`, `logic.unreachable_rule`, `logic.unreachable_objective`,
`logic.cascade_depth`, `logic.step_budget`, `logic.queue_overflow`, `logic.target_missing`,
`logic.rebuild_hot`, `logic.write_conflict`, `logic.retrigger`.

## 12. Not in this version
Arbitrary user code and expression escape hatches (no code execution, ever, per ADR 0002 and the
architecture); recursion and user-defined functions (reuse is by groups and macros of existing
actions); networking and multi-user logic; physics events; NPC dialogue (a pack, architecture
section 12) beyond the host-effect hook; `race` and `all` combinators and Verse-style transactions
(later, if fixtures need them).
