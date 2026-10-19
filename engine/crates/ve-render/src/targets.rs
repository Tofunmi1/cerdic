//! Per-view render targets: depth, and the multisampled colour target that resolves to the output.

use bevy_camera::{Camera3d, Camera3dDepthLoadOp};
use bevy_ecs::prelude::*;
use bevy_render::camera::ExtractedCamera;
use bevy_render::render_resource::{
    Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
};
use bevy_render::renderer::RenderDevice;
use bevy_render::texture::{CachedTexture, TextureCache};
use bevy_render::view::{Msaa, ViewDepthStencilTexture, ViewTarget};

use crate::lens::LensWarp;
use crate::quality::RenderQuality;

/// Depth format of the forward pass. Bevy projections are reverse-z, so depth clears to 0.
pub(crate) const DEPTH_FORMAT: TextureFormat = TextureFormat::Depth32Float;

/// Format of the offscreen texture that views with a non-sRGB output render into. It is an sRGB
/// format so the hardware encodes on write and decodes on sample, keeping the scene linear in shaders.
pub(crate) const OFFSCREEN_FORMAT: TextureFormat = TextureFormat::Rgba8UnormSrgb;

/// Whether a view with this output format must render offscreen and be copied to the output by the
/// present pass: the hardware only applies the sRGB transfer function to sRGB views.
pub(crate) fn needs_blit(output_format: TextureFormat) -> bool {
    !output_format.is_srgb()
}

/// Whether a view with this output format renders offscreen: its output needs the present pass, or
/// the lens warp is on and the present pass resamples every view.
pub(crate) fn blits(output_format: TextureFormat, lens: LensWarp, quality: RenderQuality) -> bool {
    needs_blit(output_format)
        || lens.active()
        || quality.ssao_enabled
        || quality.bloom_enabled
        || quality.auto_exposure_enabled
}

/// The format the forward pass renders in for a view with this output format.
pub(crate) fn render_format(
    output_format: TextureFormat,
    lens: LensWarp,
    quality: RenderQuality,
) -> TextureFormat {
    if blits(output_format, lens, quality) {
        OFFSCREEN_FORMAT
    } else {
        output_format
    }
}

/// The multisampled colour texture a view renders into before resolving to its output.
#[derive(Component, Clone)]
pub(crate) struct ViewMsaaColor(pub(crate) CachedTexture);

/// The single-sampled texture a blitted view resolves into; the present pass copies it to the output.
#[derive(Component, Clone)]
pub(crate) struct ViewOffscreen(pub(crate) CachedTexture);

fn target_extent(camera: &ExtractedCamera) -> Option<Extent3d> {
    camera.physical_target_size.map(|size| Extent3d {
        width: size.x,
        height: size.y,
        depth_or_array_layers: 1,
    })
}

/// Creates (or reuses, via the texture cache) a depth texture for every 3D camera.
pub(crate) fn prepare_depth_textures(
    mut commands: Commands,
    mut texture_cache: ResMut<TextureCache>,
    device: Res<RenderDevice>,
    views: Query<(Entity, &ExtractedCamera, &Camera3d, &Msaa)>,
) {
    for (entity, camera, camera_3d, msaa) in &views {
        let Some(size) = target_extent(camera) else {
            continue;
        };
        let texture = texture_cache.get(
            &device,
            TextureDescriptor {
                label: Some("ve_view_depth"),
                size,
                mip_level_count: 1,
                sample_count: msaa.samples(),
                dimension: TextureDimension::D2,
                format: DEPTH_FORMAT,
                // WebGL cannot make a multisampled texture that is also bound for sampling (it panics at startup), so
                // depth is only made readable when the view is not multisampled.
                usage: if msaa.samples() > 1 {
                    TextureUsages::RENDER_ATTACHMENT
                } else {
                    TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING
                },
                view_formats: &[],
            },
        );
        let clear = match camera_3d.depth_load_op {
            Camera3dDepthLoadOp::Clear(value) => Some(value),
            Camera3dDepthLoadOp::Load => None,
        };
        commands
            .entity(entity)
            .insert(ViewDepthStencilTexture::new(texture, clear, None));
    }
}

/// Creates the multisampled colour target for views that use MSAA, matching their output format.
pub(crate) fn prepare_msaa_targets(
    mut commands: Commands,
    mut texture_cache: ResMut<TextureCache>,
    device: Res<RenderDevice>,
    lens: Res<LensWarp>,
    views: Query<(Entity, &ExtractedCamera, &ViewTarget, &Msaa)>,
    quality: Res<RenderQuality>,
) {
    for (entity, camera, target, msaa) in &views {
        let (Some(size), Some(format)) = (target_extent(camera), target.out_texture_view_format())
        else {
            continue;
        };
        let format = render_format(format, *lens, *quality);
        if msaa.samples() <= 1 {
            commands.entity(entity).remove::<ViewMsaaColor>();
            continue;
        }
        let texture = texture_cache.get(
            &device,
            TextureDescriptor {
                label: Some("ve_view_msaa_color"),
                size,
                mip_level_count: 1,
                sample_count: msaa.samples(),
                dimension: TextureDimension::D2,
                format,
                usage: TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            },
        );
        commands.entity(entity).insert(ViewMsaaColor(texture));
    }
}

/// Creates the offscreen resolve texture for views whose output needs the present pass, and removes
/// it from views that render straight to their output.
pub(crate) fn prepare_offscreen_targets(
    mut commands: Commands,
    mut texture_cache: ResMut<TextureCache>,
    device: Res<RenderDevice>,
    lens: Res<LensWarp>,
    views: Query<(Entity, &ExtractedCamera, &ViewTarget)>,
    quality: Res<RenderQuality>,
) {
    for (entity, camera, target) in &views {
        let (Some(size), Some(format)) = (target_extent(camera), target.out_texture_view_format())
        else {
            continue;
        };
        if !blits(format, *lens, *quality) {
            commands.entity(entity).remove::<ViewOffscreen>();
            continue;
        }
        let texture = texture_cache.get(
            &device,
            TextureDescriptor {
                label: Some("ve_view_offscreen"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: OFFSCREEN_FORMAT,
                usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
        );
        commands.entity(entity).insert(ViewOffscreen(texture));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_post_effects_keep_the_direct_srgb_path() {
        let format = TextureFormat::Rgba8UnormSrgb;
        let quality = RenderQuality::default();
        assert!(!blits(format, LensWarp::default(), quality));
        assert_eq!(render_format(format, LensWarp::default(), quality), format);
    }

    #[test]
    fn enabled_post_effects_request_a_sampleable_offscreen_target() {
        let mut quality = RenderQuality::default();
        quality.ssao_enabled = true;
        assert!(blits(
            TextureFormat::Rgba8UnormSrgb,
            LensWarp::default(),
            quality
        ));
        assert_eq!(
            render_format(TextureFormat::Rgba8UnormSrgb, LensWarp::default(), quality),
            OFFSCREEN_FORMAT
        );
    }
}
