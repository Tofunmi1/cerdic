# Spatial Lab — Engine Architecture

Status: **design document**. It describes the target architecture. Parts marked *(verify)* are assumptions to confirm with a spike before relying on them. Current code is the "Spatial Lab" vertical slice described in the README; §17 maps it onto this design.

## 1. What we are building

A general-purpose, AI-native engine for authoring interactive 3D/VR experiences in the browser. A user describes an experience in natural language; the engine plans it, builds it from registered capabilities, attaches working logic, and lets the user keep shaping it by conversation, direct manipulation, or in VR.

The product's claim is **generality through real extensibility**: the core knows no domain. Escape rooms, lessons, anatomy exploration, engineering assemblies and intelligent NPCs are all *packs*. Installing a pack teaches the editor, the validator and the AI at the same time.

### Non-negotiable principles

1. **Core is domain-agnostic.** Even primitives, rooms and doors live in packs. If a first-party feature needs a core special case, the pack API is wrong.
2. **One command path.** Manual edits, AI, VR controllers, NPCs and future MCP tools all submit the same validated, atomic, undoable transactions.
3. **The AI only sees the registry.** Tool schemas, vocabulary and examples are generated from installed packs. No hand-maintained prompt lists object types.
4. **The LLM never executes code and never chooses coordinates directly.** It emits structured plans with spatial *relations*. Rust resolves them to transforms and validates the result.
5. **Rust owns the engine; TypeScript owns the shell.** Deterministic, testable logic is in Rust. UI, rendering, networking and AI orchestration are in TypeScript.
6. **Edit and Play are separate worlds.** Play runs on a clone and never overwrites the authored document.
7. **Honest validation.** We check references, types, dependency cycles and reachability heuristics. We never claim a generated scenario is proven solvable.

## 2. Language decision: one hybrid Rust + C++ module

**Rust owns the engine's state and rules. C++ owns numerical geometry. Both link into one WebAssembly module** and share one linear memory, so Rust calls C++ directly with no copies.

This was verified on 2026-10-09 in `spikes/hybrid-wasm/` (see its README). A single `wasm32-wasip1` module ran in Bun and in Chromium with the same results:
- Manifold boolean (C++)
- our own C++ using libc++
- meshoptimizer simplification (C++)
- Rapier physics (Rust) colliding with the C++-generated mesh

Size was 522 KB brotli.

| Language | Owns | Why |
|---|---|---|
| **Rust** | document, commands, history, registry, schema, validation, placement solver, logic runtime, agents/memory, physics (Rapier), parametric archetypes | correctness and memory safety for everything that touches state and AI output; `serde`/`schemars` give one schema source |
| **C++ (libraries)** | Manifold (booleans), Recast/Detour (navmesh, crowds), meshoptimizer (simplify, LOD, compression), V-HACD (convex decomposition for colliders) | mature numerical geometry that has no equal in other languages |
| **C++ (ours)** | `cpp/geomproc`: mesh repair (weld, hole fill, make-manifold), import cleanup, collider preparation, glue for the libraries | preparing imported and AI-generated meshes for CSG and physics |
| **TypeScript** | UI, rendering, XR, audio, networking, AI orchestration, pack UI | the shell |

**Build model** (all were required by the spike):
- Rust targets `wasm32-wasip1`. C++ is built with **wasi-sdk** (clang + libc++) and statically linked.
- The module is a WASI **reactor**: it exports `_initialize`, and the host calls it once.
- C++ builds with `-fno-exceptions` against the `noeh` libc++.
- **No `wasm-bindgen`.** The boundary is C-ABI exports plus JSON/buffers (§14), and a small browser shim satisfies WASI imports that are never actually called.
- Rust→C++ calls start as `extern "C"` wrappers. The `cxx` crate can replace them where richer types are worth it.

**Separate, lazy-loaded modules** are used only where size requires it: OpenCascade (CAD/STEP, tens of MB) for the engineering pack.

`boolmesh` (pure Rust, MPL-2.0) also compiles to wasm32 and remains a fallback if a pure-Rust build is ever needed.

## 3. Runtime topology

```text
┌──────────────────────── Browser ─────────────────────────────────────────┐
│ Main thread                                                              │
│  React shell (Liquid Glass UI) · R3F renderer · WebXR · audio            │
│  DocumentMirror (read-only, updated by diffs) · pack UI contributions    │
│        ▲ diffs, mesh buffers, transforms        │ transactions, input    │
│        │                                        ▼                        │
│ Engine worker                                                            │
│  engine.wasm  Rust: model · commands · history · registry · spatial ·    │
│               logic · physics · agents · validators · packs              │
│               C++: Manifold · Recast/Detour · meshoptimizer · V-HACD ·   │
│                    geomproc (ours)                                       │
│  Host bridge (TS): executes host effects                                 │
│        │                                                                 │
│  Lazy module worker: OpenCascade (engineering pack only)                 │
└────────┼─────────────────────────────────────────────────────────────────┘
         │ HTTPS (AI planning, NPC dialogue, asset jobs, persistence)
┌────────▼──────────────── Server (Bun) ───────────────────────────────────┐
│  AI orchestrator · provider adapters (LLM, 3D-gen) · same engine.wasm   │
│  for draft validation · asset catalog · project storage · rate limiting  │
└──────────────────────────────────────────────────────────────────────────┘
```

- **Single source of truth:** the Rust `Document` inside the engine worker. The main thread holds a mirror that it updates from diffs and never mutates directly.
- **Play loop:** the worker steps logic and physics at a fixed rate and publishes transforms. It uses a `SharedArrayBuffer` when the page is cross-origin isolated (COOP/COEP headers) and otherwise transfers buffers. *(Verify the latency budget on the headset.)* While the user is grabbing an object in VR, the main thread drives it kinematically and reports it to the worker, so hand latency never waits on a worker round trip.

## 4. Rust workspace

```text
crates/
  engine-model/      ids, Document, entities, components, Value, schema DSL, diff
  engine-registry/   pack manifests, archetype/component/action/condition registries,
                     capability export (JSON) for inspector + AI
  engine-commands/   transactions, dry-run, commit, inverse ops, history, revisions
  engine-geometry/   mesh types, archetype generators, named parts, UVs/normals, CSG
  engine-spatial/    bounds, BVH, raycast, placement relations + solver, overlap
  engine-logic/      events, expressions, rules, timers, objectives, Play runtime,
                     scenario dependency validator
  engine-physics/    rapier integration (Play only), colliders from geometry
  engine-agents/     agent state, perception, memory store, retrieval scoring
  engine-ffi/        safe Rust wrappers over the C++ libraries and geomproc
  engine-wasm/       the C-ABI export crate: Engine facade, _initialize, buffers
cpp/
  geomproc/          our C++: mesh repair, import cleanup, collider prep, glue
  lib/               Manifold, Clipper2, Recast/Detour, meshoptimizer, V-HACD (pinned)
packs/
  <pack-id>/rust/    optional Rust half of a code pack (implements EnginePack)
```

All crates except `engine-wasm` are plain Rust with no browser dependencies, and are tested natively with `cargo nextest`. The C++ libraries are built for the host too, so `engine-ffi` tests also run natively; the same tests then run again inside WASM.

### Size estimate (code we write; tests included)

| Crate | Lines (approx.) |
|---|---|
| engine-model + registry | 3,000–5,000 |
| engine-commands | 1,500–2,500 |
| engine-geometry (+ first-party archetypes) | 4,000–7,000 |
| engine-spatial | 2,500–4,000 |
| engine-logic | 3,000–5,000 |
| engine-physics | 1,000–2,000 |
| engine-agents | 1,500–3,000 |
| engine-ffi + engine-wasm | 1,500–2,500 |
| Rust halves of first-party packs | 4,000–8,000 |
| **Rust total** | **≈ 21,000–38,000** |
| `cpp/geomproc` (ours) | 4,000–8,000 |

Linked libraries (Rapier, parry, nalgebra, serde; Manifold, Clipper2, Recast/Detour, meshoptimizer, V-HACD) add well over 100k lines of compiled Rust and C++. **WASM size budget:** the spike (Rapier + Manifold + meshoptimizer) is 522 KB brotli. Set a budget (for example ≤ 3 MB brotli for `engine.wasm`) and check it in CI.

## 5. Data model

### 5.1 Document

```text
Document
  format: "spatial-lab.project", formatVersion
  packs: [{ id, version }]             // the packs this document depends on
  entities: Map<EntityId, Entity>
  rules: Map<RuleId, Rule>
  state: Map<StateKey, ValueSpec>      // declared scenario variables + initial values
  objectives: Map<ObjectiveId, Objective>
  revision: u64

Entity
  id: EntityId                          // stable string (UUID or AI-chosen slug), never reused
  name, parent: Option<EntityId>, tags
  components: Map<ComponentKey, Value>  // e.g. "core.transform", "interaction.lock"
```

- Entities are ECS-style: an entity is an ID plus components. Behavior comes from which components it has, not from a class hierarchy, so packs extend entities without inheritance.
- Component keys are namespaced `<pack>.<component>`. Unknown keys are kept verbatim (§9.5).
- Internally the engine maps `EntityId` to compact slot handles (`slotmap`). Only stable IDs cross the boundary.

### 5.2 Schema DSL (one schema, four consumers)

Every component, archetype parameter set, action and condition is described in a small schema language:

```text
f32{min,max,unit,step} · i32{min,max} · bool · string{max} · enum[...] · vec3{unit}
color · angle · entity_ref{with: [component...]} · state_ref · asset_ref{kind}
list<T>{max} · struct{...} · optional<T>
```

Consumers:
1. **Engine validation:** types, ranges, required fields.
2. **Referential integrity:** `entity_ref{with:["interaction.lock"]}` lets the core check *every* cross-entity link from *every* pack generically. This is how "missing references" are caught without per-pack code.
3. **Inspector UI:** fields render from the schema, so packs rarely need custom UI.
4. **AI tools:** the schema is exported as JSON Schema with descriptions and units for tool and structured-output definitions.

Native (Rust-typed) components derive the schema from their struct with `schemars` plus field attributes. Declarative components declare it as data. To the rest of the system the two are indistinguishable.

## 6. Commands, transactions, history

```text
Transaction { id, baseRevision, origin: user|ai|vr|npc|mcp, label, ops: [Op] }
Op = CreateEntity | DeleteEntity | SetComponent | PatchComponent | RemoveComponent
   | Reparent | Rename | InstantiateArchetype{archetype, params, relations}
   | UpsertRule | DeleteRule | UpsertObjective | DeleteObjective | DeclareState
```

Lifecycle: **dry_run → preview → commit → history → persist**.

- `dry_run(tx)` applies to a copy-on-write view and returns `{ diff, diagnostics, resolvedTransforms }`. The UI previews the diff (ghosted new objects, highlighted changes) before accepting.
- `commit(tx)` rejects the transaction if `baseRevision` is stale. Exception: transactions marked *rebaseable*, whose touched entities have not changed since then, can still apply. This allows late AI and asset results without clobbering newer edits.
- History stores **inverse ops**, not document snapshots, so memory scales with edits rather than scene size. Bounded depth; one transaction = one undo step; a VR drag or gizmo drag coalesces into one transaction.
- `InstantiateArchetype` with `relations` runs the placement solver (§8.2) during the transaction, so AI output never has to contain raw coordinates.

## 7. Geometry

- **Archetypes** are registered by packs with `id`, parameter schema, generator and declared **named parts**. Examples: `cabinet` has `body`, `door_left`, `door_right`, `shelf[n]`; `room` has `floor`, `walls[n]`, `openings[n]`.
- Generators output `MeshBuffers { positions, normals, uvs, indices, parts: [{name, indexRange, materialSlot, pivot}] }` plus local bounds. Named parts can be animated (doors swing about their pivot), selected and given materials, without separate entities.
- Regeneration happens only when parameters change. Results are keyed by `(entity, geometryRevision)`, so stale results are discarded.
- CSG (Manifold, C++) handles openings, cutouts and engineering parts. Inputs must be manifold: generators guarantee this, and `geomproc` repairs imported meshes first.
- Catalog/imported GLB assets are geometry components of kind `asset`. In the MVP they support transforms and materials. Part-level editing needs named parts authored in Blender or a later segmentation step.
- **Later milestone:** an editable half-edge mesh (`engine-geometry::editable`) for vertex/edge/face editing.

## 8. Spatial

### 8.1 Queries
Rust has a BVH over world-space bounds and supports raycasts (selection, VR pointing, NPC line of sight), overlap tests and support queries ("what is this resting on").

### 8.2 Placement relations + solver
The AI and the user describe intent; the solver computes transforms.

```text
on(target, region?) · inside(container, slot?) · under(target) · against(wall|entity, side)
near(target, distance?) · facing(target) · between(a, b) · row(entities, axis, spacing)
at(anchor) · clearance(min) · in_room(room) · aligned(target, axis)
```

The solver:
1. Turns relations into candidate regions: top faces, container slots, wall bands.
2. Places entities in dependency order (support before supported).
3. Resolves overlaps by priority with deterministic jitter (seeded).
4. Checks bounds against rooms and the floor.
5. Emits diagnostics for constraints it couldn't satisfy. The repair loop (§11) feeds them back to the LLM.

Manual moves remove an entity's relations (or mark them detached) so the solver never fights the user.

## 9. Pack system

### 9.1 What a pack can contribute

| Contribution | Rust half | TS half | Declarative (data) |
|---|---|---|---|
| Components (schema + defaults) | ✓ | | ✓ |
| Systems (Play-time behavior) | ✓ | | |
| Archetypes / generators | ✓ (generators) | | ✓ (compositions of existing ones) |
| Events, conditions, actions | ✓ | host actions | ✓ (macros of existing actions) |
| Validators | ✓ | | |
| Host effects (audio, UI, LLM, nav) | | ✓ | |
| Inspector overrides, panels, VR panels | | ✓ | |
| AI vocabulary: descriptions, examples, planning hints | | | ✓ |
| Asset catalogs (with license metadata) | | | ✓ |
| Migrations | ✓ | | |

### 9.2 Contracts

```rust
pub trait EnginePack: Send + Sync {
    fn manifest(&self) -> PackManifest;            // id, version, requires, capabilities
    fn register(&self, r: &mut Registry);           // components, archetypes, rules vocab, validators, systems
    fn migrate(&self, from: &Version, doc: &mut PackData) -> Result<(), MigrationError> { Ok(()) }
}
```

```ts
export default definePack({
  id: "npc", version: "1.0.0", requires: ["core@^1", "interaction@^1"],
  hostEffects: { "npc.converse": converse, "npc.speak": speak },
  inspector: { "npc.persona": PersonaEditor },
  vrPanels: { dialogue: DialoguePanel },
  ai: { description, examples: "./ai/examples.json" },
});
```

### 9.3 Tiers

1. **Declarative packs: data only, and the AI can create them.** They define new archetypes as compositions of existing ones, plus their components and rules, e.g. a vending machine that dispenses an item when a coin goes into its socket. They are validated like any document and run no code. A user can say "make this a reusable template" and the AI saves it to their library. This is how the app extends itself through conversation, safely.
2. **Code packs: trusted, compiled in.** The Rust half is linked into `engine.wasm`; a build script generates the registration list from the workspace's enabled packs. The TS half is bundled. Adding one requires a rebuild, which is honest for a first-party ecosystem.
3. **Sandboxed WASM packs (later).** A third-party pack ships its own WASM module and talks to a narrow, capability-scoped host API (registry + ops only, no DOM or network). The contracts in §9.2 are written so a Tier-2 pack could move to Tier 3 without changes.

### 9.4 Registry export → AI and UI
`engine.capabilities()` returns every registered component, archetype, relation, event, condition and action. Each comes with its schema, description, units and examples, filtered by the packs active in the project. The server builds the LLM's tools and system context from this export. **Adding a pack automatically teaches the AI.**

### 9.5 Robustness
- Documents record pack versions. On load, a missing pack produces **preserved placeholders**: its components are kept as opaque data, rendered as a labelled box, and written back unchanged on save.
- Pack migrations run in version order on load.
- A conformance test suite runs against every pack: schema round-trips, generator bounds, validator determinism, and "registered but unreachable" checks.

### 9.6 First-party packs

| Pack | Purpose |
|---|---|
| `core.primitives` | box/cylinder/sphere/plane/etc. (today's factories) |
| `architecture` | room, wall, opening, door frame, window, stairs, ramp, column |
| `furniture` | desk, table, chair, shelf, cabinet, locker, crate (parametric, named parts) |
| `interaction` | pickup, container, lock, key, socket, switch, button, door logic, trigger zone, light, sound emitter, timer |
| `scenario` | objectives, hints, completion screens |
| `physics` | rigid bodies, joints, materials (wraps engine-physics) |
| `npc` | persona, perception, memory, dialogue, action whitelist, navigation |
| `education` | label, inspect, quiz, step sequence, assessment |
| `anatomy` | curated licensed anatomy models with labelled parts (educational, not clinical) |
| `engineering` | CSG parts, measurement, units, simple beam/statics checks, STEP import (OCCT, later) |

**Test of the architecture:** `interaction`, `npc`, `education` and `anatomy` are each built without changing any `engine-*` crate.

## 10. Logic runtime (Play mode)

```text
Rule      { id, when: EventPattern, if: Expr?, then: [Action], once?: bool }
Event     interact(entity, actor) · grab/release · insert(item, socket) · enter/exit(zone, actor)
          · state_changed(key) · timer(id) · npc.* · custom(pack.event)
Expr      state comparisons · has_component · field compare · holds(actor, item) · and/or/not
Action    set_state · set_field · enable/disable · attach/detach · animate_part(entity, part, to)
          · start_timer · emit(event) · complete_objective · host(pack.effect, args)
Objective { id, title, requires: [ObjectiveId], complete_when: Expr, hint? }
```

- Play starts by cloning the document into a `World` with runtime state. Stopping discards it, and restarting reinitializes from the document.
- Each tick: input events → rules (bounded cascade depth; event cycles produce errors) → systems (physics, agents, timers) → outputs `{ transforms, effects }`. Host effects (sound, UI, LLM dialogue) run in TS, and their results come back as events.
- **Scenario validator** (Edit mode):
  - Builds a dependency graph from keys→locks→containers→contents, sockets, objectives and rules.
  - Reports dangling references, unreachable objectives, prerequisite cycles, keys sealed behind their own lock, items placed inside unopenable volumes, and actions targeting the wrong component types.
  - Optionally runs a bounded **simulated playthrough**: a greedy agent that tries every available interaction. It reports objectives it couldn't reach. A reached objective is evidence, not proof.

## 11. AI pipeline

### 11.1 Stages
1. **Understand:** classify the request (create / edit / query / template) using the scene summary and selection.
2. **Plan** (create or large edit): the LLM produces a `ScenePlan` (zones, entities with archetypes and params, relations, rules, objectives) using only registry vocabulary.
3. **Resolve assets:** procedural archetype → catalog asset → (later) neural generation job with placeholder.
4. **Compile:** turn the plan into a `Transaction`.
5. **Draft + validate:** the server runs `dry_run` in its own engine instance (same WASM). Diagnostics go back to the model.
6. **Repair loop:** at most N rounds of targeted fixes driven by diagnostics.
7. **Return:** the transaction, a summary and the remaining warnings go to the client. The client runs `dry_run` against the live revision and shows the preview; the user accepts or rejects.

### 11.2 Tools exposed to the model (generated from the registry)
`scene_summary`, `find_entities(query)`, `get_entity(id)`, `describe_capabilities(filter)`, `draft_transaction(ops)`, `validate()`, `simulate_playthrough()`, `save_template(entities)`. The same tool set will later be exposed over **MCP**.

### 11.3 Context management
- Send a compact scene summary (IDs, names, archetypes, key params, relations), not full component data. The model fetches detail with tools.
- Capability docs come only from active packs, ranked by relevance to the request.

### 11.4 Providers
`LlmProvider` adapters (Claude first; others pluggable) and `MeshGenProvider` (`submit/getStatus/fetchResult/cancel`). Use a larger model for planning and a fast model for NPC dialogue. Confirm current model IDs and terms when implementing.

### 11.5 Evaluation
`evals/` holds 50+ prompts across packs with automatic checks: validation passes, entity counts, required relations satisfied, objectives reachable in a simulated playthrough. Run them on every pipeline or prompt change. This is how we *measure* generality instead of claiming it.

## 12. Agents and NPC memory (`engine-agents` + `npc` pack)

```text
Agent
  persona: { name, role, voice, goals, style }          // authored or AI-generated
  knowledge: [{ fact, unlock_when: Expr? }]              // gated: no spoilers
  allowed_actions: [ActionTemplate]                       // whitelist → validated rule actions
  perception: { radius, fov, requires_los }
  memory:
    working:  last K turns + current situation
    episodic: [{ t, kind: saw|heard|said|did, text, entities, importance }]
    semantic: [{ belief, confidence, sources }]           // consolidated by LLM reflection
```

- **Perception:** the agent subscribes to the runtime event bus, filtered by radius, field of view and line of sight (Rust raycast).
- **Retrieval:** a score combining recency, importance and relevance (after Park et al., *Generative Agents*, 2023). Relevance uses BM25 over tokens in Rust; optional embeddings come from a provider and are compared in Rust.
- **Reflection:** when accumulated importance passes a threshold, a host effect asks the LLM to consolidate episodic memories into semantic beliefs.
- **Dialogue:** `npc.converse` host effect → server LLM receives persona, unlocked knowledge, retrieved memories, scenario state and `allowed_actions` as tools → returns speech + actions → actions are validated as ordinary rule actions. Speech uses TTS, and voice input uses Web Speech or STT.
- **Movement:** `walk_to` is handled by Recast/Detour inside `engine.wasm` (via `engine-ffi`). The navmesh is rebuilt when walkable geometry changes in Edit mode.
- **Lifetime:** memory resets with Play by default. Per-player persistent memory is optional.
- **Reuse:** the education pack's tutor and the scenario hint system use the same agent machinery.

## 13. Assets

- The catalog stores `{ id, source, license, attribution, tags, bounds, pivot, named parts, LOD }`. Licenses are checked by CI, and attributions are generated into an in-app credits page.
- Each pack has its own catalog. Anatomy uses only curated, licensed sources (e.g. Z-Anatomy CC BY-SA 4.0 with its non-commercial sub-models excluded; BodyParts3D CC BY-SA 2.1 JP).
- Neural generation (later): a job with a placeholder entity. Its result applies as a *rebaseable* transaction (§6) touching only the geometry component, so placement and later edits survive.

## 14. JS ↔ WASM boundary

A plain C ABI with no `wasm-bindgen`. Each call passes a pointer and length into linear memory. A small generated TS wrapper handles allocation (`engine_alloc`/`engine_free`), JSON encoding and typed-array views, and calls `_initialize()` once after instantiation. Logical API:

```text
Engine::new(capability_config) · load(json) · save() -> json · capabilities() -> json
dry_run(tx_json) -> json · commit(tx_json) -> json(diff) · undo()/redo() -> json(diff)
query(json) -> json · validate() -> json · simulate() -> json
mesh(entity, geometry_revision) -> MeshBuffers (owned Vec<f32>/Vec<u32> → transferred)
play_start() · play_step(dt, input_json) -> json(effects) · transforms_view() -> Float32Array
play_event(json) · play_stop()
```

- **JSON for commands and diffs:** they are infrequent, easy to debug, and the LLM already speaks JSON.
- **Binary for meshes and per-frame transforms.**
- Mesh outputs are copied out of linear memory into fresh `ArrayBuffer`s before transfer. Views into linear memory are re-created after any call that may grow memory.
- Every request carries a request ID; responses for superseded revisions are dropped.
- Generated TS types: `ts-rs` or `schemars` → JSON Schema → TS, so TypeScript never hand-writes engine types.

## 15. Persistence, rendering, XR, UI

- **Persistence:** versioned project JSON (§5.1) + asset references. Stored in IndexedDB locally; later server-side private storage. Chat logs are stored separately from the document.
- **Rendering:** R3F renders the mirror. Mesh buffers are cached by geometry revision; instancing is used for repeated catalog assets.
- **XR:** `@react-three/xr`, with interaction via controller rays, grab and hands *(verify on the headset)*. In-scene panels use `@pmndrs/uikit`: prompt (voice), inspector-lite, objectives and NPC dialogue. Full authoring stays desktop-first; VR supports playing, light editing and voice commands.
- **UI:** Liquid Glass shell (existing `src/ui`). Pack UI uses the registered inspector and panel contributions.
- **Audio:** WebAudio positional sound emitters, driven by host effects.

## 16. Security, privacy, testing, build

- **Security:**
  - No generated code ever executes; everything is data validated by Rust.
  - API keys exist only on the server.
  - Rate limits and cost caps apply to the hosted demo.
  - Request size, scene size and repair rounds are bounded.
  - NPC LLM outputs are validated like any other transaction.
- **Privacy:** the repository and deployment stay private until the owner approves publishing (the hackathon requires a public repo at submission).
- **Tests:**
  - `cargo nextest` for every engine crate (property tests for commands/undo round-trips, solver determinism).
  - Pack conformance suite.
  - WASM smoke tests in Bun.
  - Playwright end-to-end tests.
  - The AI eval suite.
  - Manual headset checklist.
- **Build:**
  1. CMake + wasi-sdk build the C++ static libraries (pinned versions, `-fno-exceptions`).
  2. `cargo build --target wasm32-wasip1 --release` links them.
  3. `wasm-opt -Oz`, then Vite bundles the result, followed by a size budget check.
  4. The same C++ libraries are also built for the host so native tests run.
  5. The toolchain versions (wasi-sdk, Rust, wasm-opt) are pinned in one file. `spikes/hybrid-wasm/build.sh` is the starting point.

## 17. Migration from the current slice

| Today | Becomes |
|---|---|
| `shared/commands.ts` (Zod schema + reducer) | `engine-model` + `engine-commands`; Zod is replaced by generated types |
| `shared/history.ts` (snapshots) | inverse-op history in `engine-commands` |
| `rust/geometry` (unit primitives) | `engine-geometry` + `core.primitives` pack |
| `src/scene/factories.ts` | mesh cache fed by engine `mesh()` |
| `src/scene/Viewport.tsx` | renders `DocumentMirror`; gizmos submit transactions |
| `server/adapters.ts` (`SceneAdapter`) | `LlmProvider` + AI orchestrator with the tool loop (mock stays for tests) |
| `localStorage` scene | IndexedDB project file |

Existing tests are ported, and keep passing at each step.

## 18. Milestones (dependency order)

1. **M0 Foundations:** Cargo workspace, engine worker, `Engine` facade, generated TS types, build and size budget.
2. **M1 Model parity:** document/commands/history in Rust; the current app runs on it with the same features.
3. **M2 Registry + packs:** schema DSL, capability export, `EnginePack`/`definePack`, `core.primitives` moved to a pack, placeholder preservation.
4. **M3 Geometry:** parametric archetypes with named parts (`architecture`, `furniture`), CSG openings.
5. **M4 Spatial:** BVH, relations solver, overlap/bounds diagnostics.
6. **M5 AI v2:** registry-generated tools, plan → transaction, server-side dry-run + repair loop, preview/accept UI, eval suite.
7. **M6 Logic + Play:** rule runtime, `interaction` + `scenario` packs, Edit/Play separation, sound, scenario validator + simulated playthrough.
8. **M7 Physics + VR:** Rapier, grab/insert in VR, uikit panels, voice prompt.
9. **M8 Agents:** `engine-agents`, `npc` pack, Recast nav, dialogue, memory, reflection.
10. **M9 Domains:** `education`, `anatomy`, `engineering`; declarative packs + AI-authored templates.
11. **M10 Assets:** catalog with licenses, neural generation provider, rebaseable results.
12. **M11 Ship:** server persistence, hosting, MCP endpoint, demo scenarios, video, submission (deadline 11 Nov 2026, 16:30 CEST per the official terms).

## 19. Open questions to verify

- Recast/Detour and V-HACD under wasi-sdk (expected to work like Manifold).
- Whether to enable C++ exceptions (`eh` libc++ + Wasm exception handling) or keep `-fno-exceptions`.
- Rust panic → JS error reporting across the C ABI.
- Startup time and memory use on the headset browser.
- Worker round-trip latency in Play on the headset; whether cross-origin isolation (for SharedArrayBuffer) conflicts with any asset hosts.
- LLM provider terms, model IDs, rate limits and cost per generated scene.
- Hand tracking vs controllers on the target headset.
