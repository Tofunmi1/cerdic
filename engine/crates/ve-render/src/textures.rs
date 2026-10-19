//! The shared material texture array: baked maps (albedo, normal, metallic-roughness, emissive) of
//! textured surfaces, one `LAYER_SIZE` square layer each, with a mip chain.
//!
//! A surface refers to its maps by layer index (see `VeMaterial::map_layers`), so every draw shares one
//! bind group and batching still depends only on the mesh. The array is rebuilt when layers change.

use bevy_ecs::prelude::*;
use bevy_render::Extract;
use bevy_render::render_resource::binding_types::{sampler, texture_2d_array};
use bevy_render::render_resource::{
    BindGroup, BindGroupEntry, BindGroupLayoutDescriptor, BindGroupLayoutEntries, BindingResource,
    Extent3d, FilterMode, MipmapFilterMode, Origin3d, PipelineCache, Sampler, SamplerBindingType,
    SamplerDescriptor, ShaderStages, TexelCopyBufferLayout, TexelCopyTextureInfo, Texture,
    TextureAspect, TextureDescriptor, TextureDimension, TextureFormat, TextureSampleType,
    TextureUsages, TextureViewDescriptor, TextureViewDimension,
};
use bevy_render::renderer::{RenderDevice, RenderQueue};

use crate::pipeline::ForwardPipelines;

/// Side of every layer in texels.
pub const LAYER_SIZE: u32 = 1024;
/// Most layers the array holds (four per fully textured surface). 32 layers of 1024 x 1024 is 128 MB, about 170 MB with mips.
pub const MAX_LAYERS: usize = 32;
/// Levels in a layer's mip chain: 1024, 512, ... 1.
const MIP_LEVELS: u32 = 11;
const FORMAT: TextureFormat = TextureFormat::Rgba8Unorm;

/// The layers wanted by the scene, kept in the main world and copied to the render world when they change.
#[derive(Resource, Clone, Default, Debug)]
pub struct MaterialTextures {
    /// RGBA8 pixels of each layer, `LAYER_SIZE` square, row 0 first.
    layers: Vec<Vec<u8>>,
    generation: u64,
}

impl MaterialTextures {
    /// Removes every layer. Surfaces holding layer indices must be replaced too.
    pub fn clear(&mut self) {
        if !self.layers.is_empty() {
            self.layers.clear();
            self.generation += 1;
        }
    }

    /// Adds one map and returns its layer, or `None` when the array is full or the data is not
    /// `LAYER_SIZE * LAYER_SIZE` RGBA8 pixels.
    pub fn add(&mut self, rgba: Vec<u8>) -> Option<i32> {
        let wanted = (LAYER_SIZE * LAYER_SIZE * 4) as usize;
        if rgba.len() != wanted || self.layers.len() >= MAX_LAYERS {
            return None;
        }
        self.layers.push(rgba);
        self.generation += 1;
        i32::try_from(self.layers.len() - 1).ok()
    }

    /// How many layers are in use.
    #[must_use]
    pub fn len(&self) -> usize {
        self.layers.len()
    }

    /// Whether no layer is in use.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
    }
}

/// The render world's copy of [`MaterialTextures`].
#[derive(Resource, Default)]
pub(crate) struct ExtractedTextures {
    layers: Vec<Vec<u8>>,
    generation: u64,
    changed: bool,
}

/// Copies the layers across only when they changed.
pub(crate) fn extract_textures(
    mut out: ResMut<ExtractedTextures>,
    source: Extract<Res<MaterialTextures>>,
) {
    out.changed = false;
    if out.generation != source.generation || (out.layers.is_empty() && !source.layers.is_empty()) {
        out.layers.clone_from(&source.layers);
        out.generation = source.generation;
        out.changed = true;
    }
}

/// The array texture, its sampler and the bind group the forward pass binds as group 2.
#[derive(Resource, Default)]
pub(crate) struct TexturesGpu {
    sampler: Option<Sampler>,
    pub(crate) bind_group: Option<BindGroup>,
}

/// The bind group layout of group 2: the layer array and its sampler.
pub(crate) fn textures_layout() -> BindGroupLayoutDescriptor {
    BindGroupLayoutDescriptor::new(
        "ve_textures_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d_array(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
            ),
        ),
    )
}

/// Halves an RGBA8 image with a box filter.
fn downsample(pixels: &[u8], size: usize) -> Vec<u8> {
    let half = (size / 2).max(1);
    let mut out = vec![0_u8; half * half * 4];
    for y in 0..half {
        for x in 0..half {
            for channel in 0..4 {
                let at = |dx: usize, dy: usize| {
                    let sx = (x * 2 + dx).min(size - 1);
                    let sy = (y * 2 + dy).min(size - 1);
                    u32::from(pixels[(sy * size + sx) * 4 + channel])
                };
                let sum = at(0, 0) + at(1, 0) + at(0, 1) + at(1, 1);
                out[(y * half + x) * 4 + channel] = u8::try_from((sum + 2) / 4).unwrap_or(u8::MAX);
            }
        }
    }
    out
}

/// Uploads one layer and every level of its mip chain.
fn write_layer(queue: &RenderQueue, texture: &Texture, layer: u32, pixels: &[u8]) {
    let mut level_pixels = pixels.to_vec();
    let mut edge = LAYER_SIZE as usize;
    for level in 0..MIP_LEVELS {
        let texels = u32::try_from(edge).unwrap_or(1);
        queue.write_texture(
            TexelCopyTextureInfo {
                texture,
                mip_level: level,
                origin: Origin3d {
                    x: 0,
                    y: 0,
                    z: layer,
                },
                aspect: TextureAspect::All,
            },
            &level_pixels,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(texels * 4),
                rows_per_image: Some(texels),
            },
            Extent3d {
                width: texels,
                height: texels,
                depth_or_array_layers: 1,
            },
        );
        if edge > 1 {
            level_pixels = downsample(&level_pixels, edge);
            edge /= 2;
        }
    }
}

/// How many layers to allocate for `used` maps. wgpu's GL backend guesses a texture's view dimension from its layer
/// count: one layer is taken as plain 2D, and a multiple of six as a cube array, and either guess contradicts the
/// 2D-array view the shader binds ("bindTexture: invalid target"), which blanks every textured surface. So never fewer
/// than two layers, and one spare layer whenever the count would be a multiple of six.
pub(crate) fn allocated_layers(used: usize) -> usize {
    let layers = used.max(2);
    if layers.is_multiple_of(6) {
        layers + 1
    } else {
        layers
    }
}

/// Builds (or rebuilds) the array when the layers changed, and always keeps a bind group ready: with no
/// layers it is a one-layer stand-in that no surface refers to.
pub(crate) fn prepare_textures(
    extracted: Res<ExtractedTextures>,
    mut gpu: ResMut<TexturesGpu>,
    pipelines: Res<ForwardPipelines>,
    pipeline_cache: Res<PipelineCache>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    if gpu.bind_group.is_some() && !extracted.changed {
        return;
    }
    let layers = u32::try_from(allocated_layers(extracted.layers.len())).unwrap_or(2);
    let texture = device.create_texture(&TextureDescriptor {
        label: Some("ve_material_textures"),
        size: Extent3d {
            width: LAYER_SIZE,
            height: LAYER_SIZE,
            depth_or_array_layers: layers,
        },
        mip_level_count: MIP_LEVELS,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: FORMAT,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (layer, pixels) in extracted.layers.iter().enumerate() {
        write_layer(&queue, &texture, u32::try_from(layer).unwrap_or(0), pixels);
    }
    let view = texture.create_view(&TextureViewDescriptor {
        dimension: Some(TextureViewDimension::D2Array),
        ..Default::default()
    });
    let sampler = gpu.sampler.take().unwrap_or_else(|| {
        device.create_sampler(&SamplerDescriptor {
            label: Some("ve_material_sampler"),
            address_mode_u: bevy_render::render_resource::AddressMode::Repeat,
            address_mode_v: bevy_render::render_resource::AddressMode::Repeat,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: MipmapFilterMode::Linear,
            ..Default::default()
        })
    });
    let bind_layout = pipeline_cache.get_bind_group_layout(&pipelines.textures_layout);
    gpu.bind_group = Some(device.create_bind_group(
        "ve_textures_bind_group",
        &bind_layout,
        &[
            BindGroupEntry {
                binding: 0,
                resource: BindingResource::TextureView(&view),
            },
            BindGroupEntry {
                binding: 1,
                resource: BindingResource::Sampler(&sampler),
            },
        ],
    ));
    // The bind group keeps the view, and the view the texture, alive.
    gpu.sampler = Some(sampler);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layers_are_added_and_cleared_and_wrong_sizes_refused() {
        let mut textures = MaterialTextures::default();
        let good = vec![0_u8; (LAYER_SIZE * LAYER_SIZE * 4) as usize];
        assert_eq!(textures.add(good.clone()), Some(0));
        assert_eq!(textures.add(good), Some(1));
        assert_eq!(textures.add(vec![0; 16]), None);
        assert_eq!(textures.len(), 2);
        textures.clear();
        assert!(textures.is_empty());
    }

    #[test]
    fn the_array_has_room_for_the_most_layers() {
        let mut textures = MaterialTextures::default();
        for _ in 0..MAX_LAYERS {
            assert!(
                textures
                    .add(vec![0; (LAYER_SIZE * LAYER_SIZE * 4) as usize])
                    .is_some()
            );
        }
        assert_eq!(
            textures.add(vec![0; (LAYER_SIZE * LAYER_SIZE * 4) as usize]),
            None
        );
    }

    #[test]
    fn the_allocated_layer_count_is_never_one_or_a_multiple_of_six() {
        for used in 0..=MAX_LAYERS {
            let layers = allocated_layers(used);
            assert!(layers >= used.max(2), "{used} maps fit in {layers} layers");
            assert!(
                layers != 1 && layers % 6 != 0,
                "{used} maps would allocate {layers} layers, which wgpu's GL backend misreads"
            );
            assert!(layers <= used.max(2) + 1, "at most one spare layer");
        }
        assert_eq!(allocated_layers(6), 7);
        assert_eq!(allocated_layers(12), 13);
        assert_eq!(allocated_layers(5), 5);
    }

    #[test]
    fn downsampling_averages_two_by_two_blocks() {
        let pixels = [
            0, 0, 0, 255, 100, 100, 100, 255, //
            200, 200, 200, 255, 100, 100, 100, 255,
        ];
        let half = downsample(&pixels, 2);
        assert_eq!(half, vec![100, 100, 100, 255]);
    }
}
