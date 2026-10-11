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

/// Depth format of the forward pass. Bevy projections are reverse-z, so depth clears to 0.
pub(crate) const DEPTH_FORMAT: TextureFormat = TextureFormat::Depth32Float;

/// The multisampled colour texture a view renders into before resolving to its output.
#[derive(Component, Clone)]
pub(crate) struct ViewMsaaColor(pub(crate) CachedTexture);

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
                usage: TextureUsages::RENDER_ATTACHMENT,
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
    views: Query<(Entity, &ExtractedCamera, &ViewTarget, &Msaa)>,
) {
    for (entity, camera, target, msaa) in &views {
        let (Some(size), Some(format)) = (target_extent(camera), target.out_texture_view_format())
        else {
            continue;
        };
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
