//! CPU simulation snapshots and WebGL2-safe R32Float texture uploads.
use bevy_ecs::prelude::*;
use bevy_render::extract_resource::ExtractResource;
use bevy_render::render_resource::*;
use bevy_render::renderer::{RenderDevice, RenderQueue};
use std::collections::BTreeMap;
use std::sync::Arc;

/// One part's immutable snapshot, shared across extraction without copying cells.
#[derive(Clone, Debug)]
pub struct WaterSimFrame {
    /// Texture width and height.
    pub size: [u32; 2],
    /// Monotonic upload version.
    pub version: u64,
    /// Cell-centred displacements in row-major order.
    pub heights: Arc<[f32]>,
}
/// Live simulation snapshots, keyed by a per-part runtime identity.
#[derive(Resource, Default, Clone, ExtractResource)]
#[extract_app(bevy_render::RenderApp)]
pub struct WaterSimTextures(pub BTreeMap<u64, WaterSimFrame>);

/// Packs single-channel floats without reducing precision.
pub fn pack_heights(heights: &[f32]) -> Vec<u8> {
    heights.iter().flat_map(|h| h.to_le_bytes()).collect()
}
pub(crate) struct SimTexture {
    pub(crate) texture: Texture,
    pub(crate) view: TextureView,
    version: u64,
    size: [u32; 2],
}
#[derive(Resource, Default)]
pub(crate) struct SimGpu(pub(crate) BTreeMap<u64, SimTexture>);

pub(crate) fn prepare(
    mut gpu: ResMut<SimGpu>,
    frames: Res<WaterSimTextures>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    gpu.0.retain(|id, _| *id == 0 || frames.0.contains_key(id));
    let fallback = WaterSimFrame {
        size: [1, 1],
        version: 0,
        heights: Arc::from([0.0]),
    };
    for (id, frame) in std::iter::once((&0, &fallback)).chain(frames.0.iter()) {
        if frame.size.contains(&0)
            || frame.heights.len() != frame.size[0] as usize * frame.size[1] as usize
        {
            continue;
        }
        let size = Extent3d {
            width: frame.size[0],
            height: frame.size[1],
            depth_or_array_layers: 1,
        };
        if gpu.0.get(id).is_none_or(|t| t.size != frame.size) {
            let texture = device.create_texture(&TextureDescriptor {
                label: Some("ve_water_sim"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::R32Float,
                usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = texture.create_view(&TextureViewDescriptor::default());
            gpu.0.insert(
                *id,
                SimTexture {
                    texture,
                    view,
                    version: u64::MAX,
                    size: frame.size,
                },
            );
        }
        if let Some(texture) = gpu.0.get_mut(id).filter(|t| t.version != frame.version) {
            queue.write_texture(
                TexelCopyTextureInfo {
                    texture: &texture.texture,
                    mip_level: 0,
                    origin: Origin3d::ZERO,
                    aspect: TextureAspect::All,
                },
                &pack_heights(&frame.heights),
                TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(frame.size[0] * 4),
                    rows_per_image: Some(frame.size[1]),
                },
                size,
            );
            texture.version = frame.version;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn simulation_snapshots_exist_with_the_shared_render_data_plugin() {
        let mut app = bevy_app::App::new();
        app.add_plugins(crate::VeRenderDataPlugin);
        assert!(app.world().contains_resource::<WaterSimTextures>());
    }

    #[test]
    fn float_texture_round_trip() {
        let heights = [-0.04_f32, 0.0, 0.00001, 1.2345];
        let bytes = pack_heights(&heights);
        let decoded: Vec<f32> = bytes
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect();
        for (a, b) in heights.iter().zip(decoded) {
            assert!((a - b).abs() < 1e-7);
        }
    }
}
