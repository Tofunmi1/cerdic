// Forward pass: metallic-roughness PBR (GGX specular, Lambert diffuse) with one sun, up to eight
// point lights and constant ambient, then ACES tonemapping. WebGL2 has no storage buffers, so the
// model matrix and the material travel as per-instance vertex attributes and the lights as one
// uniform block. The output target is an sRGB view, so values are written in linear space.

struct View {
    clip_from_world: mat4x4<f32>,
    camera_position: vec4<f32>,
    exposure: vec4<f32>,
};

struct PointLight {
    position_range: vec4<f32>,
    color: vec4<f32>,
};

struct Lights {
    ambient: vec4<f32>,
    sun_direction: vec4<f32>,
    sun_color: vec4<f32>,
    counts: vec4<f32>,
    sun_shadow_from_world: mat4x4<f32>,
    // x: shadows on (1) or off (0), y: filter radius in taps, z: one texel in uv units, w: normal bias.
    shadow_params: vec4<f32>,
    points: array<PointLight, 8>,
};

@group(0) @binding(0) var<uniform> view: View;
@group(1) @binding(0) var<uniform> lights: Lights;
@group(1) @binding(1) var shadow_map: texture_depth_2d;
@group(1) @binding(2) var shadow_sampler: sampler_comparison;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(3) model_0: vec4<f32>,
    @location(4) model_1: vec4<f32>,
    @location(5) model_2: vec4<f32>,
    @location(6) model_3: vec4<f32>,
    @location(7) base_color: vec4<f32>,
    @location(8) params: vec4<f32>,
    @location(9) emissive: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) @interpolate(flat) base_color: vec4<f32>,
    @location(3) @interpolate(flat) params: vec4<f32>,
    @location(4) @interpolate(flat) emissive: vec4<f32>,
};

@vertex
fn vertex(input: VertexInput) -> VertexOutput {
    let model = mat4x4<f32>(input.model_0, input.model_1, input.model_2, input.model_3);
    let world = model * vec4<f32>(input.position, 1.0);
    var out: VertexOutput;
    out.clip_position = view.clip_from_world * world;
    out.world_position = world.xyz;
    // The normal matrix is the inverse transpose of the model's 3x3, which stays correct under
    // non-uniform scale. The cofactor matrix (cross products of the columns) is the inverse transpose
    // times the determinant, and needs no division, so a collapsed axis gives a zero normal instead
    // of NaN. Instances with a non-positive determinant are not drawn (see `drawable` in extract.rs).
    let c0 = model[0].xyz;
    let c1 = model[1].xyz;
    let c2 = model[2].xyz;
    out.world_normal = mat3x3<f32>(cross(c1, c2), cross(c2, c0), cross(c0, c1)) * input.normal;
    out.base_color = input.base_color;
    out.params = input.params;
    out.emissive = input.emissive;
    return out;
}

const PI: f32 = 3.14159265359;

fn distribution_ggx(n_dot_h: f32, alpha: f32) -> f32 {
    let a2 = alpha * alpha;
    let d = n_dot_h * n_dot_h * (a2 - 1.0) + 1.0;
    return a2 / (PI * d * d);
}

// Height-correlated Smith visibility term (already divided by 4 * n_dot_l * n_dot_v).
fn visibility_smith(n_dot_v: f32, n_dot_l: f32, alpha: f32) -> f32 {
    let a2 = alpha * alpha;
    let gv = n_dot_l * sqrt(n_dot_v * n_dot_v * (1.0 - a2) + a2);
    let gl = n_dot_v * sqrt(n_dot_l * n_dot_l * (1.0 - a2) + a2);
    return 0.5 / max(gv + gl, 0.0001);
}

fn fresnel_schlick(v_dot_h: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (vec3<f32>(1.0) - f0) * pow(clamp(1.0 - v_dot_h, 0.0, 1.0), 5.0);
}

// Outgoing radiance from one light with incoming `radiance` arriving along `l` (toward the light).
fn shade(
    n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, radiance: vec3<f32>,
    diffuse_color: vec3<f32>, f0: vec3<f32>, alpha: f32,
) -> vec3<f32> {
    let n_dot_l = max(dot(n, l), 0.0);
    if (n_dot_l <= 0.0) {
        return vec3<f32>(0.0);
    }
    let h = normalize(v + l);
    let n_dot_v = max(dot(n, v), 0.0001);
    let n_dot_h = max(dot(n, h), 0.0);
    let v_dot_h = max(dot(v, h), 0.0);
    let f = fresnel_schlick(v_dot_h, f0);
    let specular = distribution_ggx(n_dot_h, alpha) * visibility_smith(n_dot_v, n_dot_l, alpha) * f;
    let diffuse = (vec3<f32>(1.0) - f) * diffuse_color / PI;
    return (diffuse + specular) * radiance * n_dot_l;
}

// Narkowicz ACES approximation.
fn tonemap_aces(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

// Fraction of the sun that reaches this point: 1 in the open, 0 in full shadow. Looks up the sun's
// shadow map with a comparison sampler (a hardware-filtered tap, or a grid of them), pushing the
// lookup along the normal so surfaces do not shadow themselves. Level 0 sampling needs no derivatives,
// so the early returns are fine.
fn sun_visibility(world_position: vec3<f32>, normal: vec3<f32>) -> f32 {
    if (lights.shadow_params.x < 0.5) {
        return 1.0;
    }
    let biased = world_position + normal * lights.shadow_params.w;
    let clip = lights.sun_shadow_from_world * vec4<f32>(biased, 1.0);
    let ndc = clip.xyz / clip.w;
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 || ndc.z > 1.0) {
        return 1.0;
    }
    let radius = i32(lights.shadow_params.y);
    var lit = 0.0;
    for (var dy = -radius; dy <= radius; dy = dy + 1) {
        for (var dx = -radius; dx <= radius; dx = dx + 1) {
            let offset = vec2<f32>(f32(dx), f32(dy)) * lights.shadow_params.z;
            lit += textureSampleCompareLevel(shadow_map, shadow_sampler, uv + offset, ndc.z);
        }
    }
    let side = f32(2 * radius + 1);
    return lit / (side * side);
}

@fragment
fn fragment(input: VertexOutput) -> @location(0) vec4<f32> {
    let n = normalize(input.world_normal);
    let v = normalize(view.camera_position.xyz - input.world_position);
    let metallic = input.params.x;
    let roughness = clamp(input.params.y, 0.045, 1.0);
    let alpha = roughness * roughness;
    let base = input.base_color.rgb;
    let diffuse_color = base * (1.0 - metallic);
    let f0 = mix(vec3<f32>(0.04), base, metallic);

    var radiance = vec3<f32>(0.0);
    if (dot(lights.sun_color.rgb, vec3<f32>(1.0)) > 0.0) {
        let sun_radiance = shade(n, v, normalize(lights.sun_direction.xyz), lights.sun_color.rgb, diffuse_color, f0, alpha);
        radiance += sun_radiance * sun_visibility(input.world_position, n);
    }
    let count = i32(lights.counts.x);
    for (var i = 0; i < count; i = i + 1) {
        let light = lights.points[i];
        let to_light = light.position_range.xyz - input.world_position;
        let distance_squared = max(dot(to_light, to_light), 0.0001);
        let range = max(light.position_range.w, 0.0001);
        // Inverse-square falloff windowed so the light reaches exactly zero at its range.
        let window = clamp(1.0 - pow(distance_squared / (range * range), 2.0), 0.0, 1.0);
        let attenuation = window * window / distance_squared;
        radiance += shade(n, v, to_light * inverseSqrt(distance_squared), light.color.rgb * attenuation, diffuse_color, f0, alpha);
    }

    let ambient = lights.ambient.rgb * (diffuse_color + f0 * 0.2);
    let color = radiance + ambient + input.emissive.rgb;
    return vec4<f32>(tonemap_aces(color * view.exposure.x), input.base_color.a);
}
