//! Instance projection exports, keeping the root entry point small.
use crate::projection;
use wasm_bindgen::prelude::*;

/// Appends at most 512 checked instance placements to one prototype batch.
#[wasm_bindgen]
#[must_use]
pub fn eval_projection_instances(
    token: &str,
    group: u32,
    node: &str,
    matrices: &[f32],
    indices: &[u32],
    has_range: bool,
    show_from: f32,
    show_to: f32,
    show_fade: f32,
) -> bool {
    projection::instances(
        token,
        group,
        node,
        matrices,
        indices,
        has_range.then_some((show_from, show_to)),
        show_fade,
    )
}

/// Sets the surface of one material region of the part made by `node`; a fused solid of several
/// colours has one region per colour (region 0 is what `eval_projection_material` sets).
#[wasm_bindgen]
#[must_use]
pub fn eval_projection_material_region(
    token: &str,
    node: &str,
    region: u32,
    material: &str,
) -> bool {
    projection::material_region(token, node, region, material)
}

/// Stages the baked texture maps of one material region: albedo, tangent-space normal, packed
/// metallic-roughness and emissive, each square RGBA8 of 1024, 512 or 256 texels a side (an empty array means that map is absent).
#[wasm_bindgen]
#[must_use]
pub fn eval_projection_maps(
    token: &str,
    node: &str,
    region: u32,
    albedo: &[u8],
    normal: &[u8],
    metallic_roughness: &[u8],
    emissive: &[u8],
) -> bool {
    projection::maps(
        token,
        node,
        region,
        [albedo, normal, metallic_roughness, emissive],
    )
}

/// Stages the keyframed tracks (a JSON array) of the part made by `node`; the viewer plays them.
#[wasm_bindgen]
#[must_use]
pub fn eval_projection_animation(token: &str, node: &str, tracks: &str) -> bool {
    projection::animation(token, node, tracks)
}

/// Stages the animation state machine (JSON) of the part made by `node` (`anim.machine`); the viewer drives it each frame.
#[wasm_bindgen]
#[must_use]
pub fn eval_projection_machine(token: &str, node: &str, machine: &str) -> bool {
    projection::machine(token, node, machine)
}

/// Stages a skinned part's bones (a JSON array of names), the vertex of each triangle corner, and four joint
/// indices and four weights per vertex (ADR 0030). The viewer deforms the mesh from the bones' poses each frame.
#[wasm_bindgen]
#[must_use]
pub fn eval_projection_skin(
    token: &str,
    node: &str,
    bones: &str,
    indices: &[u32],
    joints: &[u32],
    weights: &[f32],
    dual: bool,
) -> bool {
    projection::skin(token, node, bones, indices, joints, weights, dual)
}

/// Stages the physics body (a JSON `PhysBody`) of the part made by `node`, with its vertices and triangles for the
/// shapes built from them (`phys.body`).
#[wasm_bindgen]
#[must_use]
pub fn eval_projection_body(
    token: &str,
    node: &str,
    body: &str,
    points: &[f32],
    triangles: &[u32],
) -> bool {
    projection::body(token, node, body, points, triangles)
}

/// `{ active, steps, bodies: [{ node, kind, held, position, rotation, speed }] }`: the running physics world, as of the
/// last frame.
#[wasm_bindgen]
#[must_use]
pub fn viewer_physics() -> String {
    crate::physics::published()
}

/// Stages the chain of bones (a JSON `IkChain` with segments) that the part made by `node` is the target of (`rig.ik`).
#[wasm_bindgen]
#[must_use]
pub fn eval_projection_ik(token: &str, node: &str, chain: &str) -> bool {
    projection::ik(token, node, chain)
}

/// Stages the particle effect (a JSON `PartFx`) that the part made by `node` emits (`fx.emitter`).
#[wasm_bindgen]
#[must_use]
pub fn eval_projection_fx(token: &str, node: &str, effect: &str) -> bool {
    projection::fx(token, node, effect)
}

/// Stages the ragdoll definition (a JSON `RagdollSpec`) carried by the part made by `node` (`phys.ragdoll`).
#[wasm_bindgen]
#[must_use]
pub fn eval_projection_ragdoll(token: &str, node: &str, ragdoll: &str) -> bool {
    projection::ragdoll(token, node, ragdoll)
}

/// `{ cloths: [{ points, steps, min, max, pin_matrix }] }`: the live cloths (`cloth.sim`) as of the last frame.
#[wasm_bindgen]
#[must_use]
pub fn viewer_cloth() -> String {
    crate::cloth_live::published()
}

/// Gives map `slot` (0 albedo, 1 normal, 2 metallic-roughness, 3 emissive) of the part made by `node`, in `region`, a new picture: square RGBA8 of the size the map already
/// has. This is how a texture that moves (a shader drawn every frame by the page) reaches the renderer. `false` when the scene has no such map or the size is wrong.
#[wasm_bindgen]
#[must_use]
pub fn viewer_texture_update(node: &str, region: u32, slot: u32, pixels: &[u8]) -> bool {
    projection::live_update(node, region, slot, pixels)
}

/// `{ fluids: [{ particles, steps, min, max, triangles, fastest }] }`: the live liquids (`fluid.sim`) as of the last frame.
#[wasm_bindgen]
#[must_use]
pub fn viewer_fluid() -> String {
    crate::fluid_live::published()
}

/// Stages the live liquid (a JSON `FluidSpec`, collider segments filled in) of the part made by `node` (`fluid.sim`).
#[wasm_bindgen]
#[must_use]
pub fn eval_projection_fluid(token: &str, node: &str, spec: &str) -> bool {
    projection::fluid(token, node, spec)
}

/// `{ sims: [{ particles, steps, min, max, error }] }`: the live simulation programs (`sim.program`) as of the last frame.
#[wasm_bindgen]
#[must_use]
pub fn viewer_sim() -> String {
    crate::sim_live::published()
}

/// Stages the simulation program (a JSON `SimSpec`) of the part made by `node` (`sim.program`), with the part's vertices (flat xyz) and the
/// vertex each triangle corner comes from (used by a program that drives the mesh).
#[wasm_bindgen]
#[must_use]
pub fn eval_projection_sim(token: &str, node: &str, spec: &str, points: &[f32], corners: &[u32]) -> bool {
    projection::sim(token, node, spec, points, corners)
}

/// Stages the live cloth (a JSON `ClothSpec`) of the part made by `node` (`cloth.sim`), with the part's vertices (flat xyz) and its triangle
/// corners (flat vertex indices).
#[wasm_bindgen]
#[must_use]
pub fn eval_projection_cloth(
    token: &str,
    node: &str,
    spec: &str,
    points: &[f32],
    corners: &[u32],
) -> bool {
    projection::cloth(token, node, spec, points, corners)
}

/// Stages the output name of the part that the part made by `node` follows (`anim.parent`).
#[wasm_bindgen]
#[must_use]
pub fn eval_projection_parent(token: &str, node: &str, parent: &str) -> bool {
    projection::parent(token, node, parent)
}

/// Stages the `behavior.on` response (a JSON object) of the part made by `node`.
#[wasm_bindgen]
#[must_use]
pub fn eval_projection_behavior(token: &str, node: &str, behavior: &str) -> bool {
    projection::behavior(token, node, behavior)
}

/// Turns play mode on or off; behaviours only react in play mode, and leaving it resets them.
#[wasm_bindgen]
pub fn viewer_play_mode(on: bool) {
    crate::behavior::set_play(on);
}

/// Triggers the behaviour of the part built by `node`, as if clicked (play mode only).
#[wasm_bindgen]
pub fn viewer_press(node: &str) {
    crate::behavior::press(node);
}

/// Sets a scene flag (play mode only), as if a behaviour had; `flag` triggers watching it react in the next step.
#[wasm_bindgen]
pub fn viewer_flag(name: &str, on: bool) {
    crate::behavior::set_flag(name, on);
}

/// `{ play, steps, clicks, flags, parts: [{ node, time, forward, running }] }` for the behaviours as of the last frame.
#[wasm_bindgen]
#[must_use]
pub fn viewer_behaviors() -> String {
    crate::behavior::state()
}

/// Sets the shared animation clock's speed as a multiple of real time (0.05 to 8).
#[wasm_bindgen]
pub fn viewer_animation_speed(speed: f32) {
    crate::animation::set_speed(speed);
}

/// Plays or pauses the shared animation clock; a `seconds` of zero or more also moves it there.
#[wasm_bindgen]
pub fn viewer_animation(playing: bool, seconds: f32) {
    crate::animation::control(playing, seconds);
}

/// Sets a number parameter of the animation state machine on the part named `part` (empty: every machine), for example `speed` or
/// `vy`; a parameter set this way is no longer computed from the part's movement.
#[wasm_bindgen]
pub fn animation_set_float(part: &str, name: &str, value: f32) {
    crate::animation::request_float(part, name, value);
}

/// Sets a true or false parameter (for example `grounded`) of the animation state machine on the part named `part` (empty: every machine).
#[wasm_bindgen]
pub fn animation_set_bool(part: &str, name: &str, value: bool) {
    crate::animation::request_bool(part, name, value);
}

/// Raises a trigger (for example `jump`) on the animation state machine on the part named `part` (empty: every machine).
#[wasm_bindgen]
pub fn animation_trigger(part: &str, name: &str) {
    crate::animation::request_trigger(part, name);
}

/// Stages the scene's complete bounded light list alongside its mesh projection.
#[wasm_bindgen]
#[must_use]
pub fn eval_projection_lights(token: &str, lights: &str) -> bool {
    projection::lights::append(token, lights)
}

/// Stages the document's scene environment (at most one) alongside its mesh projection.
#[wasm_bindgen]
#[must_use]
pub fn eval_projection_environment(token: &str, environment: &str) -> bool {
    projection::environment::append(token, environment)
}

/// Adds one mesh chunk of a part: `group` names its placement set (0 for none) and `region` its
/// material region. A vertex is seven floats (position, normal, occlusion); `layout` bit 0 adds two
/// (texture coordinates) and bit 1 adds four (a vertex colour: water carries flow, depth and foam in it).
#[wasm_bindgen]
#[must_use]
pub fn eval_projection_chunk_part(
    token: &str,
    group: u32,
    region: u32,
    node: &str,
    name: &str,
    vertices: &[f32],
    layout: u32,
) -> bool {
    projection::chunk_part(token, (group, region), node, name, vertices, layout)
}

/// Adds one prototype mesh chunk belonging to an instance batch.
#[wasm_bindgen]
#[must_use]
pub fn eval_projection_chunk_group(
    token: &str,
    group: u32,
    node: &str,
    name: &str,
    vertices: &[f32],
) -> bool {
    projection::chunk_group(token, group, node, name, vertices)
}
