# Character pipeline: one authoring flow from brief to game asset

Date: 2026-11-10  
Status: proposed roadmap; the audit-driven character workflow has started with a measured head-study pilot. Broad character coverage remains incomplete.

## Goal

Build a character-authoring workflow that lets an agent and a person create, revise, inspect, animate, and export a broad range of game characters: stylized and realistic people, fantasy creatures, quadrupeds, and articulated machines. The system should support a playable character asset, not just a convincing still mesh.

The useful promise is **broad, measured coverage**, not a claim that one prompt or one template can produce every production-ready character. Quality comes from a generator that exposes editable structure, plus tools that reveal and repair the reasons a result fails.

This plan is not based on `body.human`. That is an existing, useful human-specific path and may be one optional starting source. The architecture below has to work for characters that do not use that data or even have human anatomy.

## The central design principle: specify and audit before generating

The ship and Cessna workflows are the right model for this work. A character prompt should not ask the model to imagine a complete asset correctly in one pass. Give it a human-owned brief, frozen measurements, a read-only audit, fixed views and poses, and a single editable character graph. The model is responsible for proposing and revising graph edits until the evidence passes. The concrete local patterns are [`audit-cessna.ts`](../tools/audit-cessna.ts) and [`audit-ship.ts`](../tools/audit-ship.ts): they independently measure geometry, check required outputs, apply tolerances, and point failures at source parameters.

Every benchmark character should have a workspace with the same roles as the existing audited builds:

```text
scenes/characters/<benchmark>/
  brief.md       human-owned intent, visual target, allowed tradeoffs
  spec.json      human-owned dimensions, ratios, topology and runtime budgets
  audit.ts       human-owned measurements, structural checks and actionable hints
  shots.json     fixed cameras for silhouette, proportions, materials and detail
  poses.json     fixed rest/expression/action poses for deformation review
  world.txt      the only scene artifact the model edits
  goldens/       accepted world, audit output and reviewed shot baselines
```

The audit is the fast feedback channel. It prints one compact PASS/FAIL line per frozen rule, including measured value, target/tolerance, and the smallest useful edit hint. A visual review score is the slower feedback channel for things that cannot be represented honestly as a scalar. Neither the LLM nor a second agent may weaken the brief, spec, audit, pose list, or accepted baseline while trying to pass them.

For a character, the spec should be derived from a human-approved concept sheet, reference, or design brief. When the character is invented and has no real-world measurements, the owner sets the target ratios and ranges. The model may help measure a draft, but it must not choose its own acceptance limits. Example rule: `arm_span_to_height` is computed from the left/right fingertip landmarks and total character height; the spec owns the target interval and its hint names shoulder width and upper/lower arm lengths. The audit implementation owns the calculation.

The intended loop is:

```text
brief + spec + operator guide
  -> silhouette and landmarks
  -> one graph edit
  -> parse/compile + structural check
  -> fast measurement audit
  -> pose/deformation audit at milestones
  -> fixed-view capture and human/vision rubric
  -> rerun all affected audits
  -> export/package validation
```

Run cheap diagnostics after every edit; measurements after every meaningful geometry/rig edit; pose sweeps and captures only at milestones. When a visual failure can be converted into a reliable proxy (a missing eye part, a shoulder landmark outside its target region, hair/scalp ray coverage, garment/body intersection), make that proxy a frozen audit rule. Keep image review for silhouette character, appeal, style match, material response, and other judgments the measurements cannot settle.

This ordering changes the implementation strategy: first create character briefs/specs/audits for the existing outputs and see where they fail. Add a modeling or rigging tool only when an audit identifies a concrete gap that the current language cannot repair.

## Research conclusions

Blender’s character workflow is a stack of separate, composable jobs: construct and refine shape; maintain useful topology; deform with bones and weights; add corrective shapes; create materials and surface detail; author motion; inspect deformations and export. Its modifier model is explicitly non-destructive, and its Skin modifier shows how a skeletal graph can become a subdivided creature surface. Voxel remeshing is a sculpting/blockout tool: it can rebuild uniform topology, but it loses mesh layers and does not operate on the multiresolution stack. These are good workflow ideas; they do not mean we should copy Blender’s whole object/modifier/depsgraph architecture.

Unreal’s production workflow reinforces that characters are an asset family, not one mesh: skeleton, skinned surfaces, clothing, morph targets, material sections, LODs, and animation all need to survive import and runtime. Modular parts trade authoring/setup cost against runtime cost and morph support. Hair groom binding and skin-weight profiles are specialized systems in their own right. We should preserve that separation in our data model and choose a runtime representation based on measured needs.

glTF already gives us a practical first delivery contract: skeleton joints and weights, node-transform animation, and morph targets with position/normal/tangent deltas and animatable weights. It does not define the game’s state machine or gameplay behavior. Keep authoring/runtime semantics in our graph and export interoperable clips and meshes through glTF.

### Research sources

- Blender, [modifier stack and non-destructive modeling](https://docs.blender.org/manual/en/latest/modeling/modifiers/introduction.html).
- Blender, [Skin Modifier](https://docs.blender.org/manual/en/latest/modeling/modifiers/generate/skin.html) and [sculpt remeshing](https://docs.blender.org/manual/en/latest/sculpt_paint/sculpting/tool_settings/remesh.html). The remesh manual calls out the topology/data tradeoff and loss of mesh layers.
- Blender, [shape keys](https://docs.blender.org/manual/en/latest/animation/shape_keys/introduction.html).
- Epic, [Skeletal Mesh Editor](https://dev.epicgames.com/documentation/en-us/unreal-engine/skeletal-mesh-editor-in-unreal-engine), [skeletal mesh LODs](https://dev.epicgames.com/documentation/en-us/unreal-engine/skeletal-mesh-lods-in-unreal-engine), [modular characters](https://dev.epicgames.com/documentation/en-us/unreal-engine/working-with-modular-characters-in-unreal-engine), and [hair groom setup](https://dev.epicgames.com/documentation/en-us/unreal-engine/hair-simulation-and-rendering-quick-start-guide-in-unreal-engine).
- Khronos, [glTF 2.0 specification](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html): skins, morph targets, and animation channels.
- libigl, [Bounded Biharmonic Weights tutorial](https://libigl.github.io/tutorial/#Bounded-Biharmonic-Weights), [biharmonic coordinates tutorial](https://libigl.github.io/tutorial/#Biharmonic-Coordinates), and [MPL-2.0 repository](https://github.com/libigl/libigl). This is an algorithm/library candidate to assess for offline weight solving, not a dependency decision. Check the exact code, build surface, WASM cost, and license before adopting any portion.
- Epic, [groom components](https://dev.epicgames.com/documentation/en-us/unreal-engine/groom-components-and-assets-in-unreal-engine) and [groom LODs](https://dev.epicgames.com/documentation/en-us/unreal-engine/setting-up-level-of-detail-for-grooms-in-unreal-engine): strands, cards, and mesh representations serve different cost/quality levels; Epic’s docs say its renderer does not itself provide a full hair grooming authoring solution.

## What is already in this repository

Treat this list as a starting inventory, not proof that the whole user-facing workflow is complete.

- `creature.rs` and its templates can generate biped, quadruped, and serpent field-based bodies, bones, basic motion, and graph text. Its own ADR says this is procedural geometry, not artist-authored anatomy or production rig quality.
- `body.human` uses the separately packaged CC0 human model data, phenotype shapes, landmarks, skeleton, and skin weights. `body.eye`, `body.bone`, `body.skin`, `body.garment`, and `body.outfit` add specialized human features.
- General rigging includes skeleton/pose nodes, distance-derived `rig.skin`, dual-quaternion deformation, painted weight stamps, deterministic IK, and `rig.morph` equal-topology targets. Current shape-key export includes glTF position and normal deltas.
- Polygon sculpt operations, subdivision, UV/corner attributes, texture graphs, material baking, and glTF export provide reusable lower-level capabilities.
- Character appearance has both procedural and packaged paths for hair and clothes. The existing handoffs document visible shortcomings: hair coverage/strand quality and garment fit/folds need review.
- The current node system and text language are the correct shared formal flow. Character tools should compose with them, not create a parallel scene format or opaque one-shot generator.

The main gap is integration and generality: anatomy authoring, topology conversion, rig semantics, appearance, animation, and game-runtime setup do not yet behave as one inspectable character asset workflow.

## Proposed architecture

### 1. Keep one formal graph and add a character authoring layer

The language model should edit a compact, typed character recipe that expands into ordinary graph nodes. The expanded graph remains the source of truth and stays editable. Do not create a second unrestricted modeling language.

Introduce a versioned `CharacterSpec`/character group interface with named semantic roles:

- **identity and style:** species/archetype, silhouette, proportions, stylization, age range where appropriate, and visual reference constraints;
- **forms:** torso, head, face, hands, feet, limbs, appendages, armor, cloth, hair, and accessory groups;
- **landmarks and measurements:** named joints, surface landmarks, bounds, ratios, symmetry planes, and attachment sockets;
- **deformation:** skeleton hierarchy, skin weights, morph targets, corrective shapes, and deform groups;
- **appearance:** material regions, UV policy, texture graphs, hair/cloth surface models, and render tags;
- **animation and game use:** named clips, semantic bone map, hit/interaction regions, sockets, root motion policy, and optional LOD budgets.

The spec is authoring metadata, not a new mesh format. It lowers to registered `character.*`, `rig.*`, `body.*`, `poly.*`, `solid.*`, `curve.*`, `material.*`, `anim.*`, and `game.*` operators. Each high-level operator must expose inspectable outputs, deterministic limits, estimates, diagnostics, and stable IDs. The user can replace a generated piece with lower-level nodes without losing the rest of the character.

### 2. Use a hybrid geometry pipeline

No single representation is right for every character task. Use each for the stage it handles well:

1. **Blockout:** semantic landmarks plus curves, tubes, blended fields, and primitive forms establish the silhouette quickly. This is where LLM iteration is strongest.
2. **Surface refinement:** editable polygon meshes, patches/lofts, subdivision, and sculpt strokes add facial planes, muscles, armor breaks, hands, and creature-specific forms.
3. **Topology transition:** remesh/retopology creates a controllable deformation mesh when the blockout topology is unsuitable. Preserve/reproject UVs, material regions, masks, landmarks, and sculpt detail explicitly; refuse or warn when a layer cannot be transferred reliably.
4. **Detail:** retain high-resolution sculpt/detail separately from the game mesh; bake normal, AO, curvature, and material signals to the lower mesh. Avoid making every detail a runtime triangle.
5. **Assembly:** body, eyes, teeth, tongue, hair, clothing, armor, and accessories may be separate meshes where that improves editing, materials, rigging, or LOD. Provide a final pack/merge operation only when its morph/skin/material constraints are satisfied.

The Blender Skin Modifier is a useful concept for turning a branch graph into a coarse mesh, but it is a blockout generator, not a universal character surface. Field meshing is similarly useful for organic unions but should not be mistaken for clean deformation topology.

### 3. Make rigs semantic and shareable

Define a `SkeletonSpec` with stable semantic roles rather than hard-coding one human skeleton: root, spine chain, neck/head, limb chains, digits, tail, wings, jaw, eyes, and optional custom joints. Species templates map roles to concrete bones. Keep a canonical biped map for retargeting, then allow extra bones without breaking the base map.

Build one rig pipeline around:

- auto-rig templates for humanoid, quadruped, serpent/tail, winged, and multi-limb forms;
- measured rest-pose validation (bone direction, connected chain, mirrored pairs, bounds, scale, and naming);
- automatic weights followed by normalization, influence pruning, symmetry, weight transfer, and the existing paint edits;
- deformation previews for a pinned pose suite, with region-specific error reporting;
- corrective morphs for shoulders, elbows, hips, knees, wrists, jaw, and other high-stress joints;
- IK constraints and poles, limits, target orientation, and multi-chain conflict handling;
- animation retargeting through semantic bone maps, not equal-name assumptions.

For automatic weights, compare distance falloff, heat diffusion, and bounded biharmonic weights on the same test meshes. Bounded biharmonic weights are a constrained smoothness optimization over the skeleton handles, with bounded non-negative weights that sum to one; they are a stronger quality candidate than plain distance weights, but they add a sparse optimization problem. Use a fast distance/heat initialization and a sparse solve where it demonstrably improves joint deformation. Keep weight generation deterministic, capped, cancellable, and reproducible in WASM. A native-only solver may be an optional authoring accelerator only if saved weights become stable scene data and browser evaluation does not need that solver.

### 4. Treat the face, hands, hair, clothing, and materials as first-class subsystems

- **Face:** skull/jaw/cheek/nose/ears from landmarks and surface patches; separate eyes, lids, brows, teeth, and tongue; expression targets organized by named controls; gaze and jaw motion. Provide stylized and realistic presets, but expose their geometry and controls.
- **Hands and feet:** palm/sole volumes, digit chains, knuckle landmarks, nails/claws/hooves/pads, and pose sets. Validate silhouette and joint bends in grasp, fist, spread, and foot-contact poses.
- **Hair/fur/feathers:** a strand/groom representation authored from scalp or body surfaces, with guides, clumps, taper, curl, length, flow, density, and deterministic seed; generate strand, card, and mesh LODs for the runtime budget. Make caps, roots, and hairline explicit; test scalp coverage and face clearance. Epic separates authoring, groom binding, rendering and simulation, and supports strand/card/mesh LOD representations; this is a useful architecture to adapt. The existing ribbon generator and packaged styles remain one route, not the whole design.
- **Clothing/armor:** separate layers with pattern panels, seams, thickness, closure/attachment points, skin weights, and collision proxies. Static fit first; optional deterministic cloth settling/simulation later. Skin-weight transfer and pose stress tests must be part of the workflow.
- **Materials:** continue using the procedural texture graph; add character material recipes (skin, eye, hair, leather, cloth, metal, bone/scales) with authored UV policy, tiling, tangent-space normal detail, and baked maps. A material preset must never be a substitute for form.

### 5. Make animation and game readiness part of the asset

Use the existing animation graph/state machine and Bevy runtime. Add named clip authoring and blending, a semantic retarget path, locomotion sets, action clips, face controls, attachment sockets, and optional root motion. Connect animation to the game graph with stable gameplay handles: capsule/shape collision, hit regions, interact points, equipment sockets, movement profile, and controller metadata.

The game-ready output is a character package containing graph source, mesh parts, skeleton, skin weights, morph targets, materials and maps, animation clips/state setup, sockets, collision metadata, LODs, and validation results. Export standard geometry/skin/morph/clip data through glTF; keep state-machine and gameplay semantics in our own package because glTF does not specify them.

### 6. Make the audit the agent's high-frequency eyes

For each character workspace, freeze `brief.md`, `spec.json`, `audit.ts`, `shots.json`, `poses.json`, and the accepted baseline as human-owned artifacts. The agent edits only `world.txt` (or the graph document it represents). Protect those files with a separate review step if multiple agents are used.

Use spec rows with stable IDs, a measured expression, target range/tolerance, units, and an actionable hint. For example, a humanoid spec can include shoulder-to-height ratio, arm span-to-height ratio, head-to-height ratio, hand length, foot length, left/right landmark error, and triangle/material budgets. A quadruped spec should instead include withers height, body length, leg proportions, paw/hoof dimensions, stance width, and tail/muzzle landmarks. These are archetype-specific references, not universal human proportions. Keep the ship/Cessna format where each rule is calculated independently and reports its likely source parameter, rather than hiding multiple failures behind a single opaque score.

The audit should cover four kinds of evidence:

1. **Measured form:** dimensions, ratios, named landmarks, symmetry, silhouette widths at fixed heights, part presence, attachment alignment, and triangle/texture budgets.
2. **Structural validity:** expected semantic roles, connected hierarchy, valid weights, normalized influences, mesh/UV/material presence, and exportable data.
3. **Pose validity:** fixed joint configurations; per-region drift, volume loss, self-intersection or garment/body penetration, eye/lid separation, hair coverage, and foot/ground contact as appropriate.
4. **Visual review:** fixed front/side/three-quarter/detail views and a pinned-pose sheet scored against a frozen rubric. Keep the owner’s score separate from automated image metrics.

Prefer measurements from the evaluated model and named part/landmark APIs; do not make an OBJ dump the permanent audit interface. The ship/Cessna scripts prove the value of independent geometry measurement and parameter hints. The character implementation should generalize that pattern to rig, topology, materials, pose and package data. Every failure line should identify the measured value, allowed interval, likely source parameter/role, and next smallest edit.

Follow the demonstrated edit discipline: state the failing rules, make one hypothesis and one bounded change, rerun parse/check and affected rules, then rerun all rules at milestones. Keep a fixed iteration cap and report raw audit lines. Do not claim success from a nice neutral render if its frozen pose or size rules fail.

## Implementation roadmap

### Phase 0 — Audit what exists before designing more tools

Create brief/spec/audit/shots/poses workspaces for the existing human and procedural-creature paths. Audit at least one generated output from each route. Separate *implemented*, *measured*, *visually reviewed*, and *known to work in the game runtime*. Do not infer quality from operator names or passing unit tests.

Use the Cessna/ship lessons directly: include measurable source-of-truth targets and explicit edit hints; compute independent geometry values from the evaluated output; assert required outputs are present; and keep estimated/reference quantities at appropriately wider tolerances. Add character-specific measurements rather than applying one universal character template.

Create a fixed benchmark gallery of four targets:

1. stylized humanoid action/RPG character with face, hands, layered clothing, hair, and 5 game poses;
2. realistic human NPC with neutral anatomy, face expressions, and walk/run/turn poses;
3. non-human quadruped with muzzle, paws/hooves, tail, and locomotion;
4. articulated fantasy/robot character with armor pieces, attachments, and a non-human rig.

All targets need a brief, frozen measurable spec, independent audit, fixed views, pinned poses, triangle/texture/rig budgets, and visual scoring. Keep human scores owner-reviewed; machine image metrics are diagnostics, not acceptance. Start with existing tools and record failures before adding operators.

**Done when:** each existing route has a reproducible run, concise verbatim PASS/FAIL audit output, fixed captures, a pose report where rigging exists, and a ranked list of failures tied to candidate parameters/tools.

### Phase 1 — Audit-backed vertical slice and inspectable graph lowering

Choose one target (the stylized humanoid action/RPG character) and make its frozen workspace the first full loop. Implement the versioned semantic spec, validation, graph expansion, stable naming, audit-facing measurement API, and inspector summaries. Round-trip it through the existing language. Start with silhouette geometry and semantic landmarks; add the rig and additional character systems only as its audit exposes gaps. The semantic spec and graph are tools to satisfy the audit, not a new project goal in themselves.

**Done when:** a model can read the frozen brief/spec, make a sequence of targeted graph edits, pass all deterministic rules, produce fixed-view evidence, and export a valid package; editing one named role leaves unrelated nodes stable; bad role references give actionable diagnostics; native/browser evaluation agree.

### Phase 2 — General rig and weight pipeline, measured by pose suites

Use frozen pose audits to complete the still-open plan items: compare automatic-weight algorithms, rig examples, IK limits/poles, corrective shape support, and deformation review. Then add the shared species-agnostic skeleton map, weight symmetry/transfer/normalization/pruning, and reusable pinned pose suites. Every proposed rig improvement must identify the failing pose/rule it fixes and pass the other pinned poses.

**Done when:** humanoid and quadruped rigs can be authored from semantic landmarks; weight sums and influence limits hold; each pinned joint pose reports clipping, collapse, or volume error; weights round-trip and deform identically in browser and export.

### Phase 3 — Robust anatomy and topology tools

Combine the existing field/curve/sculpt/poly system with branch-skin blockout, multi-resolution or equivalent coarse/detail storage, bounded voxel remesh, attribute reprojection, and explicit retopology/patch tools. Add high-to-low bake and hard limits before increasing mesh density.

**Done when:** each benchmark can move from blockout to a stable deforming mesh without losing required regions, UVs, materials, or rig metadata silently. Unsupported transfers must refuse with a precise diagnostic.

### Phase 4 — Face, extremities, and species forms

Build reusable landmark-driven modules for head/face, hands, feet, tails, wings, muzzles, ears, claws/hooves, and armor. These are editable recipe groups, not frozen presets. Add face-expression controls and digit/jaw rigs.

**Done when:** the humanoid and quadruped benchmarks pass silhouette and feature checks from front/side/three-quarter, and the face/hands survive their pinned expression and pose suites.

### Phase 5 — Hair, garments, and character materials

Improve groom guides/card generation and scalp coverage; add clothing pattern/volume/thickness and pose-aware fitting; add weight transfer and collision checks; use the texture graph for character-specific material recipes and bakes. Keep CC0 packs as opt-in starting assets and record provenance; generated forms must work without them.

**Done when:** hair has no scalp holes in the fixed views; clothing avoids the body across pinned poses within a declared tolerance; UVs and material regions export; baked maps pass image and glTF checks.

### Phase 6 — Animation and playable package

Finish named clip authoring, retargeting, locomotion/action state setup, morph animation, sockets, collision and game-controller metadata. Build the export/package step and Bevy runtime importer/consumer. Generate LODs that preserve silhouette, joints, weights, and morph behavior.

**Done when:** all four benchmark characters load in a small Bevy gameplay scene, can transition through their required clips, interact with attachments/collisions, and pass package round-trip validation.

### Phase 7 — Quality expansion, not a false universal claim

Add archetypes based on failed prompts and review evidence: stylized proportions, muscular anatomy, elderly/child proportions where appropriate, long ears/horns, extra limbs, wings, tails, scales/fur/feathers, hard-surface robots, and modular equipment. Grow the capability matrix by archetype and body system.

Call the pipeline broadly capable only when it passes a frozen prompt set across these archetypes, needs limited edit iterations, exports runtime-valid assets, and has human visual review for each category. Maintain a list of unsupported forms rather than saying “anything” without evidence.

## Acceptance and stop conditions

Every phase should report four separate outcomes:

- **Can generate:** the spec and graph produce the requested structures.
- **Can edit:** a person/model can change a named region without rewriting the whole character.
- **Can deform/play:** animation and gameplay checks pass across the pose/action suite.
- **Can ship:** package limits, exported format, materials, LOD, and runtime load are validated.

An attractive neutral render is not enough. A character is complete for its stated target only when it reads correctly in fixed views, deforms without obvious failures in required poses, exports and reloads, fits declared budgets, and includes the required animation/gameplay metadata.

## Immediate next work

1. Retire `head_study/world.txt` as a character-generation attempt. Keep its frozen brief/spec/audit/captures only as evidence that landmark ratios and triangle counts do not make a convincing face.
2. Stop synthesizing a human face from loft rings and broad displacement fields. The graph is a good assembly/edit language; it is not an anatomy authoring method.
3. Build the next character E2E around a strong, topology-preserving source mesh and explicit authored variants. Compare source quality, topology, UVs, rig, morph coverage, export rights, browser size, and runtime compatibility before adopting one.
4. Make graph nodes operate on semantic regions and preserve source topology/UVs/skin weights unless a deliberate remesh/bake stage is requested. Add new nodes only where the chosen source asset fails a measured requirement.
5. Keep the ship/Cessna audit loop: freeze a character brief, independent measurements, fixed views, pose suite, budgets, and export checks around the assembled asset. The audit is the quality gate, not a substitute for strong source forms.
6. Resolve plan drift: `plan-animation-and-blender.md` still has older B3 wording that says linear blending while its W8 summary and current operator spec say dual-quaternion blending is done. Reconcile before using that checklist as evidence.

## Current audit-driven pilot

The first runnable measurement loop is `engine/web/tests/scenes/characters/head_study/`. Its `world.txt`, brief, frozen 12-rule spec, independent mesh audit, fixed camera list, pose coverage list, run command, capture script, and visual-review sheet make one complete edit/measure/view loop. The initial mesh failed four proportion checks; targeted geometry edits brought the measured rules into range. The loop also caught excessive eye protrusion and now measures it. No target was loosened. The head uses subdivision level 3: 172,800 head triangles and 175,552 head-plus-eye triangles against a 200,000-triangle cap.

The fixed views show that this is a poor character model even though its measured proportions pass: the face reads toy-like, the eye/socket and nose/cheek forms are weak, and it has no hair, expression poses, or body. Treat it as a negative example and an audit-tool smoke fixture, not as the seed to keep polishing. The existing CC0 human pack is a separate source asset path; its existence does not mean the hand-built head used it or that the pack alone reaches the desired art quality.

## Pipeline pivot: asset source first, graph as editor/compiler

Do not promise arbitrary high-fidelity human generation from a text graph. Start with a target art direction and a licensed production-quality base mesh (or a small authored set of archetype bases). The graph should assemble and non-destructively adapt those meshes: choose body/face variants, apply bounded shape changes, attach authored gear and garments, layer material/texture recipes, preserve UVs and weights, then validate and export. For fantasy characters, identity and silhouette come from designed proportions, costume, hair, armor, and material breakup—not from adding more ellipsoids to a generic face.

Before adopting a source, render it in the actual engine and measure its real data: silhouette at fixed views, facial readability at portrait scale, topology/UV quality, rig and morph coverage, package size, license/provenance, and browser export round trip. If the source cannot pass that review, reject it before writing more adapters. The first successful E2E should be one distinctive, complete RPG character made from a proven base plus layered authored parts, not a promise to generate every kind of human from scratch.

## Scope boundaries

- No headset work is required for this plan.
- No general game-maker work is implied; the scope is character assets that work in the existing Bevy game workflow.
- Do not port Blender’s object/dependency-graph implementation wholesale. Implement the useful modeling concepts behind our typed graph and preserve our engine’s deterministic limits and cancellation.
- No new third-party character data is required. Existing CC0 human/outfit data can be used as a baseline or optional fitting source, but the system must also generate non-human and original forms.
- Treat library adoption as a separate decision: inspect exact version, license, native/WASM support, build size, determinism, and maintenance cost before adding it. Blender code has GPL obligations; concept-level study does not require copying its implementation.
