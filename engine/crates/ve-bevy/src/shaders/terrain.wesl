// Terrain layers use the same grid, hash and shading maths as the forward renderer.
// stochastic.wgsl and forward_terrain.wgsl are appended when this asset is embedded.
import bevy_pbr::render::{
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    pbr_fragment::pbr_input_from_standard_material,
    forward_io::{VertexOutput, FragmentOutput},
    mesh_view_bindings as view_bindings,
};
@if(VISIBILITY_RANGE_DITHER)
import bevy_pbr::render::pbr_functions::visibility_range_dither;

@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> terrain: TerrainParams;

@fragment
fn fragment(vertex_output: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var in = vertex_output;
    @if(VISIBILITY_RANGE_DITHER)
    visibility_range_dither(in.position, in.visibility_range_dither);
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    let wp = in.world_position.xyz;
    let n = pbr_input.N;
    let scale = terrain.c.z * terrain.c.w;
    let layer = terrain_surface(wp, n, scale, terrain);
    pbr_input.material.base_color = vec4<f32>(layer.rgb, 1.0);
    pbr_input.material.perceptual_roughness = clamp(layer.w, 0.05, 1.0);

    let slope = acos(clamp(n.y, 0.0, 1.0)) / 1.5707963;
    let broad = terrain_fbm(vec3<f32>(wp.x, 0.0, wp.z) * 0.012);
    let drift = (broad - 0.5) * terrain.b.z;
    let rock = smoothstep(terrain.a.x - terrain.a.y, terrain.a.x + terrain.a.y, slope + drift * 0.3);
    let snow = smoothstep(terrain.a.z - terrain.a.w, terrain.a.z + terrain.a.w, wp.y + drift * 40.0) * (1.0 - smoothstep(terrain.a.x * 0.6, terrain.a.x * 1.1, slope));
    // Fine relief fades out with distance (it only shimmers there) and is not computed at all beyond 200 m, which also saves its six extra noise lookups.
    let distance_to_eye = length(view_bindings::view.world_position - wp);
    let relief_fade = 1.0 - smoothstep(60.0, 200.0, distance_to_eye);
    let strength = terrain.b.w * (0.25 + 0.75 * rock) * (1.0 - snow * 0.7) * relief_fade;
    if (strength > 0.0) {
        let frequency = max(terrain.c.x, 1.0e-3);
        let q = wp * frequency;
        let e = 0.06;
        let gx = terrain_detail(q + vec3<f32>(e, 0.0, 0.0), scale).z - terrain_detail(q - vec3<f32>(e, 0.0, 0.0), scale).z;
        let gy = terrain_detail(q + vec3<f32>(0.0, e, 0.0), scale).z - terrain_detail(q - vec3<f32>(0.0, e, 0.0), scale).z;
        let gz = terrain_detail(q + vec3<f32>(0.0, 0.0, e), scale).z - terrain_detail(q - vec3<f32>(0.0, 0.0, e), scale).z;
        let gradient = vec3<f32>(gx, gy, gz) / (2.0 * e);
        pbr_input.N = normalize(n - (gradient - n * dot(gradient, n)) * strength / frequency * 0.25);
    }
    var out: FragmentOutput;
    out.color = main_pass_post_lighting_processing(pbr_input, apply_pbr_lighting(pbr_input));
    return out;
}
