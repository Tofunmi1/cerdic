// Multiplies the lit picture by the exposure scale the measuring pass just wrote (the pipeline's blend does the multiplication).

@group(0) @binding(0) var scale_state: texture_2d<f32>;

@vertex
fn vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let x = f32((index << 1u) & 2u);
    let y = f32(index & 2u);
    return vec4<f32>(x * 2.0 - 1.0, y * 2.0 - 1.0, 0.0, 1.0);
}

@fragment
fn fragment() -> @location(0) vec4<f32> {
    let scale = textureLoad(scale_state, vec2<i32>(0, 0), 0).x;
    return vec4<f32>(scale, scale, scale, 1.0);
}
