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
) -> bool {
    projection::instances(token, group, node, matrices, indices)
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
/// material region. `textured` vertices are nine floats (position, normal, occlusion, texture
/// coordinates), otherwise seven.
#[wasm_bindgen]
#[must_use]
pub fn eval_projection_chunk_part(
    token: &str,
    group: u32,
    region: u32,
    node: &str,
    name: &str,
    vertices: &[f32],
    textured: bool,
) -> bool {
    projection::chunk_part(token, (group, region), node, name, vertices, textured)
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
