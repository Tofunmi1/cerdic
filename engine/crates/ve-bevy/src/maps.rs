//! The baked maps of textured surfaces (albedo, normal, metallic-roughness, emissive) as Bevy images.
//!
//! A surface names its maps by id (`VeMaterial::map_layers`); the pixels live in [`MaterialTextures`]. Each id used becomes one image the
//! first time a material asks for it, with its full mip chain, and stays until the maps change.

use std::collections::HashMap;

use bevy_asset::{Assets, Handle, RenderAssetUsages};
use bevy_ecs::prelude::*;
use bevy_image::{Image, ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy_render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use ve_render::{MaterialTextures, mip_chain, mip_levels};

/// The images made from the current maps, by map id and by whether the colours are sRGB (albedo and emissive) or plain data.
#[derive(Resource, Default)]
pub struct MapImages {
    generation: u64,
    images: HashMap<(i32, bool), Handle<Image>>,
    /// The live generation of the maps these images were last brought up to date with, and the version each image shows.
    live_seen: u64,
    shown: HashMap<(i32, bool), u64>,
}

impl MapImages {
    /// Brings the images of maps that were replaced in place (live textures) up to date: the image is given the new pixels and mip chain, so Bevy uploads it again.
    /// Cheap when nothing changed.
    pub fn refresh(&mut self, textures: &MaterialTextures, images: &mut Assets<Image>) {
        if self.live_seen == textures.live_generation() {
            return;
        }
        self.live_seen = textures.live_generation();
        for (&(id, srgb), handle) in &self.images {
            let version = textures.layer_version(id);
            if version == 0 || self.shown.get(&(id, srgb)) == Some(&version) {
                continue;
            }
            let Some((side, pixels)) = textures.layer(id) else {
                continue;
            };
            // A live image keeps its pixels in the main world too (it is changed every frame), unlike a still map.
            let image = map_image_with(side, pixels, srgb, RenderAssetUsages::default());
            if images.insert(handle.id(), image).is_ok() {
                self.shown.insert((id, srgb), version);
            }
        }
    }

    /// The image of map `id`, made on first use. `None` when the id is not a map (negative, or no longer in `textures`).
    pub fn get(
        &mut self,
        textures: &MaterialTextures,
        images: &mut Assets<Image>,
        id: i32,
        srgb: bool,
    ) -> Option<Handle<Image>> {
        if self.generation != textures.generation() {
            self.generation = textures.generation();
            self.images.clear();
            self.shown.clear();
        }
        if id < 0 {
            return None;
        }
        if let Some(handle) = self.images.get(&(id, srgb)) {
            return Some(handle.clone());
        }
        let (side, pixels) = textures.layer(id)?;
        let handle = images.add(map_image(side, pixels, srgb));
        self.images.insert((id, srgb), handle.clone());
        Some(handle)
    }
}

/// A repeating, trilinear 2D image of `side` square RGBA8 pixels with every mip level.
#[must_use]
pub fn map_image(side: u32, pixels: &[u8], srgb: bool) -> Image {
    map_image_with(side, pixels, srgb, RenderAssetUsages::RENDER_WORLD)
}

fn map_image_with(side: u32, pixels: &[u8], srgb: bool, usage: RenderAssetUsages) -> Image {
    let format = if srgb {
        TextureFormat::Rgba8UnormSrgb
    } else {
        TextureFormat::Rgba8Unorm
    };
    let extent = Extent3d {
        width: side,
        height: side,
        depth_or_array_layers: 1,
    };
    let mut image = Image::new(
        extent,
        TextureDimension::D2,
        pixels.to_vec(),
        format,
        usage,
    );
    image.data = Some(mip_chain(pixels, side));
    image.texture_descriptor.mip_level_count = mip_levels(side);
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        ..ImageSamplerDescriptor::default()
    });
    image
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_map_image_holds_every_mip_level_of_its_pixels() {
        let side = 4;
        let pixels: Vec<u8> = (0..side * side * 4)
            .map(|i| u8::try_from(i % 251).unwrap_or(0))
            .collect();
        let image = map_image(side, &pixels, true);
        assert_eq!(image.texture_descriptor.mip_level_count, 3);
        // 4x4 + 2x2 + 1x1 texels of 4 bytes.
        assert_eq!(image.data.as_ref().map(Vec::len), Some((16 + 4 + 1) * 4));
        assert_eq!(
            image.texture_descriptor.format,
            TextureFormat::Rgba8UnormSrgb
        );
    }

    #[test]
    fn a_negative_id_is_no_map() {
        let mut maps = MapImages::default();
        let mut images = Assets::<Image>::default();
        assert!(
            maps.get(&MaterialTextures::default(), &mut images, -1, false)
                .is_none()
        );
        assert!(
            maps.get(&MaterialTextures::default(), &mut images, 5, false)
                .is_none()
        );
    }
}
