//! Versioned simulation images shared by all chunks of a water part.
use bevy_asset::{Assets, Handle, RenderAssetUsages};
use bevy_ecs::prelude::*;
use bevy_image::Image;
use bevy_render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use std::collections::BTreeMap;
use ve_render::water_sim::{WaterSimTextures, pack_heights};

#[derive(Resource, Default)]
pub(crate) struct WaterImages(BTreeMap<u64, (u64, [u32; 2], Handle<Image>)>);

impl WaterImages {
    pub(crate) fn get(&self, id: u64) -> Option<Handle<Image>> {
        self.0.get(&id).map(|(_, _, handle)| handle.clone())
    }

    pub(crate) fn update(&mut self, frames: &WaterSimTextures, images: &mut Assets<Image>) {
        self.0.retain(|id, _| frames.0.contains_key(id));
        for (id, frame) in &frames.0 {
            if let Some((version, size, handle)) = self.0.get_mut(id)
                && *size == frame.size
                && images.contains(handle.id())
            {
                if *version != frame.version {
                    if let Some(mut image) = images.get_mut(handle.id()) {
                        image.data = Some(pack_heights(&frame.heights));
                    }
                    *version = frame.version;
                }
                continue;
            }
            let image = Image::new(
                Extent3d {
                    width: frame.size[0],
                    height: frame.size[1],
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                pack_heights(&frame.heights),
                TextureFormat::R32Float,
                RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
            );
            self.0
                .insert(*id, (frame.version, frame.size, images.add(image)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ve_render::water_sim::WaterSimFrame;

    #[test]
    fn versions_control_uploads_and_chunks_share_a_stable_float_image() -> Result<(), &'static str>
    {
        let mut cache = WaterImages::default();
        let mut images = Assets::default();
        let mut frames = WaterSimTextures::default();
        frames.0.insert(
            7,
            WaterSimFrame {
                size: [2, 1],
                version: 0,
                heights: [0.0, 0.1].into(),
            },
        );
        cache.update(&frames, &mut images);
        let handle = cache.get(7).ok_or("image was not built")?;
        assert_eq!(
            images.get(&handle).map(|i| i.texture_descriptor.format),
            Some(TextureFormat::R32Float)
        );
        frames.0.insert(
            7,
            WaterSimFrame {
                size: [2, 1],
                version: 0,
                heights: [0.2, -0.3].into(),
            },
        );
        cache.update(&frames, &mut images);
        assert_eq!(
            images.get(&handle).and_then(|i| i.data.as_deref()),
            Some(pack_heights(&[0.0, 0.1]).as_slice())
        );
        if let Some(frame) = frames.0.get_mut(&7) {
            frame.version += 1;
        }
        cache.update(&frames, &mut images);
        assert_eq!(cache.get(7), Some(handle.clone()));
        assert_eq!(
            images.get(&handle).and_then(|i| i.data.as_deref()),
            Some(pack_heights(&[0.2, -0.3]).as_slice())
        );
        frames.0.insert(
            7,
            WaterSimFrame {
                size: [1, 1],
                version: 1,
                heights: [0.4].into(),
            },
        );
        cache.update(&frames, &mut images);
        assert_ne!(cache.get(7), Some(handle));
        frames.0.clear();
        cache.update(&frames, &mut images);
        assert!(cache.get(7).is_none());
        Ok(())
    }
}
