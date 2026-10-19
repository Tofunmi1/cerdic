// Forward pass: metallic-roughness PBR (GGX specular, Lambert diffuse) with one sun, up to sixteen
// point lights and constant ambient, then ACES tonemapping and viewer grading. WebGL2 has no storage
// buffers, so the model matrix and the material travel as per-instance vertex attributes and the
// lights as one uniform block. The output target is an sRGB view, so values are written in linear
// space.

struct View {
    clip_from_world: mat4x4<f32>,
    camera_position: vec4<f32>,
    exposure: vec4<f32>,
    // x: contrast, y: saturation (viewer grading, applied after tonemapping).
    grading: vec4<f32>,
    // rgb: per-channel tint multiplier applied last.
    tint: vec4<f32>,
    // x enables geometric specular anti-aliasing.
    effects: vec4<f32>,
    world_from_clip: mat4x4<f32>,
    viewport: vec4<f32>,
    horizon: vec4<f32>,
    zenith: vec4<f32>,
    ground: vec4<f32>,
    sun: vec4<f32>,
    cloud: vec4<f32>,
};

struct PointLight {
    position_range: vec4<f32>,
    color: vec4<f32>,
};

struct SpotLight {
    position_range: vec4<f32>,
    direction_outer: vec4<f32>,
    color_inner: vec4<f32>,
};

struct Lights {
    ambient: vec4<f32>,
    // Low-order (L1) sky/ground irradiance band: rgb constant term, then rgb the +Y directional term.
    ambient_sh0: vec4<f32>,
    ambient_sh1: vec4<f32>,
    // rgb warm ground-bounce colour, w strength (0 turns it off).
    ambient_bounce: vec4<f32>,
    sun_direction: vec4<f32>,
    sun_color: vec4<f32>,
    counts: vec4<f32>,
    // One matrix per shadow cascade (world to the sun's clip space).
    sun_shadow_from_world: array<mat4x4<f32>, 3>,
    // x: number of cascades (0 is off), y: filter radius in taps, z and w: one atlas texel in uv units along x and y.
    shadow_params: vec4<f32>,
    // xyz: far edge of each cascade along the viewing direction (metres).
    shadow_splits: vec4<f32>,
    // xyz: normal-offset bias of each cascade (metres).
    shadow_bias: vec4<f32>,
    // xyz: the viewing direction the splits are measured along.
    shadow_forward: vec4<f32>,
    points: array<PointLight, 16>,
    spots: array<SpotLight, 8>,
};

@group(0) @binding(0) var<uniform> view: View;
@group(1) @binding(0) var<uniform> lights: Lights;
@group(1) @binding(1) var shadow_map: texture_depth_2d;
@group(1) @binding(2) var shadow_sampler: sampler_comparison;
// Baked maps of textured surfaces, one layer each: albedo (sRGB), normal, metallic-roughness, emissive (sRGB).
@group(2) @binding(0) var material_maps: texture_2d_array<f32>;
@group(2) @binding(1) var material_sampler: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) occlusion: f32,
    @location(3) model_0: vec4<f32>,
    @location(4) model_1: vec4<f32>,
    @location(5) model_2: vec4<f32>,
    @location(6) model_3: vec4<f32>,
    @location(7) base_color: vec4<f32>,
    @location(8) params: vec4<f32>,
    @location(9) emissive: vec4<f32>,
    // Texture-array layers of the albedo, normal, metallic-roughness and emissive maps (negative: none).
    @location(10) maps: vec4<f32>,
    @location(11) uv: vec2<f32>,
    // Cel shading: x bands (0 is off), y feather, z shift, w highlight size; then rgb shadow colour, w rim.
    @location(12) toon_a: vec4<f32>,
    @location(13) toon_b: vec4<f32>,
    // rgb outline colour (linear), w outline width in screen pixels (0 draws none).
    @location(14) toon_c: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) @interpolate(flat) base_color: vec4<f32>,
    @location(3) @interpolate(flat) params: vec4<f32>,
    @location(4) @interpolate(flat) emissive: vec4<f32>,
    @location(5) occlusion: f32,
    @location(6) object_position: vec3<f32>,
    @location(7) @interpolate(flat) maps: vec4<f32>,
    @location(8) uv: vec2<f32>,
    @location(9) @interpolate(flat) toon_a: vec4<f32>,
    @location(10) @interpolate(flat) toon_b: vec4<f32>,
};

// The per-vertex occlusion slot of a static mesh carries three things in one exactly-representable float (an integer
// above 2^14): occlusion in 4 bits and the corner-welded smooth normal as two 10-bit octahedral coordinates. The outline
// pass offsets along that smooth normal so hull faces meet at hard corners. Plain occlusion (0..1, skinned meshes) is
// left as it is and the outline falls back to the face normal.
fn unpack_occlusion(packed: f32) -> f32 {
    if (packed < 2.0) {
        return packed;
    }
    return f32(u32(packed) & 15u) / 15.0;
}

fn outline_normal(packed: f32, normal: vec3<f32>) -> vec3<f32> {
    if (packed < 2.0) {
        return normal;
    }
    let bits = u32(packed) >> 4u;
    let oct = vec2<f32>(f32((bits & 1023u) - 1u), f32(((bits >> 10u) & 1023u) - 1u)) / 1021.0 * 2.0 - 1.0;
    var n = vec3<f32>(oct.x, oct.y, 1.0 - abs(oct.x) - abs(oct.y));
    if (n.z < 0.0) {
        let folded = (vec2<f32>(1.0) - abs(vec2<f32>(n.y, n.x))) * vec2<f32>(select(-1.0, 1.0, n.x >= 0.0), select(-1.0, 1.0, n.y >= 0.0));
        n = vec3<f32>(folded.x, folded.y, n.z);
    }
    return normalize(n);
}

@vertex
fn vertex(input: VertexInput) -> VertexOutput {
    let model = mat4x4<f32>(input.model_0, input.model_1, input.model_2, input.model_3);
    var position = input.position;
    // Recipe 10 is water. Its fourth material parameter is the authored speed; the first is
    // the shared viewer animation clock. Other recipes take this branch never and remain unchanged.
    if input.params.z > 9.5 && input.params.z < 10.5 && input.normal.y > 0.7 {
        let phase = input.position.x * 1.7 + input.position.z * 2.3 + input.params.x * input.params.w * 1.4;
        position.y += 0.018 * sin(phase) + 0.008 * sin(input.position.x * 3.1 - input.position.z * 1.2 + input.params.x * input.params.w * 0.8);
    }
    let world = model * vec4<f32>(position, 1.0);
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
    out.occlusion = unpack_occlusion(input.occlusion);
    out.object_position = input.position;
    out.maps = input.maps;
    out.uv = input.uv;
    out.toon_a = input.toon_a;
    out.toon_b = input.toon_b;
    return out;
}

struct OutlineOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) @interpolate(flat) color: vec4<f32>,
};

// The toon outline: the same mesh drawn again with front faces culled, every vertex pushed outward along its normal by a
// fixed number of screen pixels (so the line keeps its width at any distance). Instances without an outline are moved
// outside the clip volume and vanish.
@vertex
fn vertex_outline(input: VertexInput) -> OutlineOutput {
    var out: OutlineOutput;
    let width = input.toon_c.w;
    if (width <= 0.0) {
        out.clip_position = vec4<f32>(2.0, 2.0, 2.0, 1.0);
        out.color = vec4<f32>(0.0);
        return out;
    }
    let model = mat4x4<f32>(input.model_0, input.model_1, input.model_2, input.model_3);
    let world = model * vec4<f32>(input.position, 1.0);
    let c0 = model[0].xyz;
    let c1 = model[1].xyz;
    let c2 = model[2].xyz;
    let world_normal = normalize(mat3x3<f32>(cross(c1, c2), cross(c2, c0), cross(c0, c1)) * outline_normal(input.occlusion, input.normal));
    let clip = view.clip_from_world * world;
    let size = max(view.viewport.zw, vec2<f32>(1.0));
    let direction_clip = (view.clip_from_world * vec4<f32>(world_normal, 0.0)).xy;
    let direction_pixels = direction_clip * size;
    let length_pixels = length(direction_pixels);
    var offset = vec2<f32>(0.0);
    if (length_pixels > 1.0e-6) {
        offset = direction_pixels / length_pixels / size * 2.0 * width * clip.w;
    }
    out.clip_position = vec4<f32>(clip.xy + offset, clip.z, clip.w);
    out.color = input.toon_c;
    return out;
}

@fragment
fn fragment_outline(input: OutlineOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(grade(input.color.rgb), 1.0);
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let low = c / 12.92;
    let high = pow((c + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4));
    return select(high, low, c <= vec3<f32>(0.04045));
}

// Perturbs `n` by a tangent-space normal `map_n`, building the tangent frame from screen-space derivatives
// of the position and the texture coordinate (no vertex tangents needed). Texture v grows downward and
// glTF's +Y is up, so the frame is taken on (u, -v).
fn perturb_normal(n: vec3<f32>, p: vec3<f32>, uv: vec2<f32>, map_n: vec3<f32>) -> vec3<f32> {
    let dp1 = dpdx(p);
    let dp2 = dpdy(p);
    let uvf = vec2<f32>(uv.x, -uv.y);
    let duv1 = dpdx(uvf);
    let duv2 = dpdy(uvf);
    let dp2perp = cross(dp2, n);
    let dp1perp = cross(n, dp1);
    let t = dp2perp * duv1.x + dp1perp * duv2.x;
    let b = dp2perp * duv1.y + dp1perp * duv2.y;
    let invmax = inverseSqrt(max(max(dot(t, t), dot(b, b)), 1.0e-20));
    return normalize(mat3x3<f32>(t * invmax, b * invmax, n) * map_n);
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

// The authored sky along `dir`: the horizon to zenith and horizon to ground gradients the sky pass draws (same constants), blurred toward
// the sky's average colour as the surface gets rougher. The sun itself is not here: it is a direct light with its own specular.
fn sky_environment(dir: vec3<f32>, roughness: f32) -> vec3<f32> {
    let up = dir.y;
    let upper = mix(view.horizon.rgb, view.zenith.rgb, smoothstep(0.0, 0.85, max(up, 0.0)));
    let lower = mix(view.horizon.rgb, view.ground.rgb, 1.0 - smoothstep(-0.35, 0.0, min(up, 0.0)));
    let sharp = select(lower, upper, up >= 0.0);
    let average = 0.5 * (mix(view.horizon.rgb, view.zenith.rgb, 0.5) + mix(view.horizon.rgb, view.ground.rgb, 0.5));
    return mix(sharp, average, roughness * roughness * 0.7);
}

// A procedural studio, seen from the middle: a blue-grey sky that brightens toward the horizon, a
// dim floor, a large soft light box up and to the front, and a cooler strip light low at the side.
// Rough surfaces blur the boxes (wider, dimmer) toward the average colour, standing in for a prefiltered
// environment map.
fn environment(dir: vec3<f32>, roughness: f32) -> vec3<f32> {
    let blur = roughness * roughness;
    // With an authored sky (the view carries its colours, flagged in horizon.w) surfaces reflect that sky, not the studio.
    if (view.horizon.w > 0.5) {
        return sky_environment(dir, roughness);
    }
    let sky = mix(vec3<f32>(0.62, 0.68, 0.76), vec3<f32>(0.20, 0.32, 0.58), clamp(dir.y, 0.0, 1.0));
    let floor_color = vec3<f32>(0.34, 0.31, 0.28);
    var env = mix(floor_color, sky, smoothstep(-0.2, 0.2, dir.y));
    let key = normalize(vec3<f32>(-0.4, 0.75, 0.55));
    let key_edge = mix(0.93, 0.35, blur);
    env += vec3<f32>(5.0, 4.8, 4.5) * mix(1.0, 0.25, blur) * smoothstep(key_edge, key_edge + 0.05, dot(dir, key));
    let rim = normalize(vec3<f32>(0.85, 0.25, -0.45));
    let rim_edge = mix(0.90, 0.30, blur);
    env += vec3<f32>(1.2, 1.5, 2.0) * mix(1.0, 0.25, blur) * smoothstep(rim_edge, rim_edge + 0.05, dot(dir, rim));
    return mix(env, vec3<f32>(0.40, 0.44, 0.52), blur * 0.6);
}

// Split-sum environment BRDF, Karis's analytic fit: the scale and bias on f0 for a given roughness.
fn env_brdf(f0: vec3<f32>, roughness: f32, n_dot_v: f32) -> vec3<f32> {
    let c0 = vec4<f32>(-1.0, -0.0275, -0.572, 0.022);
    let c1 = vec4<f32>(1.0, 0.0425, 1.04, -0.04);
    let r = roughness * c0 + c1;
    let a004 = min(r.x * r.x, exp2(-9.28 * n_dot_v)) * r.x + r.y;
    let ab = vec2<f32>(-1.04, 1.04) * a004 + r.zw;
    return f0 * ab.x + vec3<f32>(ab.y);
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

// Viewer grading on the tonemapped display-referred colour: contrast around mid grey, saturation
// around Rec. 709 luminance, then a per-channel tint. Mirrors `Grading::grade` in `grade.rs`.
fn grade(color: vec3<f32>) -> vec3<f32> {
    var c = (color - vec3<f32>(0.5)) * view.grading.x + vec3<f32>(0.5);
    let luminance = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
    c = mix(vec3<f32>(luminance), c, view.grading.y);
    return c * view.tint.rgb;
}

fn hash3(p: vec3<f32>) -> f32 {
    return fract(sin(dot(p, vec3<f32>(127.1, 311.7, 74.7))) * 43758.5453);
}

fn noise3(p: vec3<f32>) -> f32 {
    let cell = floor(p);
    let f = fract(p);
    let blend = f * f * (vec3<f32>(3.0) - 2.0 * f);
    let x00 = mix(hash3(cell), hash3(cell + vec3<f32>(1.0, 0.0, 0.0)), blend.x);
    let x10 = mix(hash3(cell + vec3<f32>(0.0, 1.0, 0.0)), hash3(cell + vec3<f32>(1.0, 1.0, 0.0)), blend.x);
    let x01 = mix(hash3(cell + vec3<f32>(0.0, 0.0, 1.0)), hash3(cell + vec3<f32>(1.0, 0.0, 1.0)), blend.x);
    let x11 = mix(hash3(cell + vec3<f32>(0.0, 1.0, 1.0)), hash3(cell + vec3<f32>(1.0, 1.0, 1.0)), blend.x);
    return mix(mix(x00, x10, blend.y), mix(x01, x11, blend.y), blend.z);
}

// Returns albedo multiplier, roughness offset and a low-amplitude height field.
fn procedural_surface(recipe: f32, position: vec3<f32>, seed: f32) -> vec3<f32> {
    if (recipe < 0.5) {
        return vec3<f32>(1.0, 0.0, 0.0);
    }
    let fine = noise3(position * 3.0);
    let broad = noise3(position * 0.7);
    let instance = fract(sin(seed * 91.17 + 17.13) * 43758.5453);
    var shade = 0.92 + instance * 0.16;
    var roughness = 0.0;
    var height = (fine - 0.5) * 0.012;
    if (recipe < 1.5) {
        let grain = sin(position.z * 24.0 + broad * 5.0);
        shade *= 0.84 + 0.16 * (0.5 + 0.5 * grain);
        height += grain * 0.008;
        roughness = (fine - 0.5) * 0.12;
    } else if (recipe < 2.5) {
        let seam = step(0.965, fract(position.z * 1.4 + floor(position.x * 0.35) * 0.5));
        let board = 0.92 + 0.16 * fract(sin(floor(position.x * 0.35) * 19.19 + floor(position.z * 1.4) * 7.7) * 43758.5453);
        shade *= mix(board * (0.92 + 0.12 * fine), 0.28, seam);
        height += seam * -0.035;
        roughness = (fine - 0.5) * 0.16;
    } else if (recipe < 3.5) {
        let brush = noise3(position * vec3<f32>(2.0, 36.0, 2.0));
        shade *= 0.88 + 0.12 * brush;
        height += (brush - 0.5) * 0.003;
        roughness = (brush - 0.5) * 0.18;
    } else if (recipe < 4.5) {
        let pebble = smoothstep(0.38, 0.72, fine);
        shade *= 0.83 + 0.24 * pebble;
        height += (pebble - 0.5) * 0.018;
        roughness = (fine - 0.5) * 0.1;
    } else if (recipe < 5.5) {
        let weave = 0.5 + 0.25 * sin(position.x * 38.0) + 0.25 * sin(position.z * 38.0);
        shade *= 0.82 + weave * 0.22;
        height += (weave - 0.5) * 0.012;
        roughness = 0.08 + (fine - 0.5) * 0.08;
    } else if (recipe < 6.5) {
        let row = floor(position.y * 1.8);
        let seam_x = step(0.94, fract(position.x * 1.2 + fract(row * 0.5)));
        let seam_y = step(0.93, fract(position.y * 1.8));
        let mortar = max(seam_x, seam_y);
        shade *= mix(0.86 + fine * 0.22, 0.32, mortar);
        height += mortar * -0.028;
        roughness = 0.08 + fine * 0.12;
    } else if (recipe < 7.5) {
        shade *= 0.86 + 0.22 * fine + 0.08 * broad;
        height += (fine - 0.5) * 0.016;
        roughness = 0.12 + fine * 0.12;
    } else if (recipe < 8.5) {
        let vein = abs(sin(position.x * 5.0 + position.y * 2.0 + broad * 8.0));
        let marble = 1.0 - smoothstep(0.0, 0.16, vein);
        shade *= 0.82 + 0.22 * fine + 0.12 * marble;
        height += marble * 0.008;
        roughness = (fine - 0.5) * 0.08;
    } else {
        shade *= 0.96 + 0.08 * fine;
        height += (fine - 0.5) * 0.004;
        roughness = (fine - 0.5) * 0.05;
    }
    return vec3<f32>(shade, roughness, height);
}

// Fraction of the sun that reaches this point: 1 in the open, 0 in full shadow. Looks up the sun's
// shadow map with a comparison sampler (a hardware-filtered tap, or a grid of them), pushing the
// lookup along the normal so surfaces do not shadow themselves. Level 0 sampling needs no derivatives,
// so the early returns are fine.
fn cascade_split(index: i32) -> f32 {
    if (index == 0) { return lights.shadow_splits.x; }
    if (index == 1) { return lights.shadow_splits.y; }
    return lights.shadow_splits.z;
}

fn cascade_bias(index: i32) -> f32 {
    if (index == 0) { return lights.shadow_bias.x; }
    if (index == 1) { return lights.shadow_bias.y; }
    return lights.shadow_bias.z;
}

// Visibility of one point in one cascade of the atlas (the cascades sit side by side), or 1 outside its box.
fn cascade_visibility(index: i32, count: f32, world_position: vec3<f32>, normal: vec3<f32>) -> f32 {
    let biased = world_position + normal * cascade_bias(index);
    let clip = lights.sun_shadow_from_world[index] * vec4<f32>(biased, 1.0);
    let ndc = clip.xyz / clip.w;
    if (abs(ndc.x) > 1.0 || abs(ndc.y) > 1.0 || ndc.z > 1.0) {
        return 1.0;
    }
    let uv = vec2<f32>((ndc.x * 0.5 + 0.5 + f32(index)) / count, 0.5 - ndc.y * 0.5);
    let radius = i32(lights.shadow_params.y);
    // Keep the filter taps inside this cascade's tile, so they never read a neighbour's depths.
    let low = vec2<f32>(f32(index) / count + lights.shadow_params.z * f32(radius), lights.shadow_params.w * f32(radius));
    let high = vec2<f32>(f32(index + 1) / count - lights.shadow_params.z * f32(radius), 1.0 - lights.shadow_params.w * f32(radius));
    let centre = clamp(uv, low, high);
    var lit = 0.0;
    for (var dy = -radius; dy <= radius; dy = dy + 1) {
        for (var dx = -radius; dx <= radius; dx = dx + 1) {
            let offset = vec2<f32>(f32(dx) * lights.shadow_params.z, f32(dy) * lights.shadow_params.w);
            lit += textureSampleCompareLevel(shadow_map, shadow_sampler, centre + offset, ndc.z);
        }
    }
    let side = f32(2 * radius + 1);
    return lit / (side * side);
}

// Cascaded sun shadow. The cascade is chosen by distance along the view; the last tenth of each cascade blends into the next, and the
// last cascade fades to light at its far edge, so no seam or cut-off shows. Level 0 sampling needs no derivatives, so the early
// returns are fine.
fn sun_visibility(world_position: vec3<f32>, normal: vec3<f32>) -> f32 {
    let count = lights.shadow_params.x;
    if (count < 0.5) {
        return 1.0;
    }
    let depth = dot(world_position - view.camera_position.xyz, lights.shadow_forward.xyz);
    let last = i32(count) - 1;
    var cascade = 0;
    for (var i = 0; i < last; i = i + 1) {
        if (depth > cascade_split(i)) {
            cascade = i + 1;
        }
    }
    var visibility = cascade_visibility(cascade, count, world_position, normal);
    let far = cascade_split(cascade);
    let blend = smoothstep(far * 0.9, far, depth);
    if (cascade < last) {
        if (blend > 0.0) {
            visibility = mix(visibility, cascade_visibility(cascade + 1, count, world_position, normal), blend);
        }
    } else {
        visibility = mix(visibility, 1.0, blend);
    }
    return visibility;
}

// Cel lighting: how far the surface is into the lit side, in `bands` flat steps. `x` is N.L plus the authored shift, from -1
// to 1; the first edge sits where light just grazes the surface (x = 0) and the others are evenly spaced toward full light.
// `feather` softens each edge. Returns 0 (all shadow colour) to 1 (all lit colour).
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
fn fragment(input: VertexOutput) -> @location(0) vec4<f32> {
    let pattern = procedural_surface(
        input.params.z,
        input.object_position * clamp(input.params.w, 0.1, 50.0),
        input.emissive.a,
    );
    let bump_gradient = dpdx(input.world_position) * dpdx(pattern.z)
        + dpdy(input.world_position) * dpdy(pattern.z);
    var n = normalize(input.world_normal - bump_gradient * 0.035);
    let v = normalize(view.camera_position.xyz - input.world_position);
    // Baked maps: sampled unconditionally (the layer index is clamped) so derivatives stay uniform, then
    // used only where the surface has them.
    let albedo_layer = max(i32(input.maps.x), 0);
    let normal_layer = max(i32(input.maps.y), 0);
    let mr_layer = max(i32(input.maps.z), 0);
    let emissive_layer = max(i32(input.maps.w), 0);
    let albedo_map = textureSample(material_maps, material_sampler, input.uv, albedo_layer);
    let normal_map = textureSample(material_maps, material_sampler, input.uv, normal_layer);
    let mr_map = textureSample(material_maps, material_sampler, input.uv, mr_layer);
    let emissive_map = textureSample(material_maps, material_sampler, input.uv, emissive_layer);
    // Derivatives must be taken in uniform control flow, so the mapped normal is always computed.
    let mapped_normal = perturb_normal(normalize(input.world_normal), input.world_position, input.uv, normalize(normal_map.xyz * 2.0 - vec3<f32>(1.0)));
    n = select(n, mapped_normal, input.maps.y >= 0.0);
    var metallic = input.params.x;
    if input.params.z > 9.5 && input.params.z < 10.5 { metallic = 0.0; }
    var surface_roughness = input.params.y;
    if (input.maps.z >= 0.0) {
        // glTF packing: green is roughness, blue is metallic, both multiplying the factors.
        surface_roughness = surface_roughness * mr_map.g;
        metallic = metallic * mr_map.b;
    }
    let base_roughness = clamp(surface_roughness + pattern.y, 0.045, 1.0);
    let normal_dx = dpdx(n);
    let normal_dy = dpdy(n);
    let variance = max(dot(normal_dx, normal_dx) + dot(normal_dy, normal_dy), 0.0);
    let alpha = clamp(base_roughness * base_roughness + variance * view.effects.x, 0.045 * 0.045, 1.0);
    let roughness = sqrt(alpha);
    var recipe_color = input.base_color.rgb;
    if (input.params.z > 0.5) {
        let hue_shift = fract(sin(input.emissive.a * 23.17 + 3.11) * 43758.5453) * 0.08;
        recipe_color = mix(recipe_color, recipe_color.gbr, hue_shift);
    }
    if (input.maps.x >= 0.0) {
        recipe_color = recipe_color * srgb_to_linear(albedo_map.rgb);
    }
    var emitted = input.emissive.rgb;
    if (input.maps.w >= 0.0) {
        emitted = emitted + srgb_to_linear(emissive_map.rgb);
    }
    let base = recipe_color * pattern.x;
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
    let spot_count = i32(lights.counts.y);
    for (var i = 0; i < spot_count; i = i + 1) {
        let light = lights.spots[i];
        let to_light = light.position_range.xyz - input.world_position;
        let distance_squared = max(dot(to_light, to_light), 0.0001);
        let range = max(light.position_range.w, 0.0001);
        let window = clamp(1.0 - pow(distance_squared / (range * range), 2.0), 0.0, 1.0);
        let attenuation = window * window / distance_squared;
        let toward_light = to_light * inverseSqrt(distance_squared);
        let cone_cosine = dot(normalize(light.direction_outer.xyz), -toward_light);
        let cone = smoothstep(light.direction_outer.w, light.color_inner.w, cone_cosine);
        radiance += shade(n, v, toward_light, light.color_inner.rgb * attenuation * cone, diffuse_color, f0, alpha);
    }

    // Low-order (L1) spherical-harmonic ambient irradiance. A two-colour sky/ground hemisphere is
    // exactly the first-order band along Y, so this reproduces the sky-to-ground gradient exactly;
    // it is a bounded approximation of an environment, not global illumination.
    let ambient_irradiance = lights.ambient_sh0.rgb + lights.ambient_sh1.rgb * clamp(n.y, -1.0, 1.0);
    // Optional warm ground bounce: light leaving a warm floor reaches downward-facing surfaces.
    let bounce_term = lights.ambient_bounce.rgb * (lights.ambient_bounce.w * max(-n.y, 0.0))
        * diffuse_color * input.occlusion;
    var ambient = ambient_irradiance * (diffuse_color + f0 * 0.2) * input.occlusion + bounce_term;
    if (lights.ambient.w > 0.0) {
        // The reflected sky replaces the flat specular ambient, so metal shows the studio around it.
        let n_dot_v = max(dot(n, v), 0.0001);
        let facing_up = clamp(n.y * 0.5 + 0.5, 0.6, 1.0);
        let sky = environment(reflect(-v, n), roughness) * env_brdf(f0, roughness, n_dot_v);
        ambient = (ambient_irradiance * diffuse_color + sky * facing_up * lights.ambient.w) * input.occlusion + bounce_term;
    }
    var color = radiance + ambient + emitted;
    if (input.toon_a.x > 0.5) {
        // Cel shading replaces the physically based response: one lit colour, one tinted shadow colour, flat bands between.
        let bands = input.toon_a.x;
        let feather = input.toon_a.y;
        let sun_light = normalize(lights.sun_direction.xyz);
        let sun_power = lights.sun_color.rgb / PI;
        let flat_ambient = lights.ambient_sh0.rgb + lights.ambient_sh1.rgb * 0.25;
        let n_dot_sun = dot(n, sun_light);
        // Cast shadows are all-or-nothing in a cel look: the shadow map's soft edge is snapped.
        let unshadowed = smoothstep(0.4, 0.6, sun_visibility(input.world_position, n));
        let sun_step = toon_step(n_dot_sun + input.toon_a.z, bands, feather) * unshadowed;
        let lit = base * (flat_ambient + sun_power);
        let shadowed = base * input.toon_b.rgb * (flat_ambient + sun_power * 0.12);
        var toon = mix(shadowed, lit, sun_step);
        // Other lights add their own banded light on top.
        for (var i = 0; i < count; i = i + 1) {
            let light = lights.points[i];
            let to_light = light.position_range.xyz - input.world_position;
            let distance_squared = max(dot(to_light, to_light), 0.0001);
            let range = max(light.position_range.w, 0.0001);
            let window = clamp(1.0 - pow(distance_squared / (range * range), 2.0), 0.0, 1.0);
            let attenuation = window * window / distance_squared;
            let step_light = toon_step(dot(n, to_light * inverseSqrt(distance_squared)) + input.toon_a.z, bands, feather);
            toon = toon + base * light.color.rgb * attenuation / PI * step_light;
        }
        // Rim: a bright edge where the surface turns away from the viewer, only on the side the sun reaches.
        let rim_mask = pow(1.0 - clamp(dot(n, v), 0.0, 1.0), 3.0) * smoothstep(-0.1, 0.3, n_dot_sun) * unshadowed;
        toon = toon + base * (sun_power + flat_ambient) * rim_mask * input.toon_b.w;
        // Highlight: a small hard-edged blob where the half vector meets the normal.
        if (input.toon_a.w > 0.0) {
            let half_vector = normalize(v + sun_light);
            let threshold = 1.0 - input.toon_a.w * 0.06;
            let blob = smoothstep(threshold, threshold + 0.004, dot(n, half_vector)) * step(0.0, n_dot_sun) * unshadowed;
            toon = toon + lights.sun_color.rgb * 0.18 * blob;
        }
        // The baked ambient occlusion is a smooth gradient, which a cel look does not have: it is left out here.
        color = toon + emitted;
    }
    return vec4<f32>(grade(tonemap_aces(color * view.exposure.x)), input.base_color.a);
}
