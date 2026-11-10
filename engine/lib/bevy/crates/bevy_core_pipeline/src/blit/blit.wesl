import package::fullscreen_vertex_shader::FullscreenVertexOutput;
@if(SRGB_TO_LINEAR) import bevy_render::color_operations::srgb_to_linear;
@if(OKLAB_TO_LINEAR) import bevy_render::color_operations::oklab_to_linear_rgb;
// VE patch 3: an 8-bit non-sRGB target (the WebXR layer framebuffer) gets no hardware encoding, so encode here.
@if(LINEAR_TO_SRGB_OUTPUT) import bevy_render::color_operations::linear_to_srgb;

@group(0) @binding(0) var in_texture: texture_2d<f32>;
@group(0) @binding(1) var in_sampler: sampler;

@fragment
fn fs_main(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    var color = textureSample(in_texture, in_sampler, in.uv);
    @if(SRGB_TO_LINEAR)
    color = vec4(srgb_to_linear(color.rgb), color.a);
    @if(OKLAB_TO_LINEAR)
    color = vec4(oklab_to_linear_rgb(color.rgb), color.a);
    @if(LINEAR_TO_SRGB_OUTPUT)
    color = vec4(linear_to_srgb(color.rgb), color.a);
    return color;
}
