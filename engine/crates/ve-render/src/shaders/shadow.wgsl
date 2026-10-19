// Shadow pass: depth only, from the sun's orthographic camera. It reads the same per-instance model
// matrix as the forward pass (locations 3 to 6) and has no fragment stage.

struct View {
    clip_from_world: mat4x4<f32>,
    camera_position: vec4<f32>,
    exposure: vec4<f32>,
    // Present for layout compatibility with the forward pass; the depth pass does not grade.
    grading: vec4<f32>,
    tint: vec4<f32>,
    world_from_clip: mat4x4<f32>,
    viewport: vec4<f32>,
    horizon: vec4<f32>,
    zenith: vec4<f32>,
    ground: vec4<f32>,
    sun: vec4<f32>,
    cloud: vec4<f32>,
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
