# ADR 0060: Fields, attributes, filters and recipes (the language core for worlds)

- Status: accepted (2026-10-22, owner accepted the three proposals in "Open decisions" below)
- Date: 2026-10-21
- Amends: ADR 0047 (number values and sockets; it left "Blender-style fields and attributes" open), ADR 0013 (groups), ADR 0056 (channels)
- Part of: `plans/game-world-system.md` workstream A

## Context

An audit on 2026-10-21 built five recipes (an oak from field unions, a pine from cones, a meadow, a cliff with a cave, a river with a tributary) with the nodes that exist. Shapes came out well (the language is stronger than expected), but every recipe hit the same walls:

1. **Parameters are literals.** `count=2500`, `density=1`, `size=0.7` are numbers. A model cannot say "fewer leaves toward the crown edge", "grass thins on rock", "tint by height". ADR 0047 sockets take one number from a `value.*` node; nothing takes a *function of position*.
2. **Three expression dialects.** The text language folds `{...}` arithmetic at parse time (`ve-graph/src/text/expr.rs`); texture programs have their own per-pixel `expr` (compiled in the browser, ADR 0025); `Field` is signed-distance only, evaluated by a C++ tape (`ve-geometry/src/field`); and `ve-world` has a separate 2D `Field2` for scatter. None can reference another.
3. **No attributes.** Per-copy colour, scale jitter, wind phase, age and masks have no home. Today a `Value` carries ad hoc `colors`, `normals` and `uvs` vectors, and instances carry transforms only.
4. **One output per operator** (`OperatorSpec::output`), so "keep these, drop those" cannot be expressed; there is no filter, no branch, no join of sets.
5. **Recipes cannot be called with arguments by element.** `group.use` instantiates a subgraph with overrides, but nothing applies a recipe to every point of a set.
6. **Geometry types do not bridge.** `xform.move` demands a closed manifold although moving is rigid; trees, terrain and open polygon meshes need the same transforms.

## Decision

Everything below is bounded, deterministic and typed; no scripting language (providers return typed commands, never code).

### 1. One expression language, the *field expression*
A field is a pure function from a sample (position, and optionally normal, uv and attributes of the element being sampled) to a typed value: `float`, `vec2`, `vec3`, `colour` (`vec4`) or `bool`. The expression syntax is the one texture programs already use (`q`, `p`, `fbm`, `smoothstep`, `mix`, `step`, `length`, vector constructors) extended with:
- inputs: `p` (world position), `n` (normal), `uv`, `attr("name")` (an attribute of the element), `height`, `slope`, and the values of other fields bound through named `field` input ports of `field.expr` (a distance to a curve or solid is a node that produces a field, not a function that looks nodes up by name; see the open decisions);
- noise: `fbm`, `ridged`, `voronoi`, `perlin`, each with seed and octave arguments;
- `ramp(x, [stops])` (piecewise colour or number curves) and `curve(x, [points])`;
- no loops, no recursion, no user functions; size and node counts are bounded (the 8,192-byte string bound of ADR 0025 stays, plus a 256-operation cap at evaluation).
An SDF is the special case of a scalar field with a distance guarantee; `field.to_solid` meshes any field whose guarantee says it is one, and refuses others with a `suggestion`. The existing SDF nodes (`field.ellipsoid`, `field.noise`, ...) keep their names and become constructors of the same `Field` type. A new node `field.expr(code=...)` builds a field from an expression, and `field.mask_from(tile, layer)` exposes a terrain channel as a field.
**Two implementations, one corpus.** The CPU side (Rust, tape-compiled like the SDF tape, bit-reproducible, used for scatter, filters, attributes, masks and meshing) and the GPU side (WGSL generated from the same expression for material layers and the texture compiler in the browser) must agree. A shared corpus of programs and expected samples (`tests/fixtures/field-expr/`) is run by both; the existing evaluator parity runner is the model. Where the browser texture compiler cannot yet express a function, the node is refused for GPU use with a diagnostic, never approximated silently.

### 2. Field-valued parameters ("fieldable")
`ParamSpec` gains a `fieldable: Option<FieldType>` flag. A link of type `Field` into a fieldable parameter (the same socket mechanism as ADR 0047, which keeps the text and printer forms) makes the node sample the field where it needs the value: at each point for `scatter` density and size, at each instance for scale and colour, at each vertex for displacement, at each pixel for a material layer's weight. Non-fieldable parameters refuse a field link with `param.not_fieldable` and name the nearest fieldable one. A literal stays a literal; a number or `value.*` link stays a constant (a constant is a trivially constant field).
First fieldable parameters (each with tests): `scatter.place` and `inst.scatter_on` (`density`, `scale`, `tilt`, `colour`), `terrain.material` layers (weight and colour), `terrain.mask` (the mask value), `solid.material` (colour, roughness, opacity), `poly.displace` (new), `river.water` (flow, depth).

### 3. Attributes
An **attribute** is a named column attached to one *domain* of a value: `point`, `vertex`, `face`, `curve` (per curve), `curve_point`, `instance`. Types: `float`, `vec3`, `colour`, `int`, `bool`. A `Value` holds `attributes: BTreeMap<Name, Column>` per domain (ordered for determinism); `colors`, `normals`, `uvs` become the reserved attributes `colour`, `normal`, `uv` on their domains. Reserved names with engine meaning, carried to the shader where the renderer has a channel for them: `colour`, `scale`, `rotation`, `radius`, `phase` (wind), `age`, `depth`, `mask`. Other names stay CPU-side and can drive other nodes.
Nodes (all take the value and return it with the attribute added or changed): `attr.set(name, domain, expr)` computes a column from a field expression sampled per element; `attr.noise`/`attr.random(seed)` convenience forms; `attr.transfer(from, to, name)` copies a column between values by nearest element (a tile's wetness onto the points scattered on it); `attr.stat(name, kind)` reduces a column to a `Number` (for sockets and checks). Evaluators that create elements (scatter, growth, instancing, subdivision) must propagate attributes by documented rules (copy for instancing, interpolate for subdivision, inherit from the parent node for growth).
Delivery to the renderer, bounded by what Bevy/WebGL2 offers: reserved colour and wind attributes ride vertex colours of merged chunks and `MeshTag` of near entities (ADR 0058); everything else is not sent.

### 4. Filter, split, join
`set.keep(expr)` keeps the elements for which a `bool` field expression holds; `set.drop(expr)` is its complement (two nodes because an operator has one output); `set.join(a, b, ...)` merges values of the same kind and domain, attribute columns taking defaults where one side lacks them; `set.take(n, seed)` and `set.sort(expr)` bound and order sets. All preserve determinism by element order. Estimates count elements before evaluation (a filter cannot grow a set).

### 5. Recipes and repetition
A **recipe** is a group with typed parameters and defaults (ADR 0013 groups already expose parameters; this ADR adds *typed signatures* with ranges so the guide and tool schema list them, and `recipe` is a flag on the group so discovery can list recipes separately). A new node `map.each(points, recipe)` instantiates a recipe for every element of a point or instance set: the recipe's exposed parameters are filled from the element's attributes by name (or from constants), its output placed at the element's transform, results joined into one instance set. Repetition is bounded by the per-set budget of ADR 0058 and by `MAX_GROUP_DEPTH`; recursion stays refused. This is how species, buildings and biomes are written: `oak(height=…)` is a recipe the model (or the guide) defines, `map.each` plants it.

### 6. Type bridges, defined once
A table in the language reference gives every legal conversion with its node and cost: Poly to Solid (`poly.to_solid`, requires manifold), Solid to Poly (`poly.from_solid`), Curve to Poly (`curve.tube`, `solid.sweep`), Inst to Poly (`inst.realize`), Tile to Poly and Field (`terrain.to_poly`, `field.mask_from`), Field to Solid (`field.to_solid`), Sdf to Field (identity). **Rigid transforms (`xform.move`, `rotate`, `scale`, `mirror`) and `xform.array` accept any mesh-like value (Poly, open meshes, tiles' parts, growth output)**; only operations that need a closed volume (booleans, `solid.*` finishing, volume measures) require manifolds. An argument of the wrong type is refused with `type.bridge` naming the bridge node, never converted implicitly.

### 7. Evaluation rules
Sampling is lazy and per element, in element order; a field expression compiles once per node (cache key includes the expression text and bound attribute names). Per-evaluation limits: elements (the budget estimate), expression operations per sample, and total samples; exceeding any is `budget.*` with a suggestion. All noise uses the platform-independent helpers (no libm trigonometry), so native and WASM agree to the bit; the parity corpus enforces it. WASM speed is measured, not assumed: any node that exceeds an interactive budget on the WASM evaluator reports progress and is cancellable (the colonizer lesson).

## Consequences

- A model gets, with one idea ("a parameter may be a function of where"), variation, density control, masks driving materials and scatter, and recipes applied per element, without any named species or biome in Rust.
- The existing SDF field nodes, texture programs and `ve-world` `Field2` migrate onto the single `Field` type in steps; until a step lands, the old forms keep working (nothing in a saved document breaks: new parameters are optional links).
- Text form: `density=@wet_edge` style links are not added; a field is a node (`f = field.expr(code="...")`) linked by port name like a socket, so the printer, diff and undo are unchanged. Attributes are named in strings.
- Cost: a new `Field` evaluator (tape extension), attribute plumbing through `Value` and every element-creating evaluator, a WGSL code generator that must match the CPU, a larger parity corpus. Each phase below ships with tests first.
- Risk: expression sprawl. Mitigations: fixed grammar, operation caps, no user functions, one corpus. Risk: GPU/CPU divergence; mitigated by refusing GPU use of anything the corpus does not cover.

## Phases (each a task list in the game-world guide, tests first)
1. **Field expression core** (Rust tape: types, grammar, noise, `ramp`/`curve`, bit-reproducibility, corpus) and `field.expr`, `field.mask_from`; SDF fields become `Field` constructors.
2. **Fieldable parameters** on `scatter.place`, `inst.scatter_on`, `terrain.mask`, `terrain.material`, `solid.material`, with the socket mechanism extended to `Field` links.
3. **Attributes** (`Value` columns, reserved names, `attr.set/transfer/stat`, propagation rules), then renderer delivery for `colour`, `scale`, `phase`.
4. **Filter/split/join** (`set.*`).
5. **Recipes and `map.each`**, typed signatures, discovery.
6. **Type bridges:** rigid transforms accept any mesh-like value; `type.bridge` diagnostics; the conversion table in the guide.
7. **GPU generation** of material-layer and texture fields from the same expressions (parity corpus on both).

## Alternatives considered
- **Blender-style lazy fields over every domain with implicit capture.** The model-facing surface is large and implicit; ours is explicit nodes and one sample type with bounded cost. Still compatible later.
- **A scripting language for placement and variation.** Refused by the providers-return-typed-commands rule; also unsafe to bound in WASM.
- **Per-feature variation parameters (`scale_jitter`, `colour_jitter`, `density_noise` ...).** The current shape; each new need adds parameters to every node. Fields remove the growth.
- **Keeping the texture `expr` dialect separate.** Leaves the audit's second wall: a material layer cannot use the same mask as the scatter that decorates it.

## Decisions taken (owner, 2026-10-22: "yeah" to each proposal)
- Whether the field expression grammar is allowed to read `dist(name)` of document nodes (powerful, adds dependencies between nodes) or only values passed as bound inputs (simpler). Proposed: bound inputs only, through named `field` ports; `dist` is then a node, not a function.
- Where the shared corpus lives (proposed `engine/tests/field-expr/`).
- Whether `map.each` may nest (proposed: depth 2).
