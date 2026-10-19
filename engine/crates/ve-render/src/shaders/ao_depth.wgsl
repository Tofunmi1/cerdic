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
