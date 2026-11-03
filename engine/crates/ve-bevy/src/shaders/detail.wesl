// Procedural surface relief: 3D noise perturbs the shading normal in the pixel shader, so a surface carries wrinkles, scales or pores finer than its
// mesh, with no texture memory and no resolution limit. The noise is read at the object-space position of the pixel (the world position taken back
// through the entity's transform), so it stays on a part that moves as a whole; a skinned part would need its rest position as an attribute (not yet).

import bevy_pbr::render::{
    pbr_types,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
    pbr_fragment::pbr_input_from_standard_material,
    forward_io::{VertexOutput, FragmentOutput},
    mesh_functions,
    mesh_view_bindings as view_bindings,
};
@if(VISIBILITY_RANGE_DITHER)
import bevy_pbr::render::pbr_functions::visibility_range_dither;

// x strength (how steep the relief is), y scale (features per metre), z style (0 smooth, 1 ridged, 2 scales), w octaves.
struct DetailParams {
    a: vec4<f32>,
    // x wetness, y iridescence, z sheen, w subsurface.
    b: vec4<f32>,
    // x thickness.
    c: vec4<f32>,
};
@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> detail: DetailParams;

fn hash3(p: vec3<f32>) -> f32 {
    var q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    q += dot(q, q.yzx + 33.33);
    return fract((q.x + q.y) * q.z);
}

fn value_noise(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = mix(mix(hash3(i), hash3(i + vec3<f32>(1.0, 0.0, 0.0)), u.x), mix(hash3(i + vec3<f32>(0.0, 1.0, 0.0)), hash3(i + vec3<f32>(1.0, 1.0, 0.0)), u.x), u.y);
    let b = mix(mix(hash3(i + vec3<f32>(0.0, 0.0, 1.0)), hash3(i + vec3<f32>(1.0, 0.0, 1.0)), u.x), mix(hash3(i + vec3<f32>(0.0, 1.0, 1.0)), hash3(i + vec3<f32>(1.0, 1.0, 1.0)), u.x), u.y);
    return mix(a, b, u.z);
}

// Distance to the nearest of a jittered lattice of points: round cells, which read as scales or pebbles.
fn cell_noise(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    var nearest = 8.0;
    for (var x = -1; x <= 1; x++) {
        for (var y = -1; y <= 1; y++) {
            for (var z = -1; z <= 1; z++) {
                let offset = vec3<f32>(f32(x), f32(y), f32(z));
                let jitter = vec3<f32>(hash3(i + offset), hash3(i + offset + 17.0), hash3(i + offset + 41.0));
                nearest = min(nearest, length(offset + jitter - f));
            }
        }
    }
    return nearest;
}

// The height of the relief at an object-space point, 0 to 1.
fn height(p: vec3<f32>) -> f32 {
    let style = detail.a.z;
    var total = 0.0;
    var weight = 0.5;
    var norm = 0.0;
    var q = p * detail.a.y;
    for (var o = 0; o < 4; o++) {
        if (f32(o) >= detail.a.w) { break; }
        var n = value_noise(q);
        if (style > 1.5) {
            n = 1.0 - clamp(cell_noise(q) * 1.1, 0.0, 1.0);
            n = smoothstep(0.0, 0.8, n);
        } else if (style > 0.5) {
            n = 1.0 - abs(2.0 * n - 1.0);
        }
        total += n * weight;
        norm += weight;
        weight *= 0.5;
        q *= 2.0;
    }
    return total / max(norm, 1.0e-4);
}

@fragment
fn fragment(vertex_output: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var in = vertex_output;
    @if(VISIBILITY_RANGE_DITHER)
    visibility_range_dither(in.position, in.visibility_range_dither);
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color = alpha_discard(
        pbr_input.material.flags,
        pbr_input.material.alpha_cutoff,
        pbr_input.material.base_color
    );
    // Object-space position, and the gradient of the height along it by central differences about a pixel's footprint. A skinned part has no
    // stable object-space position in the pixel shader (the world position moves with the bones), so the projection stores its rest position
    // in the vertex colour (x / 32 + 0.5 per axis, not skinned, so it interpolates as the rest pose), and the multiplication that colour
    // applied to the base colour is taken back out here.
    let local_from_world = mesh_functions::get_local_from_world(in.instance_index);
    var p = (local_from_world * vec4<f32>(in.world_position.xyz, 1.0)).xyz;
@if(VERTEX_COLORS) {
    p = (in.color.rgb - vec3<f32>(0.5)) * 32.0;
    pbr_input.material.base_color = vec4<f32>(pbr_input.material.base_color.rgb / max(in.color.rgb, vec3<f32>(0.02)), pbr_input.material.base_color.a / max(in.color.a, 0.02));
}
    let n = pbr_input.N;
    let v = pbr_input.V;
    var low = 1.0; // 1 in the low parts of the relief (where water collects), 0 on its tops
    if (detail.a.x > 0.0) {
        let e = 0.5 / max(detail.a.y, 1.0e-3) * 0.12;
        let gx = height(p + vec3<f32>(e, 0.0, 0.0)) - height(p - vec3<f32>(e, 0.0, 0.0));
        let gy = height(p + vec3<f32>(0.0, e, 0.0)) - height(p - vec3<f32>(0.0, e, 0.0));
        let gz = height(p + vec3<f32>(0.0, 0.0, e)) - height(p - vec3<f32>(0.0, 0.0, e));
        var gradient_local = vec3<f32>(gx, gy, gz) / (2.0 * e);
        // To world space, then keep only the part along the surface (the part along the normal changes nothing).
        var gradient = (mesh_functions::get_world_from_local(in.instance_index) * vec4<f32>(gradient_local, 0.0)).xyz;
        gradient = gradient - n * dot(gradient, n);
        pbr_input.N = normalize(n - gradient * detail.a.x / max(detail.a.y, 1.0e-3) * 0.25);
        low = 1.0 - smoothstep(0.25, 0.75, height(p));
    }
    let n_dot_v = clamp(dot(pbr_input.N, v), 0.0, 1.0);
    let wet = detail.b.x;
    let iridescence = detail.b.y;
    let sheen = detail.b.z;
    let subsurface = detail.b.w;
    // Wet: darker and glossy, most of all in the crevices.
    if (wet > 0.0) {
        let amount = clamp(wet * (0.55 + 0.45 * low), 0.0, 1.0);
        pbr_input.material.base_color = vec4<f32>(pbr_input.material.base_color.rgb * (1.0 - 0.38 * amount), pbr_input.material.base_color.a);
        pbr_input.material.perceptual_roughness = mix(pbr_input.material.perceptual_roughness, 0.06, amount);
    }
    // Iridescence: a thin-film hue that turns with the view angle, strongest at glancing angles.
    if (iridescence > 0.0) {
        let film = 0.5 + 0.5 * cos(6.2831853 * (vec3<f32>(0.0, 0.33, 0.67) + n_dot_v * 1.6));
        // A shell keeps its own colour and the film shifts it: the mix stays well under one even at full strength.
        let amount = iridescence * (0.12 + 0.4 * (1.0 - n_dot_v));
        pbr_input.material.base_color = vec4<f32>(mix(pbr_input.material.base_color.rgb, film * 0.9, amount), pbr_input.material.base_color.a);
        pbr_input.material.metallic = mix(pbr_input.material.metallic, 0.7, iridescence * 0.6);
    }
    var out: FragmentOutput;
    var lit = apply_pbr_lighting(pbr_input);
    // Sheen: fuzz or velvet brightening at grazing angles, in the surface's own colour.
    if (sheen > 0.0) {
        let rim = pow(1.0 - n_dot_v, 3.0);
        lit = vec4<f32>(lit.rgb + pbr_input.material.base_color.rgb * rim * sheen * (0.25 + luma_of(lit.rgb)), lit.a);
    }
    // Subsurface: light wraps past the shadow edge and comes out warm and a little red.
    if (subsurface > 0.0) {
        let sun = view_bindings::lights.directional_lights[0];
        let sun_color = sun.color.rgb * view_bindings::view.exposure;
        let n_dot_l = dot(pbr_input.N, sun.direction_to_light);
        let wrap = 0.5;
        let wrapped = max((n_dot_l + wrap) / (1.0 + wrap), 0.0) - max(n_dot_l, 0.0);
        let tint = pbr_input.material.base_color.rgb * vec3<f32>(1.0, 0.42, 0.3);
        lit = vec4<f32>(lit.rgb + tint * sun_color * wrapped * subsurface * 0.32, lit.a);
    }
    // Thickness: sunlight passing through a thin body (after Barre-Brisebois, GDC 2011). With the sun behind the surface the view direction lines up with the
    // light bent a little by the normal, and the surface glows in its own colour. It ignores shadow maps, so use it on thin parts only.
    let thickness = detail.c.x;
    if (thickness > 0.0) {
        let sun = view_bindings::lights.directional_lights[0];
        let sun_color = sun.color.rgb * view_bindings::view.exposure;
        let bent = normalize(sun.direction_to_light + pbr_input.N * 0.2);
        let through = pow(clamp(dot(v, -bent), 0.0, 1.0), 3.0);
        let tint = pbr_input.material.base_color.rgb * vec3<f32>(1.4, 1.1, 0.8);
        lit = vec4<f32>(lit.rgb + tint * sun_color * through * thickness * 0.6, lit.a);
    }
    out.color = main_pass_post_lighting_processing(pbr_input, lit);
    return out;
}

fn luma_of(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}
