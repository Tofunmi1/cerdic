use super::*;

#[test]
fn the_shader_without_texture_coordinates_drops_exactly_the_uv_input() {
    let plain = forward_shader_without_uv();
    assert!(FORWARD_SHADER.contains("@location(11) uv: vec2<f32>,"));
    assert!(
        !plain.contains("@location(11) uv"),
        "the vertex input is gone"
    );
    assert!(
        !plain.contains("out.uv = input.uv"),
        "the vertex stage no longer copies it"
    );
    assert!(plain.contains("out.uv = vec2<f32>(0.0);"));
    // Everything else is the same source.
    assert_eq!(
        plain.lines().count() + 1,
        FORWARD_SHADER.lines().count(),
        "one line fewer"
    );
}

#[test]
fn only_the_foliage_draw_skips_culling() {
    let pipelines = ForwardPipelines::new(
        Handle::default(),
        Handle::default(),
        Handle::default(),
        Handle::default(),
    );
    let cull = |kind| {
        pipelines
            .descriptor(
                VertexBufferLayout::default(),
                TextureFormat::Rgba8UnormSrgb,
                1,
                false,
                kind,
                false,
                false,
            )
            .primitive
            .cull_mode
    };
    assert_eq!(cull(PassKind::Opaque), Some(Face::Back));
    assert_eq!(cull(PassKind::Transparent), Some(Face::Back));
    assert_eq!(cull(PassKind::Outline), Some(Face::Front));
    assert_eq!(cull(PassKind::TwoSided), None);
}
