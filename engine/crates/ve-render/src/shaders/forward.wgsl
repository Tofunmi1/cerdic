// Forward pass v0: per-instance model matrix as instance attributes (WebGL2 has no storage buffers),
// one directional light, flat albedo. Materials and real lighting arrive in M5.

struct View {
    clip_from_world: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> view: View;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(3) model_0: vec4<f32>,
    @location(4) model_1: vec4<f32>,
    @location(5) model_2: vec4<f32>,
    @location(6) model_3: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_normal: vec3<f32>,
};

@vertex
fn vertex(input: VertexInput) -> VertexOutput {
    let model = mat4x4<f32>(input.model_0, input.model_1, input.model_2, input.model_3);
    var out: VertexOutput;
    out.clip_position = view.clip_from_world * (model * vec4<f32>(input.position, 1.0));
    // Correct for rotation and uniform scale; non-uniform scale needs the inverse transpose (M5).
    out.world_normal = mat3x3<f32>(model[0].xyz, model[1].xyz, model[2].xyz) * input.normal;
    return out;
}

@fragment
fn fragment(input: VertexOutput) -> @location(0) vec4<f32> {
    let normal = normalize(input.world_normal);
    let light = normalize(vec3<f32>(0.4, 0.8, 0.45));
    let diffuse = max(dot(normal, light), 0.0);
    let albedo = vec3<f32>(0.60, 0.74, 0.92);
    return vec4<f32>(albedo * (0.25 + 0.75 * diffuse), 1.0);
}
