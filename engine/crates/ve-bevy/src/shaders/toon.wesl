// Cel ("toon") shading as an extension of Bevy's standard material, ported from the forward renderer (`ve-render`'s forward.wgsl).
//
// Bevy lights the surface as usual (sun, shadows, ambient, point lights); this shader then measures how much light reached the pixel
// and replaces the response with flat bands: one lit colour, one tinted shadow colour, hard steps between, an optional rim and a
// small hard highlight. The measurement is the lit colour's brightness over the albedo's, scaled by the sun's own strength, which
// puts it near N.L where the sun reaches the surface and near 0 in shade or in a cast shadow.

import bevy_pbr::render::{
    pbr_types,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
    pbr_fragment::pbr_input_from_standard_material,
    forward_io::{VertexOutput, FragmentOutput},
    mesh_view_bindings as view_bindings,
};
@if(VISIBILITY_RANGE_DITHER)
import bevy_pbr::render::pbr_functions::visibility_range_dither;

// a: x bands (1 to 4), y feather, z shift, w highlight size. b: rgb shadow colour multiplier, w rim strength.
struct ToonParams {
    a: vec4<f32>,
    b: vec4<f32>,
};
@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> toon: ToonParams;

const PI: f32 = 3.14159265;

fn luma(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

// How far the surface is into the lit side, in `bands` flat steps: edges evenly spaced from 0 toward full light, each softened by
// `feather`. Returns 0 (all shadow colour) to 1 (all lit colour).
fn toon_step(x: f32, bands: f32, feather: f32) -> f32 {
    let half_width = max(feather * 0.25, 0.004);
    var total = 0.0;
    for (var k = 0; k < 4; k = k + 1) {
        if (f32(k) < bands) {
            let edge = f32(k) / bands;
            total = total + smoothstep(edge - half_width, edge + half_width, x);
        }
    }
    return total / max(bands, 1.0);
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
    var out: FragmentOutput;
    let lit_pbr = apply_pbr_lighting(pbr_input);
    let base = pbr_input.material.base_color.rgb;
    let sun = view_bindings::lights.directional_lights[0];
    // Light colours are in physical units; the view's exposure brings them to the scale of the lit picture.
    let sun_color = sun.color.rgb * view_bindings::view.exposure;
    let sun_power = max(luma(sun_color) / PI, 1.0e-4);
    let n = pbr_input.N;
    let v = pbr_input.V;
    // The light that reached the pixel, in units of the sun at full strength: about N.L in sun, about 0 in shade (ambient is a small part).
    let reached = luma(lit_pbr.rgb) / max(luma(base), 0.04);
    let ambient = sun_power * 0.12;
    let t = clamp((reached - ambient) / sun_power, 0.0, 1.0);
    let bands = toon.a.x;
    let sun_step = toon_step(t + toon.a.z, bands, toon.a.y);
    let lit = base * (ambient + sun_power);
    let shadowed = base * toon.b.rgb * (ambient + sun_power * 0.12);
    var color = mix(shadowed, lit, sun_step);
    // Rim: a bright edge where the surface turns away from the viewer, only on the side the sun reaches.
    let rim_mask = pow(1.0 - clamp(dot(n, v), 0.0, 1.0), 3.0) * smoothstep(-0.1, 0.3, t);
    color = color + base * (sun_power + ambient) * rim_mask * toon.b.w;
    // Highlight: a small hard-edged blob where the half vector meets the normal.
    if (toon.a.w > 0.0) {
        let half_vector = normalize(v + sun.direction_to_light);
        let threshold = 1.0 - toon.a.w * 0.06;
        let blob = smoothstep(threshold, threshold + 0.004, dot(n, half_vector)) * step(0.05, t);
        color = color + sun_color * 0.18 * blob;
    }
    // Glow stays as authored.
    color = color + pbr_input.material.emissive.rgb;
    out.color = main_pass_post_lighting_processing(pbr_input, vec4<f32>(color, pbr_input.material.base_color.a));
    return out;
}
