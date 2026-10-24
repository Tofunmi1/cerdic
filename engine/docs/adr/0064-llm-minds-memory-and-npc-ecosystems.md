# ADR 0064: NPC minds, memory and LLM cognition (`ve-npc`)

Status: accepted (owner, 2026-10-24: "an intelligent NPC system with an LLM, memory, etc.; our main moats are here"). Companion to ADR 0063 (navigation and the brain layers); read that first.

## Context

The product's edge is that a model turns plain language into 3D scenes **and working logic**, and that the world it builds can be populated by intelligent NPCs and agents ("autonomous NPC ecosystems" is one of the named directions of the AI builder challenge: intelligent interaction, procedural storytelling, AI-assisted mechanics; the browser is the platform; a backend may run separately). Our advantage is that the same text language that builds the world also describes the people in it, so a model can write the town, its shopkeeper and the shopkeeper's memory in one pass, and the engine can read the world back to the NPC as knowledge.

Evidence from research and from shipped products (2023 to 2026):
- **Memory architecture** (Park et al., *Generative Agents*, 2023): an append-only **memory stream** of natural-language experiences; **retrieval** by a weighted sum of *recency* (exponential decay since last access), *importance* (rated 1 to 10 when the memory is made) and *relevance* (embedding similarity to the current situation); periodic **reflection** that writes higher-level conclusions back into the stream; **planning** from reflections. Reported agents gossip, remember and coordinate. Exact weights are not fixed by the paper; we choose and tune ours with tests.
- **Product lessons** (Inworld, Convai, NVIDIA ACE; secondary sources): an unconstrained model produces nonsense, out-of-context or unsafe lines, so every product wraps it in persona, game-state facts, guardrails and a filter; voice replies need well under about a second to feel alive (figures vary: 200 to 800 ms quoted, excluding speech-to-text and text-to-speech); responses are cached; conversation depth is capped to limit tokens; cost grows with every concurrent NPC and player; the runtime is deliberately independent of any model vendor.
- **In-browser inference** exists (WebLLM: OpenAI-style API, WebGPU kernels, runs in a worker, about 80% of native decode speed) but small models are weak and a headset browser's limits are unverified; so a cloud or local-server model is the default and an on-device model is an optional provider.
- **Shipped AAA games do not let a language model drive the frame loop** (ADR 0063): movement, perception, combat and scheduling are deterministic systems; designers author macros, tags and criteria as data. The language model's natural job is what those systems cannot do: open dialogue, interpreting what a player said, deciding what a character *wants* given what it remembers, inventing a plausible response to the unscripted.

## Decision

An NPC's cognition is **two speeds on one blackboard**, and the language model is a *plug-in provider* behind a strict boundary:

```
  slow (seconds, async, optional)   Mind: LLM deliberation on events -> validated Intents (say, goal, plan macro, feeling, memory note)
                                      |  reads: persona, retrieved memories, world facts, current percepts, allowed actions
                                      v
  shared                            Blackboard (ADR 0063) + MemoryStore (episodic stream, facts, relationships, reflections)
                                      ^
  fast (every tick, deterministic)  Brain: FSM / behaviour tree / utility / HTN / skills (ADR 0063) -> Commands -> nav, animation, game events
```

### Rules (these are what keep it shippable)

1. **The deterministic core stays authoritative.** The model never moves a character, spends an item or changes game state directly. It returns **Intents** drawn from a closed vocabulary the game registered (`say`, `set_goal(name, args)`, `run_macro(name, args)`, `remember(text, importance)`, `set_fact`, `adjust_relationship`, `emote`, `emit_event`, custom by name), each with typed arguments. Anything outside the vocabulary or failing validation is dropped and logged; the brain carries on. An NPC therefore cannot hallucinate an ability the game never gave it.
2. **The game works with no model.** Every Mind has a fallback: canned and templated lines by topic, a scripted-reply table, and the plain brain. Offline, over budget, slow or refused (rate limit, content filter) all degrade to it. A playtest bot, a headless test and a save/replay run with a **scripted provider**.
3. **Determinism and replay.** The Rust core (memory, retrieval, prompt assembly, intent validation, scheduling) is pure and deterministic with a seeded generator. Model calls happen outside it (in the page: `fetch` or an in-browser engine) and re-enter as recorded **responses**; a session can be recorded and replayed bit for bit, which is also how tests and networked clients stay in agreement.
4. **Budgets everywhere.** Per NPC: calls per minute, prompt tokens, memory count (default 512), reply length. Per scene: concurrent calls, tokens per minute, a priority order (the NPC the player is talking to first; nearby and important before far). Over budget means the fallback, never a stall.
5. **Guardrails are data.** Persona, forbidden topics, allowed knowledge ("does not know the king is dead"), speech style, output schema, a content filter hook and an in-character check. An NPC only knows what is in its memory, its persona and the facts the game grants it (**knowledge scoping**), which is how secrets and unreliable gossip work.
6. **Everything an NPC thinks is inspectable.** The retrieved memories, the prompt (size and parts), the raw reply, the validated intents, the dropped ones, the brain state and the reason an action was taken are exported as text for the viewer's mind inspector, the tool calls and the tests.
7. **Memory is saved data**, not hidden model state: serialisable, versioned, capped, forgetting by decay and consolidation, and shared by explicit gossip with provenance (who told whom, how many hops, with confidence decaying per hop).

### Memory (module `memory`, first to build)

Per NPC a `MemoryStore` with four kinds of content:
- **Episodic stream:** text memories with time, place, participants, tags, importance, a `source` (witnessed, told by X with hop count, reflected, imagined), last access and access count, and an embedding.
- **Facts and beliefs:** key to value with confidence and time ("player.name = Ada", "baker.mood = angry"); newer overwrites.
- **Relationships:** per other character `affinity`, `trust`, `familiarity` in -1 to 1 (familiarity 0 to 1), adjusted by events, clamped.
- **Reflections:** memories of kind reflection, pinned (not forgotten), produced when accumulated importance since the last reflection passes a threshold; the Mind asks the model to write them.

Retrieval score = `w_recency * recency + w_importance * importance + w_relevance * relevance`, each in 0 to 1: recency = `0.5^((now - last_access) / half_life)`, importance as stored, relevance = cosine similarity of embeddings clamped to 0 to 1. Ties go to the lower id. Embeddings come from an `Embedder` trait: the default is a **lexical hashing embedder** (deterministic, offline, no model: hashed word and bigram features, stop words removed, L2-normalised), good enough for "bread" to find "the baker gave me bread" and runnable in tests and on a headset; a semantic provider (a cloud or on-device embedding model) can replace it behind the same trait without touching the store.

### Mind (module `mind`)

A scheduler that decides when to think (player spoke to it; a salient event; its brain asks; a reflection is due; an idle tick for ambient life), builds a **prompt** from fixed sections in a fixed order (persona and rules; world facts it is allowed to know; its goals and current state; retrieved memories with ages; recent conversation; the situation; the output schema and the allowed intent vocabulary), hands it to a **provider**, then **parses and validates** the reply into intents. Providers are a trait on the Rust side (`Provider`: request in, response later) implemented by the page: an OpenAI-compatible HTTP provider, an in-browser engine provider, the scripted provider. Streaming text is shown while the intents are only applied when the reply completes and validates.

### Authoring (module `spec`, graph nodes, tools)

A line-per-statement spec in the style of `ve-game` and ADR 0063's spec, written by the model through the same tool calls as everything else:
`npc baker persona="..." speech="warm, brisk" knows="bread,prices,the mill" secrets="owes the miller" goals="sell bread, keep the shop clean" brain=shopkeeper provider=default budget=...`,
`topic gossip ...`, `intent offer_discount args=(percent:number[0..30])`, `relationship baker player affinity=0.2`.
Graph nodes: `npc.persona`, `npc.knowledge`, `npc.brain`, `npc.intent` (registers a game action as something a mind may request), `npc.topic`, `npc.schedule`, `mind.provider`. Tools: `npc_describe`, `npc_say` (as the player, for tests), `npc_memory` (dump or search), `npc_trace` (why did it do that), `npc_script` (queue scripted replies for a headless run). The world is also **grounded knowledge**: a fact source turns the document (named parts, kinds, places, the game's variables) into facts an NPC may be granted.

### Ecosystems (module `society`, later)

Schedules and needs (utility), shared group blackboards and roles (ADR 0063 group layer), **gossip** (an NPC may tell another a memory it chooses, with hops and confidence; both stores update), story sifting over the event stream (find "betrayal", "debt repaid"), simulation tiers (ADR 0063 N8): nearby and important NPCs get the full mind, far ones run needs and schedule only and exchange memories statistically, so a village of fifty is affordable.

## Where it lives

A new crate **`ve-npc`** (pure Rust, wasm-safe, deterministic, depends on `ve-nav` only for the shared blackboard and command types once they exist; until then standalone). Reason for a crate rather than modules of `ve-nav`: navigation is geometry; cognition must not pull geometry into the page's language-model code and vice versa; ADR 0063's brain layers (N5 perception, N6 brains) move here, `ve-nav` keeps space, route, steering and locomotion. This is the second crate of the NPC subsystem, accepted against the few-crates rule because the dependency direction is clean and the two have different change rates.

## Steps (fixed tests first, by Claude; Codex implements one narrow step at a time)

- **M1 Memory** (`ve-npc::memory`): store, facts, relationships, retrieval scoring, lexical embedder, capacity and decay, gossip with provenance, reflection trigger, JSON save/load. Contract: `crates/ve-npc/tests/memory.rs`.
- **M2 Mind core** (`ve-npc::mind`): intent vocabulary and validator, prompt assembler with section budgets, scripted provider, scheduler with budgets and fallbacks, record and replay, conversation state.
- **M3 Brains and blackboard** (`ve-npc::brain`, from ADR 0063 N5/N6): blackboard, skills, FSM, behaviour tree, utility, HTN, registries; the mind sets goals the brain executes.
- **M4 Page bridge (no UI yet):** providers in the page (HTTP, in-browser engine, scripted), runtime exports, and a **UI-agnostic dialogue session model** in `ve-npc` (transcript, streaming text, whose turn it is, speaking and listening state, suggested replies, pending intents) with a stable event stream. The **dialogue and mind-inspector UI is deferred**: the owner has no design yet and will use an Apple Liquid Glass style later; when it comes it only binds to the session model and its events, so nothing built now needs to change. Voice (speech to text, text to speech) is a provider hook behind the same session model, also later.
- **M5 Language and tools:** the spec parser, `npc.*` nodes, `npc_*` tools, world-facts source.
- **M6 Ecosystem:** schedules, needs, gossip in play, simulation tiers, story sifting.
- **Demonstration:** "the village that remembers": a model builds a village from a paragraph, five NPCs with personas; the player gives the baker a gift, tells the smith a rumour; the next day the miller already mentions both, a bot replays the whole session from the recording, and the inspector shows why.

## Open for the owner

Which provider is the default for the demo (a hosted model through our own backend, or the user's key); whether voice (speech to text, text to speech) is in the first demo; the content-safety bar for public worlds; whether NPC memory persists across sessions per player (storage and privacy).
