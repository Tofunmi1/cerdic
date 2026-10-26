// Forward PBR, instanced materials and texture-free water; the output is linear for an sRGB view.
struct View {
    clip_from_world: mat4x4<f32>,
    camera_position: vec4<f32>,
    exposure: vec4<f32>,
    // x: contrast, y: saturation (viewer grading, applied after tonemapping), w: grass lighting enabled.
    grading: vec4<f32>,
    // rgb: per-channel tint multiplier applied last.
    tint: vec4<f32>,
    // x: specular AA, y: SSAO, z: bloom, w: stochastic sampling permitted.
    effects: vec4<f32>,
    world_from_clip: mat4x4<f32>,
    viewport: vec4<f32>,
    horizon: vec4<f32>,
    zenith: vec4<f32>,
    ground: vec4<f32>,
    sun: vec4<f32>,
    cloud: vec4<f32>,
    // x, y: unit wind direction on the ground plane (x, z), z: strength, w: seconds on the animation clock.
    wind: vec4<f32>,
};

// `wind_displacement` comes from wind.wgsl, which is put in front of this file when it is built.

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
    // xyz: world position of the first light probe, w: the distance between probes.
    gi_origin_spacing: vec4<f32>,
    // xyz: probes along each axis, w: 1 when a probe volume lights the scene (else the constant sky ambient).
    gi_dims: vec4<f32>,
    points: array<PointLight, 16>,
    spots: array<SpotLight, 8>,
};

@group(0) @binding(0) var<uniform> view: View;
// The automatic exposure now in use: a 1 by 1 float texture, 1 while auto exposure is off (see exposure.rs).
@group(0) @binding(1) var exposure_state: texture_2d<f32>;
@group(1) @binding(0) var<uniform> lights: Lights;
@group(1) @binding(1) var shadow_map: texture_depth_2d;
@group(1) @binding(2) var shadow_sampler: sampler_comparison;
// Light probes (global illumination): irradiance over pi as first-order spherical harmonics, one 3D texture per coefficient (the
// constant term, then the x, y and z terms, each as rgb), with a trilinear sampler. A zero 1 by 1 by 1 stand-in while off.
@group(1) @binding(3) var probe_dc: texture_3d<f32>;
@group(1) @binding(4) var probe_x: texture_3d<f32>;
@group(1) @binding(5) var probe_y: texture_3d<f32>;
@group(1) @binding(6) var probe_z: texture_3d<f32>;
@group(1) @binding(7) var probe_sampler: sampler;
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
    // x: alpha cutoff, y: sway, z: flutter, w: stochastic grid scale (0 is off).
    @location(15) foliage: vec4<f32>,
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
    @location(11) @interpolate(flat) foliage: vec4<f32>,
    @location(12) water_data: vec4<f32>,
};

@vertex
fn vertex(input: VertexInput) -> VertexOutput {
    let model = mat4x4<f32>(input.model_0, input.model_1, input.model_2, input.model_3);
    let position = input.position;
    var world = model * vec4<f32>(position, 1.0);
    // Waves use the same world XZ coordinates and animation clock in both stages.
    if input.params.z > 9.5 && input.params.z < 10.5 && input.normal.y > 0.7 {
        world.y += water_wave_height(world.xz, view.wind.w);
    }
    world = vec4<f32>(world.xyz + wind_displacement(model[3].xyz, world.xyz, input.position, input.foliage), 1.0);
    world = vec4<f32>(grass_world_position(world.xyz, model, input.position, view.camera_position.xyz, water.grass), 1.0);
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
    out.water_data = vec4<f32>(0.5, 0.5, 0.4, 0.0);
    out.object_position = input.position;
    out.maps = input.maps;
    out.foliage = input.foliage;
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
    var world = model * vec4<f32>(input.position, 1.0);
    if (water.grass.look.x > 0.5) {
        let moved = world.xyz + wind_displacement(model[3].xyz, world.xyz, input.position, input.foliage);
        world = vec4<f32>(grass_world_position(moved, model, input.position, view.camera_position.xyz, water.grass), 1.0);
    }
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
