// Depth pass for screen-space effects: the opaque geometry's depth in a float colour texture. The GL backend (WebGL2) cannot
// read a depth texture except by comparison, so the post pass reads this instead with `textureLoad`.
//
// The value is the fragment's device depth: reverse-Z, so 1 is the near plane and 0 the far plane, which is also what the
// texture is cleared to (the sky). The post shader rebuilds world positions from it with the view's `world_from_clip`.

struct View {
    clip_from_world: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> view: View;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(3) model_0: vec4<f32>,
    @location(4) model_1: vec4<f32>,
    @location(5) model_2: vec4<f32>,
    @location(6) model_3: vec4<f32>,
};

@vertex
fn vertex(input: VertexInput) -> @builtin(position) vec4<f32> {
    let model = mat4x4<f32>(input.model_0, input.model_1, input.model_2, input.model_3);
    return view.clip_from_world * (model * vec4<f32>(input.position, 1.0));
}

@fragment
fn fragment(@builtin(position) position: vec4<f32>) -> @location(0) f32 {
    return position.z;
}

// Cut-out surfaces (leaves, fences): the same depth, but a pixel whose albedo alpha is under the instance's cutoff leaves the
// depth as it was, so the occlusion pass sees the leaf shape and not its whole card (see the forward pass and shadow.wgsl).
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
    out.clip_position = view.clip_from_world * (model * vec4<f32>(input.position, 1.0));
    out.uv = input.uv;
    out.albedo_layer = input.maps.x;
    out.cutoff = input.foliage.x;
    return out;
}

@fragment
fn fragment_cutout(input: CutoutOutput) -> @location(0) f32 {
    if (input.cutoff > 0.0 && input.albedo_layer >= 0.0) {
        let alpha = textureSampleLevel(material_maps, material_sampler, input.uv, i32(input.albedo_layer), 0.0).a;
        if (alpha < input.cutoff) {
            discard;
        }
    }
    return input.clip_position.z;
}
