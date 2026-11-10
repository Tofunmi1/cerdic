// Adapted from RedGPU landscape grass shaders (MIT, Copyright (c) 2019~ Redcamel).
// See ADR 0070. CPU mirrors: grass.rs.
struct GrassParams {
    // rgb ground colour (linear), w blending strength.
    ground: vec4<f32>,
    // x enabled, y translucency, z exposure boost, w reserved.
    look: vec4<f32>,
    // x near, y far, z mesh minimum y, w inverse mesh height.
    shape: vec4<f32>,
};
fn grass_height_ratio(y: f32, min_y: f32, inv_mesh_height: f32) -> f32 {
    return clamp((y - min_y) * inv_mesh_height, 0.0, 1.0);
}
fn grass_ground_blend(base: vec3<f32>, ground: vec3<f32>, ratio: f32, strength: f32, exposure_boost: f32) -> vec3<f32> {
    var blend = 0.0;
    if (strength > 0.01) { blend = clamp((0.4 - ratio) * 2.5, 0.0, 1.0) * strength; }
    return mix(base * max(0.1, exposure_boost), ground, blend);
}
fn grass_sky_occlusion(ratio: f32) -> f32 {
    return 0.65 + 0.35 * clamp(ratio * 1.43, 0.0, 1.0);
}
fn grass_contact_ao(ratio: f32) -> f32 {
    return 0.4 + 0.6 * clamp(ratio * 4.0, 0.0, 1.0);
}
fn grass_upward_normal(normal: vec3<f32>, ratio: f32) -> vec3<f32> {
    let n = mix(normal, vec3<f32>(0.0, 1.0, 0.0), 0.55 + 0.3 * ratio);
    if (dot(n, n) > 1e-12) { return normalize(n); }
    return vec3<f32>(0.0, 1.0, 0.0);
}
fn grass_sink_correction(local_y: f32, ratio: f32) -> f32 {
    return max(0.0, -local_y) * (1.0 - ratio * 0.7);
}
fn grass_fade_scale(distance: f32, near: f32, far: f32) -> f32 {
    // Comparisons also reject NaN and infinity; normalize endpoints before subtracting to avoid overflow.
    let finite_limit = 3.4028234663852886e38;
    if (!(abs(distance) <= finite_limit && abs(near) <= finite_limit && abs(far) <= finite_limit) || far <= near) { return 1.0; }
    let scale = max(1.0, max(abs(distance), max(abs(near), abs(far))));
    let width = far / scale - near / scale;
    if (width <= 0.0) { return select(0.0, 1.0, distance <= near); }
    return clamp(1.0 - (distance / scale - near / scale) / width, 0.0, 1.0);
}

// The base stays fixed under non-uniform scale and rotation. Wind is shrunk along with the blade.
fn grass_world_position(world: vec3<f32>, model: mat4x4<f32>, local: vec3<f32>, camera: vec3<f32>, grass: GrassParams) -> vec3<f32> {
    if (grass.look.x <= 0.5) { return world; }
    let ratio = grass_height_ratio(local.y, grass.shape.z, grass.shape.w);
    let base = (model * vec4<f32>(0.0, grass.shape.z, 0.0, 1.0)).xyz;
    var offset = world - base;
    offset.y += grass_sink_correction(offset.y, ratio);
    let fade = grass_fade_scale(distance(base, camera), grass.shape.x, grass.shape.y);
    return base + offset * fade;
}
