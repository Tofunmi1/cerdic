//! Shader specialization for interactive water.
/// Removes simulation resources entirely from ordinary water and non-water pipelines.
pub(crate) fn without_sim(source: &str) -> String {
    let start = source.find("@group(3) @binding(1) var water_sim");
    let end = source.find("fn water_wave_phase");
    match (start, end) {
        (Some(start), Some(end)) => format!(
            "{}fn water_sim_height(p: vec2<f32>) -> f32 {{ return 0.0; }}\nfn water_sim_gradient(p: vec2<f32>) -> vec2<f32> {{ return vec2<f32>(0.0); }}\n{}",
            &source[..start],
            &source[end..]
        ),
        _ => source.to_owned(),
    }
}

/// Source of the forward shader, compiled at runtime by the shader pipeline.
pub(crate) const FORWARD_SHADER: &str = concat!(
    include_str!("shaders/wind.wgsl"),
    include_str!("shaders/grass.wgsl"),
    include_str!("shaders/surface_types.wgsl"),
    include_str!("shaders/stochastic.wgsl"),
    include_str!("shaders/forward_terrain.wgsl"),
    include_str!("shaders/water_waves.wgsl"),
    include_str!("shaders/forward_lighting.wgsl"),
    include_str!("shaders/forward_surface.wgsl"),
    include_str!("shaders/forward_maps.wgsl"),
    include_str!("shaders/forward.wgsl"),
    include_str!("shaders/forward_fragment.wgsl")
);
