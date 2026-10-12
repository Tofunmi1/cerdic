# 0008: Logic is embedded in the node language, with rules as the canonical form
- Status: accepted (owner asked for Blueprint-style logic inside the language, 2026-10-12)
- Date: 2026-10-12

## Context
The owner wants behaviour ("blueprints") inside the same language as geometry. `docs/architecture.md`
section 10 already defines a rule runtime (`Rule { when, if, then }`, bounded cascade, host effects,
a scenario validator and a simulated playthrough). Research (see the plan, section 8): Godot removed
its VisualScript (0.5 percent of users, no high-level components); Unity separates script graphs from
state graphs; Blueprints split exec and data and have latent actions; Scratch uses fixed yield points;
Verse annotates reversible and permanent effects; studies find tangled visual programs hard to read.

## Decision
- One typed graph, one document, one command path for geometry and logic. Ports gain a kind (`Data` or
  `Flow`); operators gain named output ports; the document gains typed variables.
- The canonical logic form is the rule (event, condition, actions). Sequences, branches and waits are
  structure inside a rule; state machines and the text `rule` block compile to it. The node editor and
  the text language are two views of the same nodes and links.
- The interpreter is a small Rust crate with a fixed 60 Hz tick, a per-tick step budget, bounded
  cascades, no recursion and no user code, logged inputs and a seeded random stream, so a session is
  replayable.
- It runs inside the engine module on the window thread; the geometry worker (ADR 0006) stays for
  rebuild jobs. This replaces the worker-hosted logic loop sketched in architecture section 8.
- Per-frame actions write only live parameters (transforms, visibility, material parameters). Writing a
  geometry parameter is a discrete rebuild request, coalesced.
- High-level vocabulary first (events, parts, sockets, objectives, interaction packs); a generic
  low-level graph is not the product.

## Consequences
Schema 2 adds variables and flow links (migration 1 to 2 adds empty variables). `OperatorSpec` needs
output ports and a port kind. G4 (parts, sockets, instances) is a prerequisite. The editor needs a flow
view and a debugger (L5). Contracts live in `docs/modeling/logic.md`; milestones L0 to L8 in the plan.
