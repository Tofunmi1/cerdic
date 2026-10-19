# Handoff: after G9. Roadmap, modeling extensions, procedural textures, animation

Date: 2026-10-15. Deadline 11 Nov 2026 16:30 CEST (27 days). Read first: `handoff-common.md` (rules; `cargo xtask quick` while working, the content-cached
`cargo xtask check --strict` once at the end), `docs/plans/track2-modeling.md` (milestones table near line 246, G10 near 438, L0 to L9 near 500),
`docs/modeling/logic.md` (the logic and animation design), `handoff-g9-fidelity-and-ai.md` (the G9 package list this builds on),
`handoff-l0-l1-logic.md`, `docs/modeling/limits.md`. Nothing here is committed; ADRs stay Proposed until the owner accepts them. Do not cut scope for
schedule reasons; the order below is chosen so the most visible and the most blocking work lands first.

## 1. Where the project stands (verified 2026-10-15)

Done and gated: G0 to G8 (documents, typed graph, groups and instances, surfaces and deformers, polygon tools, subdivision with UV and float channels,
bounded spatial fields), V0 to V4 (editor, outliner, inspector, node graph, viewport editing, groups), M1 to M8 and M11 (runtime, XR, interaction).
In progress: **G9** (fixes A1 to A5 done; lights B1 started; ambient occlusion, procedural materials, rounding, post effects, composition, visual review and
the whole AI evaluation open).
Not started: G10, L0 to L7, G13. Optional and unstarted: G11, G12, V5 to V7, L8, L9. Measured on a real headset: nothing (M9 is prepared, needs the owner
and a Quest). Everything rendered so far is SwiftShader.

Remaining core release (plan): G9, G10, L0 to L7, G13. This handoff adds three extension tracks that the plan only lists as deferred or wanted:
a modeling-tools batch ("G10b"), procedural texture generation (the deep half of G10), and animation beyond `act.animate` (an "L-A" track).
Record the scope additions in ADR 0024 (the plan says changing a decision needs an ADR) and update the milestone table.

## 2. Roadmap and parallel tracks

The owner runs several agents at once, so these tracks are independent in code. Suggested order inside each track, and what blocks what:

| Track | Work | Needs | Why this order |
|---|---|---|---|
| Render and appearance | finish G9b (B1 lights, B2 AO bake, B5 materials, B6 rounding, B3/B4 post, B8 review) then **G10 procedural textures and gallery** (section 4) | G9a done | biggest visible gain; blocks the demo look |
| Logic and animation | **L0, L1, L2, L3** (section 5), then clips and procedural motion (L-A), then L4, L5, L6 | G4 (done) | animation and interactivity are the "VR" part of the demo |
| Modeling kit | **G10b**: placement, LOD, hull and slicing, profiles and curves, fillet, expressions, kits (section 3) | G8 (done), G9b B6 | fills scenes quickly; many small independent operators |
| AI and tooling | G9c harness, `snapshot`, `measure`, `get_operator_help`, guide trimming, DeepSeek runs, later L7 | provider credit (DeepSeek only works today) | feedback loop for every other track |
| Owner | M9 Quest run, ADR acceptance, scoring the visual rubric, provider credit for Opus and GPT | headset | decides tier defaults for lights, textures and animation counts |
| Last week | G13 release hardening, compatibility report, integrated demo | everything | do not start earlier than the last 7 to 8 days |

Integrated demo target (use it to order priorities and to test the tracks together): a person types or says "a cosy study with a desk, a bookcase, a sofa and a
window", the engine builds it with real lighting and procedural materials, then in Play the door opens when grabbed, a desk lamp switches on and lights
the desk, a ceiling fan spins, and a small creature walks across the rug. Each track contributes a visible piece of that.

## 3. Extending the modeling tools ("G10b")

### 3.1 The pattern for adding an operator (follow it exactly; G6, G7 and G8 did)
1. Spec in `ve-graph/src/builtin/<family>.rs` (new file per family to stay under 400 lines): name and version, port types, parameter types with units,
   ranges and defaults, help text, and the cost model; regenerate the schema (`bun run schema:generate`).
2. Evaluator in `ve-modeling/src/<family>/` (new module), typed refusals with stable codes added to `docs/modeling/diagnostics.md`, cancellation polls.
3. Plan estimate in `ve-modeling/src/plan/estimate.rs` (additive and bounded; never multiply chained operators, see G9 A1) plus an after-the-fact check on the
   real triangle count.
4. Tests: analytic or reference cases, refusals, save/reload/undo/group/instance, determinism, and at least one `crates/ve-modeling/examples/corpus.rs` line so
   `cargo xtask parity` proves native equals WASI.
5. MCP guide block (short) and one MCP test; a gallery fixture with six views; limits and cases rows; report.
Every operator declares version, units, defaults, ranges, preconditions, attribute rules, determinism, preview and final policy, cost and limits.

### 3.2 Batches, in priority order (all deterministic, all procedural, nothing imported)
- **Placement (biggest payoff for scenes).** `inst.scatter` (seeded points over a surface or a volume, density or count, minimum spacing by Poisson-disc,
  slope and height filters, region masks), `inst.along_curve` (count or spacing, align to tangent, up vector), `inst.jitter` (position, rotation, scale
  variation from a seeded stream), `inst.on_grid_of` (grid on a surface patch). All produce placement sets that share one prototype (the existing `Instances`
  type), so cost stays low. Caps: 16,384 placements per set today (`limits.md`); measure before raising.
- **LOD and decimation.** The kernel already has a mesh simplifier (`ve_simplify`, meshoptimizer) with no operator. Add `solid.decimate` (ratio and error
  targets, preserves boundary and material regions where it can) and `inst.lod` (an instance set with N prototypes chosen by distance, evaluated at draw
  time, not by the kernel). Needed for Quest budgets.
- **Wrappers over Manifold.** `solid.hull` (convex hull), `solid.split` and `solid.trim` (by plane), `solid.slice` (cross-section to a profile),
  `solid.project` (silhouette), `solid.minkowski` (convex pieces first; `third_party/manifold/src/minkowski.cpp` is vendored), `solid.shell` (hollow with
  thickness; offset-based). Each needs a size and cost limit and a refusal for non-manifold input.
- **Profiles and curves.** `profile.fillet` (rounded polygon corners, first), `profile.arc`, `profile.ellipse`, `profile.star`, `profile.regular`,
  `profile.rounded_rect`; `curve.arc`, `curve.helix`, `curve.spiral`, `curve.spline` (Catmull-Rom through points), `curve.offset`, `curve.resample`,
  `curve.fillet`. These feed existing sweep, loft and revolve directly. No text-to-profile until there is a procedural stroke font (fonts are imported assets).
- **Rounding and bevel.** Finish G9b B6: `solid.round` and `solid.chamfer`, extend `poly.bevel_edges` to edge networks (G6 limits it to disjoint edges, so a
  box cannot be fully rounded), and an experiment on field-based rounding (`field.offset`).
- **More deformers.** `xform.shear`, `xform.wave`, `xform.noise_displace` (uses the deterministic noise from G8), `xform.lattice` (free-form deformation on a
  bounded lattice). Position-only deformers keep triangle counts, so estimates stay cheap (G9 A1 already handles that).
- **Fields (G8 follow-ups).** Noise, displace, twist and bend with our own deterministic math, regions, `field.from_solid` (distance to a mesh; needs a BVH,
  measure first), and terrain presets (plane plus noise plus domain warp).
- **Terrain and landscape.** `solid.heightfield` from a noise or field expression (grid mesh with normals, bounded size, LOD rings), plus scatter masks
  (slope, height, near-water). A terrain demo plus scattered rocks and trees is the "world" showcase.
- **Parameter expressions and measurement.** Parametric models need relationships ("table height equals leg height plus top thickness"). Check whether `math.*`
  nodes or parameter expressions exist (I could not confirm); if not, add a bounded pure expression type (numbers, vec3, a small function set, no loops, no
  user code, per logic.md's rule of no user code) plus `measure.bounds`, `measure.volume`, `measure.count` nodes that expose numbers to parameters.
- **Kits (groups built from existing operators, shipped in the library panel; no new Rust).** Architecture (wall with openings, door, window, stairs,
  railing, roof, column, arch, dome), furniture (table, chair, shelf, bed, sofa, lamp), props (books, mug, plant, fence, crates), mechanical (gear, spring,
  bolt, pipe run), vehicles (wheel assembly). Each kit has typed parameters, sockets for attachment, a thumbnail view, and an acceptance render. Domain packs
  (escape room, anatomy) are collections of kits plus logic templates, as the project goals say.

### 3.3 What not to promise
Sculpt brushes, automatic retopology, exact B-rep CAD, imported models and good topology for rigging are deferred by the plan. Say so in the guide so a model does
not try them.

## 4. Procedural texture generation and appearance (G10 and the deep half of G9b B5)

Principle: nothing imported; every texture is generated from our own graph, deterministically, within stated budgets.

### 4.1 Two delivery modes (build both; they share one recipe language)
- **Shader-side procedural (G9b B5).** Recipes evaluated in the fragment shader from object-space 3D noise; no UVs; costs fill rate; per-instance variation for
  free. Best for desktop and for simple recipes on Quest.
- **Baked texture sets.** The evaluator generates small tiling PBR sets on the CPU (albedo, normal, roughness and optional height or AO, 128 to 1024 squared,
  8-bit, with mipmaps) and the runtime uploads them as WebGL2 2D textures (an atlas or a texture array, budgeted). Costs memory instead of fill rate and is the
  right Quest path for rich materials. Needs a way to sample: triplanar or box projection in the shader (no UVs needed), or UVs from generators (G7 carries
  `u`/`v`; a unwrap tool is X4/xatlas, optional).

### 4.2 A texture graph (new value type `Tex2`, bounded like the field tape)
- Generators: `tex.noise` (value, gradient, fbm with octaves, lacunarity, gain, seed), `tex.voronoi` (cells, distance, edges), `tex.gradient`, `tex.checker`,
  `tex.bricks`, `tex.tiles`, `tex.stripes`, `tex.grain` (wood ring and fibre model), `tex.weave` (fabric), `tex.scratches`, `tex.speckle`.
- Operators: `tex.mix`, `tex.blend` (multiply, add, overlay), `tex.warp` (domain warp by another texture), `tex.threshold`, `tex.invert`, `tex.levels`,
  `tex.colorramp` (stops), `tex.blur` (bounded radius), `tex.transform` (scale, rotate, offset), `tex.height_to_normal` (Sobel with strength), `tex.curvature`
  and `tex.ao` (from the baked per-vertex AO or a height map), `tex.tile_seamless`.
- Outputs: `material.pbr(albedo, normal, roughness, metallic, height, emissive, scale, projection)` producing a `Material` value that `solid.material` accepts
  (today `solid.material` takes plain numbers; keep those parameters working).
- Budgets and determinism: instruction cap per texture (reuse the field tape design: a bounded tape lowered in Rust, run in C++ with no per-sample JS call),
  resolution caps, total texture memory per scene (propose 64 MiB on Quest and 256 MiB on desktop; measure), a refusal code for each; integer-hash noise and
  our own sin/cos only (the libm trap from G8: native and WASI math differ); native versus WASI parity on corpus lines (compare a hash of the pixel buffer).

### 4.3 Materials people and models actually need
Wood (planks with per-plank tint and grain), painted wall (subtle noise, roller texture), fabric (weave plus fuzz), leather and stipple (the pistol grip),
brushed and worn metal, concrete, brick, marble, glass and water (see 4.5), dirt and wear (masks from AO and curvature: darker in crevices, lighter on exposed
edges), emissive screens. Provide a recipe library (a kit-style library of `Material` groups) and a **material editor panel** in the inspector (plan G10),
with live preview on a sphere and the selected part.

### 4.4 Per-instance and per-region variation
Hue, brightness and roughness jitter from a hash of the instance index (a forest or a bookcase does not look copy-pasted); material regions (ADR 0018) map a
different `Material` to each region of one solid; decals (`decal.box` projected stains, labels, posters; bounded count) later.

### 4.5 Transparency and special surfaces (pipeline gaps to scope first)
Glass and water need alpha blending or refraction, sorted drawing and a depth policy; today materials carry `alpha` only as the base colour's fourth channel and
there is no sorting. Scope it: simple alpha-blended glass with fresnel, then water (animated normal from time-driven noise, depth tint, sun glint). These are
renderer work (`ve-render`), tier-gated, and must be measured on the headset.

### 4.6 Acceptance for G10 (plan text plus these)
Core fixtures show material boundaries, seams, mirrored cases and follow-up edits correctly; all survive save, reload, undo and redo; the supported recipes and
limits are published with screenshots; the study room, pistol, creature and a terrain scene are re-rendered with the B8 rubric and the owner scores them;
texture memory and generation time are recorded; native versus WASI pixel hashes match.

## 5. Animation and logic (L0 to L7 from the plan, plus an animation layer the plan only hints at)

### 5.1 What the plan already fixes (do not redesign; see `logic.md`)
Logic lives in the node language (ADR 0008): events, flow, `act.*`, `state.*`; a text `rule` form compiles to nodes. Play clones the document into a world and runs a
fixed 60 Hz integer-tick interpreter on the window thread under a step budget; the geometry worker only does rebuild jobs. **Per-frame animation never runs the
geometry kernel.** Every action parameter has a class: **Live** (instance transform, visibility, material parameters, part pivot angle: applied to the play world
directly every frame) or **Rebuild** (anything that changes geometry; a discrete action through the worker, coalesced to 4 per second per node; animating one is
refused with `logic.rebuild_hot`). Determinism: seeded random streams, stable iteration order, input log, per-tick state hash, a pure `libm` (X9) for
trigonometry. Milestones: L0 contracts and schema 2, L1 IR and validation, L2 pure `ve-logic` play runtime, L3 interaction bridge and Edit/Play toggle, L4 rebuild
actions, L5 flow editor and replay debugger, L6 scenario validator, L7 AI authoring. Experiments X8 (interpreter cost and hash stability) and X9 (deterministic
math) come first. Start with `handoff-l0-l1-logic.md`.

### 5.2 Animation layers to add on top (all Live-class unless stated; each needs a fixture before a gate)
- **A1. Tweens and curves in `act.animate`.** Easing set (linear, ease in/out, back, bounce, spring with fixed constants), `to`, `from`, `delta`, `loop`,
  `ping_pong`, awaited or not, cancel and replace policy. Targets: part or instance transform channels (translate, rotate about a named pivot, scale),
  visibility, material parameters (colour, emissive, roughness), light parameters (intensity, colour), camera.
- **A2. Clips and tracks.** `anim.clip@1`: named tracks of keyframes (time, value, interpolation) over live parameters, with length, loop mode and speed;
  `act.play_clip`, `act.stop_clip`, crossfade between clips (a weighted blend of track values), events at times (a track of triggers). Stored in the document,
  evaluated per tick from integer ticks (no wall clock). Edit with a **timeline panel** (new optional V8: tracks, keys, scrub, drag keys, curve editor),
  and the AI can write clips as text.
- **A3. Procedural motion nodes (pure functions of time and state).** `motion.oscillate` (sine/triangle with amplitude, frequency, phase; our own sin),
  `motion.spin`, `motion.bob`, `motion.noise` (smooth noise sway for wind, flicker for flames and lamps), `motion.follow_path` (a `Curve3`, speed, loop,
  orient to tangent), `motion.look_at`, `motion.spring_follow` (critically damped follow with a fixed time step), `motion.orbit`. A fan, a flag, a flickering
  lamp, a patrolling guard and a bobbing pickup each need only these.
- **A4. Rigid hierarchies and joints.** Parts already have named pivots and sockets and `xform.attach`; make attachment live in Play so children follow
  animated parents, and add joint types with limits that logic or the user can drive: `joint.hinge` (door, lid), `joint.slider` (drawer), `joint.spin` (wheel,
  fan), `joint.ball` (look). Constraints: `constraint.aim`, `constraint.copy`, `constraint.follow`. Interactive grabbing can drive a joint within its limits.
- **A5. Skeletons and skinning (deferred by the plan; define the path, build only after A1 to A4 and the M9 result).** `rig.skeleton` from a curve or from
  sockets (bones with parents and rest pose), `rig.bind` with **procedurally computed weights** (capsule or envelope distance falloff, normalised, at most 4
  influences; fields from G8 can supply region masks), GPU skinning in the forward shader (WebGL2: a bone matrix uniform block, cap about 64 bones; one extra
  vertex attribute pair for indices and weights), CPU fallback for tests. Motion sources: clips (A2), procedural gait (`motion.walk_cycle` with two-bone IK for
  legs, deterministic), look-at for heads. Fixture: the G8 creature (tube legs map naturally to bones). Be honest in reports: weights are procedural, not
  painted, and quality is stylised.
- **A6. Particles and effects (optional).** CPU or instanced billboard particles with bounded counts and deterministic seeds (smoke, sparks, dust motes in a
  window beam, steam), driven by events. Counts and fill-rate are Quest-sensitive; tier-gated and measured.
- **A7. Physics (deferred; do an experiment first).** The plan lists physics events as deferred. If wanted: an X-experiment with a standalone `rapier3d` for
  rigid bodies and simple colliders (native versus WASM size, cost per body, **determinism across native and WASI**, and whether it can run inside the 60 Hz
  fixed tick). Kinematic bodies first (doors, elevators), dynamic props second. Not part of the core release.
- **A8. Cinematics.** A camera clip (A2 applied to the camera) with cuts and eases for demo capture and for guided tours; plays in Play and in the editor.

### 5.3 Authoring and AI
- Flow editor and debugger are L5 in the plan; the animation timeline (A2) is a new panel built on the same V2 canvas conventions.
- AI authoring (L7) gets tools generated from the registry: `describe_events`, `describe_actions`, `draft_rules`, `draft_clip`, `validate`,
  `simulate_playthrough`; the validator owns correctness. Add animation-specific refusals: unreachable clips, conflicting writers to one live parameter, rebuild-hot
  animation, unbounded loops.
- Add `snapshot` (several fixed views) and a `measure` tool and a short **play log** so the model can see what an animation did (positions at tick N).

### 5.4 Limits, performance and acceptance
- Live parameter updates for up to 10,000 instances must not run the kernel and must stay under a stated per-frame budget on desktop; measure on the
  headset before setting Quest caps (M9). Count animated entities, active tracks, particles and bones per tier.
- Acceptance per layer: deterministic replay (same document, seed and input log give the same state hash every tick, native and WASM); an instrumented test proves
  zero kernel calls during animation; Edit mode is unchanged after Play (document equality); the door, drawer, fan, lamp-flicker and patrol fixtures work by mouse
  and by emulated controller; a creature walk fixture if A5 is built.

## 6. The remaining plan items, in one place

- **G11 (optional):** toon material mode, outlines with a defined mesh policy, strokes and layers. Pick up only after G10 and the headset result, if a
  stylised look is wanted for the demo.
- **G12 (optional):** constrained 2D sketches (a solver chosen by experiment X5); the dimensioned bracket fixture.
- **V5 to V7 (optional):** polygon edit mode over G6, XR editing (grab and scale produce coalesced commands), editor polish (shortcuts, themes, onboarding, large
  graphs). V6 matters for the VR story: editing in headset.
- **L8, L9 (optional):** state machines, declarative interaction packs.
- **M10 (conditional):** multiview rendering, only if the headset run misses the frame budget. **M12:** runtime hardening.
- **G13:** release hardening and the compatibility report over everything shipped: versions, migrations, limits, sizes, timings, the visual rubric scores, the
  evaluation results, known gaps.

## 7. Cross-cutting rules for every track

- **Quest first.** No number measured under SwiftShader says anything about the Quest. The M9 run (owner plus headset) decides tier defaults for lights, texture
  memory, animated entity counts and bones. Until then mark Quest costs unmeasured, keep expensive features off the Low tier, and record the measurements
  you can (CPU time, memory, triangle and draw counts).
- **Determinism.** Anything computed in the evaluator or the play loop uses fixed sequences, integer hashing, sorted structures and our own trigonometry; native
  and WASI must match byte for byte on corpus lines. Never call libm in an evaluator.
- **Procedural only.** No imported textures, models, animation data or external generators.
- **Budgets and typed refusals.** Every new cost has a cap, a preflight, a stable diagnostic code with a fix suggestion, and a test that the cap holds.
- **Evidence, not claims.** Reports state what a test proves; a refusal test is not an implementation; screenshots come with the rubric. Keep SwiftShader-only
  and no-headset limits stated.
- **Gate discipline.** `cargo xtask quick` while working; no agent strict gate (quick, rules, parity/cpp and focused browser scripts instead; the owner runs strict at milestones); Rust files under 400 lines, functions under 80, no
  `#[allow]`; C++ through clang-format, clang-tidy, ASan and UBSan.
- **Process.** Another agent may be editing the tree; re-run before assuming a failure is yours. No commits, no attribution lines, edit tools only (formatters and
  `bun run schema:generate` are fine). Never reuse a provider key pasted in chat without the owner asking; rotate pasted keys.

## 8. Decisions for the owner (recommendation in brackets)

1. Add G10b (modeling batch) and the animation layers A1 to A4 and A8 to the core release. [Yes; A5 skeletons and A6 particles after the headset result; A7 physics
   as an experiment only.]
2. Texture delivery. [Both: shader-side recipes first for desktop, baked sets for Quest, one recipe language.]
3. Glass and water. [Scope in G10 as a renderer item, simple alpha glass first, water second, tier-gated.]
4. Parameter expressions. [Add a bounded pure expression type if it does not already exist; no user code.]
5. Timeline panel (V8). [Build after L3; keep clips text-editable first.]
6. Calendar. [Start G10 textures, L0 to L3, and the G10b placement batch this week in parallel; reserve the last 7 days for G13 and the integrated demo.]

## 9. First packages to hand out now

- Logic agent: `handoff-l0-l1-logic.md`, then L2 and L3 with A1 tweens and A3 oscillate/spin/noise as the first animation deliverable (a spinning fan, a door).
- Modeling-kit agent: placement batch (3.2 first bullet), `profile.fillet`, LOD (`solid.decimate`), then hull and slicing wrappers, then the first four kits.
- Render agent: continue `handoff-g9-fidelity-and-ai.md` B2 AO bake and B5 materials, designed as in section 4 so G10 reuses the recipe language.
- AI agent: `snapshot`, `measure`, `get_operator_help`, fix suggestions on diagnostics and the DeepSeek runs; the mock corpus for G9c.
- Owner: schedule the M9 headset run (this unblocks tier defaults for everything above); decide the items in section 8.
