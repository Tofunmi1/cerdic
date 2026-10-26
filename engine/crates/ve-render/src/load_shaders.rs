//! Forward and auxiliary shader asset registration.
use super::*;
/// Loads the forward variants and the shared passes into the main world's shader assets.
pub(super) fn load_shaders(app: &mut App) -> Option<[bevy_asset::Handle<Shader>; 14]> {
    app.world_mut()
        .get_resource_mut::<Assets<Shader>>()
        .map(|mut shaders| {
            [
                shaders.add(Shader::from_wgsl(
                    pipeline::without_sim(pipeline::FORWARD_SHADER),
                    "ve_forward.wgsl",
                )),
                shaders.add(Shader::from_wgsl(
                    pipeline::without_sim(&pipeline::forward_shader_without_uv()),
                    "ve_forward_without_uv.wgsl",
                )),
                shaders.add(Shader::from_wgsl(
                    pipeline::without_sim(&pipeline::forward_shader_with_color(true)),
                    "ve_forward_color.wgsl",
                )),
                shaders.add(Shader::from_wgsl(
                    pipeline::without_sim(&pipeline::forward_shader_with_color(false)),
                    "ve_forward_color_without_uv.wgsl",
                )),
                shaders.add(Shader::from_wgsl(
                    present::PRESENT_SHADER,
                    present::PRESENT_SHADER_NAME,
                )),
                shaders.add(Shader::from_wgsl(
                    shadow::SHADOW_SHADER,
                    shadow::SHADOW_SHADER_NAME,
                )),
                shaders.add(Shader::from_wgsl(sky::SHADER, "ve_sky.wgsl")),
                shaders.add(Shader::from_wgsl(ao_depth::SHADER, ao_depth::SHADER_NAME)),
                shaders.add(Shader::from_wgsl(ao::SHADER, ao::SHADER_NAME)),
                shaders.add(Shader::from_wgsl(exposure::SHADER, exposure::SHADER_NAME)),
                shaders.add(Shader::from_wgsl(pipeline::FORWARD_SHADER, "ve_sim.wgsl")),
                shaders.add(Shader::from_wgsl(
                    pipeline::forward_shader_without_uv(),
                    "ve_sim_no_uv.wgsl",
                )),
                shaders.add(Shader::from_wgsl(
                    pipeline::forward_shader_with_color(true),
                    "ve_sim_color.wgsl",
                )),
                shaders.add(Shader::from_wgsl(
                    pipeline::forward_shader_with_color(false),
                    "ve_sim_color_no_uv.wgsl",
                )),
            ]
        })
}
