// Flowing water (ADR 0059): colour by depth, a surface pattern that streams along the current, foam at the banks and in fast water, a soft
// edge where the sheet meets the bank. The mesh carries what the shader needs in its vertex colour (flow x and z remapped to 0..1, depth in
// units of 6 metres, foam), so the shader never reads the scene depth or a screen copy: it works the same on WebGL2 and in VR. The
// surface pattern is procedural gradient-style noise, scrolled in two phases half a cycle apart and cross-faded (the technique Valve used
// for Portal 2), so the restart of each layer is hidden and the pattern does not visibly repeat or swim.

import bevy_pbr::render::{
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    pbr_fragment::pbr_input_from_standard_material,
    forward_io::{Vertex, VertexOutput, FragmentOutput, decompress_vertex},
    mesh_functions,
    view_transformations::position_world_to_clip,
    mesh_view_bindings as view_bindings,
};
@if(VISIBILITY_RANGE_DITHER)
import bevy_pbr::render::pbr_functions::visibility_range_dither;

struct WaterParams {
    // rgb shallow colour (linear), w clarity in metres.
    shallow: vec4<f32>,
    // rgb deep colour (linear), w opacity of the deepest water.
    deep: vec4<f32>,
    // x flow speed multiplier, y ripple strength, z foam amount.
    a: vec4<f32>,
    sim_bounds: vec4<f32>,
    spatial: array<vec4<f32>, 12>,
    temporal: array<vec4<f32>, 12>,
};
@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> water: WaterParams;
@group(constants::MATERIAL_BIND_GROUP) @binding(101) var water_sim: texture_2d<f32>;

fn sim_height(p: vec2<f32>) -> f32 {
    if water.sim_bounds.z <= 0.0 { return 0.0; }
    let uv = (p - water.sim_bounds.xy) / water.sim_bounds.zw;
    if any(uv < vec2<f32>(0.0)) || any(uv >= vec2<f32>(1.0)) { return 0.0; }
    let dims = vec2<i32>(textureDimensions(water_sim));
    let c = clamp(uv * vec2<f32>(dims) - vec2<f32>(0.5), vec2<f32>(0.0), vec2<f32>(dims - vec2<i32>(1)));
    let a = vec2<i32>(c);
    let b = min(a + vec2<i32>(1), dims - vec2<i32>(1));
    let t = fract(c);
    return mix(mix(textureLoad(water_sim, a, 0).x, textureLoad(water_sim, vec2<i32>(b.x, a.y), 0).x, t.x),
               mix(textureLoad(water_sim, vec2<i32>(a.x, b.y), 0).x, textureLoad(water_sim, b, 0).x, t.x), t.y);
}

fn surface_height(p: vec2<f32>) -> f32 {
    var h = sim_height(p);
    for (var i = 0u; i < 12u; i += 1u) {
        let wave = water.spatial[i];
        h += wave.z * sin(dot(wave.xy, p) - water.temporal[i].x * water.a.w + wave.w);
    }
    return h;
}

fn surface_gradient(p: vec2<f32>) -> vec2<f32> {
    var gradient = vec2<f32>(0.0);
    if water.sim_bounds.z > 0.0 {
        let cell = water.sim_bounds.zw / vec2<f32>(textureDimensions(water_sim));
        gradient = vec2<f32>(sim_height(p + vec2<f32>(cell.x, 0.0)) - sim_height(p - vec2<f32>(cell.x, 0.0)),
                            sim_height(p + vec2<f32>(0.0, cell.y)) - sim_height(p - vec2<f32>(0.0, cell.y))) / (2.0 * cell);
    }
    for (var i = 0u; i < 12u; i += 1u) {
        let wave = water.spatial[i];
        gradient += wave.xy * wave.z * cos(dot(wave.xy, p) - water.temporal[i].x * water.a.w + wave.w);
    }
    return gradient;
}

@vertex
fn vertex(input: Vertex) -> VertexOutput {
    let v = decompress_vertex(input, input.instance_index);
    let world_from_local = mesh_functions::get_world_from_local(input.instance_index);
    var out: VertexOutput;
    out.world_position = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(v.position, 1.0));
    out.world_position.y += surface_height(out.world_position.xz);
    out.position = position_world_to_clip(out.world_position.xyz);
@if(VERTEX_NORMALS)
    out.world_normal = mesh_functions::mesh_normal_local_to_world(v.normal, input.instance_index);
@if(VERTEX_UVS_A)
    out.uv = v.uv;
@if(VERTEX_UVS_B)
    out.uv_b = v.uv_b;
@if(VERTEX_TANGENTS)
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(world_from_local, v.tangent, input.instance_index);
@if(VERTEX_COLORS)
    out.color = v.color;
@if(VERTEX_OUTPUT_INSTANCE_INDEX)
    out.instance_index = input.instance_index;
@if(VISIBILITY_RANGE_DITHER)
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(input.instance_index, world_from_local[3]);
    return out;
}

fn hash2(p: vec2<f32>) -> f32 {
    var q = fract(p * vec2<f32>(0.1031, 0.1030));
    q += dot(q, q.yx + 33.33);
    return fract((q.x + q.y) * q.x);
}

fn noise2(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash2(i), hash2(i + vec2<f32>(1.0, 0.0)), u.x), mix(hash2(i + vec2<f32>(0.0, 1.0)), hash2(i + vec2<f32>(1.0, 1.0)), u.x), u.y);
}

fn fbm2(p: vec2<f32>) -> f32 {
    return noise2(p) * 0.55 + noise2(p * 2.07 + vec2<f32>(5.2, 1.3)) * 0.3 + noise2(p * 4.3 + vec2<f32>(9.1, 7.7)) * 0.15;
}

// The slope of the ripple height at a point (x and z), by central differences.
fn ripple_slope(p: vec2<f32>) -> vec2<f32> {
    let e = 0.06;
    return vec2<f32>(
        fbm2(p + vec2<f32>(e, 0.0)) - fbm2(p - vec2<f32>(e, 0.0)),
        fbm2(p + vec2<f32>(0.0, e)) - fbm2(p - vec2<f32>(0.0, e))
    ) / (2.0 * e);
}

@fragment
fn fragment(vertex_output: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var in = vertex_output;
    @if(VISIBILITY_RANGE_DITHER)
    visibility_range_dither(in.position, in.visibility_range_dither);
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    var data = vec4<f32>(0.5, 0.5, 0.4, 0.0);
@if(VERTEX_COLORS) {
    data = in.color;
}
    let flow = vec2<f32>(data.x, data.y) * 2.0 - vec2<f32>(1.0);
    let speed = clamp(length(flow), 0.0, 1.0);
    let heading = flow / max(speed, 1.0e-3);
    let depth01 = data.z;
    let wp = in.world_position.xyz;

    // Two phases of the surface pattern, half a cycle apart, each dragged along the current and cross-faded.
    let t = water.a.w * water.a.x;
    let cycle = 1.6;
    let phase_a = fract(t / cycle);
    let phase_b = fract(t / cycle + 0.5);
    let blend = abs(1.0 - 2.0 * phase_a);
    let travel = 1.6 * (0.35 + speed);
    let drift = select(vec2<f32>(0.4, 0.2), heading, speed > 0.05);
    let uv_a = wp.xz * 0.8 - drift * phase_a * travel;
    let uv_b = wp.xz * 0.8 - drift * phase_b * travel + vec2<f32>(3.7, 8.1);
    let slope = mix(ripple_slope(uv_b), ripple_slope(uv_a), blend) + 0.5 * ripple_slope(wp.xz * 2.6 - drift * t * 0.9);

    // Colour by depth, from clear shallows to deep water.
    let depth_m = depth01 * 6.0;
    let depth_mix = smoothstep(0.0, 1.0, clamp(depth_m / max(water.shallow.w, 0.05), 0.0, 1.0));
    var colour = mix(water.shallow.rgb, water.deep.rgb, depth_mix);

    // Foam from the bank factor, the speed and the surface noise, kept to the shallow parts.
    let foam_noise = fbm2(wp.xz * 1.7 - drift * t * 0.5);
    let foam_field = clamp(data.w + (foam_noise - 0.5) * 0.7, 0.0, 1.0);
    let foam = smoothstep(0.55, 0.85, foam_field) * water.a.z * (1.0 - 0.5 * depth_mix);
    colour = mix(colour, vec3<f32>(0.9, 0.93, 0.95), foam * 0.9);

    // A soft shore: the sheet fades out over the first 0.7 metres of depth, foam fades in as it does.
    let edge = smoothstep(0.0, 0.12, depth01);
    let alpha = clamp(mix(0.35, water.deep.w, depth_mix) * edge + foam * 0.7 * edge, 0.0, 1.0);

    pbr_input.material.base_color = vec4<f32>(colour, alpha);
    pbr_input.material.metallic = 0.0;
    pbr_input.material.perceptual_roughness = clamp(0.05 + foam * 0.55, 0.04, 1.0);
    let n = pbr_input.N;
    let gradient = surface_gradient(wp.xz);
    pbr_input.N = normalize(n + vec3<f32>(-gradient.x, 0.0, -gradient.y) + vec3<f32>(slope.x, 0.0, slope.y) * water.a.y * 0.28 * (1.0 - foam * 0.6));

    var out: FragmentOutput;
    let lit = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, lit);
    return out;
}
