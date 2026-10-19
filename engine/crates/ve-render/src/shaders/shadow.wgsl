// Shadow pass: depth only, from the sun's orthographic camera. It reads the same per-instance model
// matrix as the forward pass (locations 3 to 6) and has no fragment stage.

struct View {
    clip_from_world: mat4x4<f32>,
    camera_position: vec4<f32>,
    exposure: vec4<f32>,
    // Present for layout compatibility with the forward pass; the depth pass does not grade.
    grading: vec4<f32>,
    tint: vec4<f32>,
    effects: vec4<f32>,
    world_from_clip: mat4x4<f32>,
    viewport: vec4<f32>,
    horizon: vec4<f32>,
    zenith: vec4<f32>,
    ground: vec4<f32>,
    sun: vec4<f32>,
    cloud: vec4<f32>,
    wind: vec4<f32>,
};

@group(0) @binding(0) var<uniform> view: View;

// wind:begin
// How far the wind moves a foliage vertex, in world metres. `origin` is the instance's origin, `world` the vertex, `local` its
// position in the shape's own space and `foliage` the instance's (cutoff, sway, flutter). Sway bends the whole shape downwind
// more the higher the vertex is above the origin (a gust travels across the ground, so neighbours are out of step); flutter
// trembles it at a small scale. No sway and no flutter, or no wind, gives zero.
fn wind_displacement(origin: vec3<f32>, world: vec3<f32>, local: vec3<f32>, foliage: vec4<f32>) -> vec3<f32> {
    let strength = view.wind.z;
    if ((foliage.y <= 0.0 && foliage.z <= 0.0) || strength <= 0.0) {
        return vec3<f32>(0.0);
    }
    let t = view.wind.w;
    let direction = vec3<f32>(view.wind.x, 0.0, view.wind.y);
    let height = clamp(world.y - origin.y, 0.0, 6.0);
    let phase = dot(origin.xz, view.wind.xy) * 0.35;
    let gust = 0.6 + 0.4 * sin(t * 1.3 + phase) + 0.25 * sin(t * 2.9 + phase * 1.7);
    let side = vec3<f32>(-direction.z, 0.0, direction.x) * 0.3 * sin(t * 1.9 + phase * 0.6);
    var offset = (direction * gust + side) * (foliage.y * strength * 0.06 * height * height);
    // The shape swings rather than shears: the more it leans, the lower it sits.
    offset.y = -dot(offset, offset) * 0.25 / max(height, 0.5);
    let tremble = vec3<f32>(
        sin(t * 9.0 + local.x * 7.0 + local.z * 3.0),
        sin(t * 11.0 + local.y * 6.0 + local.x * 2.0),
        sin(t * 10.0 + local.z * 8.0 + local.y * 4.0),
    );
    return offset + tremble * (foliage.z * 0.03 * (0.3 + strength));
}
// wind:end

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(3) model_0: vec4<f32>,
    @location(4) model_1: vec4<f32>,
    @location(5) model_2: vec4<f32>,
    @location(6) model_3: vec4<f32>,
    @location(15) foliage: vec4<f32>,
};

@vertex
fn vertex(input: VertexInput) -> @builtin(position) vec4<f32> {
    let model = mat4x4<f32>(input.model_0, input.model_1, input.model_2, input.model_3);
    let world = model * vec4<f32>(input.position, 1.0);
    let moved = world.xyz + wind_displacement(model[3].xyz, world.xyz, input.position, input.foliage);
    return view.clip_from_world * vec4<f32>(moved, 1.0);
}

// Cut-out surfaces (leaves, fences): the same depth, but a pixel whose albedo alpha is under the instance's cutoff casts no
// shadow, so a leaf casts a leaf-shaped shadow. The albedo layer, cutoff and the mesh's texture coordinates come with the
// instance, as in the forward pass; an instance with no cutoff drops nothing.
@group(1) @binding(0) var material_maps: texture_2d_array<f32>;
@group(1) @binding(1) var material_sampler: sampler;

struct CutoutInput {
    @location(0) position: vec3<f32>,
    @location(3) model_0: vec4<f32>,
    @location(4) model_1: vec4<f32>,
    @location(5) model_2: vec4<f32>,
    @location(6) model_3: vec4<f32>,
    @location(10) maps: vec4<f32>,
    @location(11) uv: vec2<f32>,
    @location(15) foliage: vec4<f32>,
};

struct CutoutOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) albedo_layer: f32,
    @location(2) @interpolate(flat) cutoff: f32,
};

@vertex
fn vertex_cutout(input: CutoutInput) -> CutoutOutput {
    let model = mat4x4<f32>(input.model_0, input.model_1, input.model_2, input.model_3);
    var out: CutoutOutput;
    let world = model * vec4<f32>(input.position, 1.0);
    let moved = world.xyz + wind_displacement(model[3].xyz, world.xyz, input.position, input.foliage);
    out.clip_position = view.clip_from_world * vec4<f32>(moved, 1.0);
    out.uv = input.uv;
    out.albedo_layer = input.maps.x;
    out.cutoff = input.foliage.x;
    return out;
}

@fragment
fn fragment_cutout(input: CutoutOutput) {
    if (input.cutoff > 0.0 && input.albedo_layer >= 0.0) {
        let alpha = textureSampleLevel(material_maps, material_sampler, input.uv, i32(input.albedo_layer), 0.0).a;
        if (alpha < input.cutoff) {
            discard;
        }
    }
}
