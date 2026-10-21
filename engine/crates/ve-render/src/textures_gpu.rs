//! The GPU side of the material texture arrays: one 2D-array texture per map size, their sampler, and the bind group the
//! forward, shadow and depth passes bind (see `textures.rs` for what goes in them).

use bevy_ecs::prelude::*;
use bevy_render::render_resource::binding_types::{sampler, texture_2d_array};
use bevy_render::render_resource::{
    AddressMode, BindGroup, BindGroupEntry, BindGroupLayoutDescriptor, BindGroupLayoutEntries,
    BindingResource, Extent3d, FilterMode, MipmapFilterMode, Origin3d, PipelineCache, Sampler,
    SamplerBindingType, SamplerDescriptor, ShaderStages, TexelCopyBufferLayout,
    TexelCopyTextureInfo, Texture, TextureAspect, TextureDescriptor, TextureDimension,
    TextureFormat, TextureSampleType, TextureUsages, TextureView, TextureViewDescriptor,
    TextureViewDimension,
};
use bevy_render::renderer::{RenderDevice, RenderQueue};

use crate::pipeline::ForwardPipelines;
use crate::textures::{ExtractedTextures, TIER_SIZES};

const FORMAT: TextureFormat = TextureFormat::Rgba8Unorm;

/// The array textures' views, their sampler and the bind group the passes bind.
#[derive(Resource, Default)]
pub(crate) struct TexturesGpu {
    sampler: Option<Sampler>,
    pub(crate) bind_group: Option<BindGroup>,
}

/// The bind group layout of the textures group: the three arrays (1024, 512 and 256 texels) and one sampler.
pub(crate) fn textures_layout() -> BindGroupLayoutDescriptor {
    BindGroupLayoutDescriptor::new(
        "ve_textures_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d_array(TextureSampleType::Float { filterable: true }),
                texture_2d_array(TextureSampleType::Float { filterable: true }),
                texture_2d_array(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
            ),
        ),
    )
}

/// Halves an RGBA8 image with a box filter.
pub(crate) fn downsample(pixels: &[u8], size: usize) -> Vec<u8> {
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

/// Levels in the mip chain of a `side` square map: `side`, `side / 2`, ... 1.
#[must_use]
pub fn mip_levels(side: u32) -> u32 {
    side.max(1).ilog2() + 1
}

/// Every level of the mip chain of a `side` square RGBA8 map, largest first, one after the other (the data layout of one texture layer
/// with its mips).
#[must_use]
pub fn mip_chain(pixels: &[u8], side: u32) -> Vec<u8> {
    let mut chain = pixels.to_vec();
    let mut level = pixels.to_vec();
    let mut edge = side as usize;
    while edge > 1 {
        level = downsample(&level, edge);
        edge /= 2;
        chain.extend_from_slice(&level);
    }
    chain
}

/// Uploads one layer of a `side` square array and every level of its mip chain.
fn write_layer(queue: &RenderQueue, texture: &Texture, layer: u32, side: u32, pixels: &[u8]) {
    let mut level_pixels = pixels.to_vec();
    let mut edge = side as usize;
    for level in 0..mip_levels(side) {
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

/// One tier's array view: its maps uploaded, or a 1 by 1, two-layer stand-in that no surface refers to when it has none.
fn tier_view(
    device: &RenderDevice,
    queue: &RenderQueue,
    side: u32,
    maps: &[Vec<u8>],
) -> TextureView {
    let (side, layers) = if maps.is_empty() {
        (1, 2)
    } else {
        (
            side,
            u32::try_from(allocated_layers(maps.len())).unwrap_or(2),
        )
    };
    let texture = device.create_texture(&TextureDescriptor {
        label: Some("ve_material_textures"),
        size: Extent3d {
            width: side,
            height: side,
            depth_or_array_layers: layers,
        },
        mip_level_count: mip_levels(side),
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: FORMAT,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (layer, pixels) in maps.iter().enumerate() {
        write_layer(
            queue,
            &texture,
            u32::try_from(layer).unwrap_or(0),
            side,
            pixels,
        );
    }
    // The view keeps its texture alive.
    texture.create_view(&TextureViewDescriptor {
        dimension: Some(TextureViewDimension::D2Array),
        ..Default::default()
    })
}

/// Builds (or rebuilds) the arrays when the maps changed, and always keeps a bind group ready.
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
    let views: Vec<TextureView> = TIER_SIZES
        .iter()
        .zip(&extracted.tiers)
        .map(|(&side, maps)| tier_view(&device, &queue, side, maps))
        .collect();
    let sampler = gpu.sampler.take().unwrap_or_else(|| {
        device.create_sampler(&SamplerDescriptor {
            label: Some("ve_material_sampler"),
            address_mode_u: AddressMode::Repeat,
            address_mode_v: AddressMode::Repeat,
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
                resource: BindingResource::TextureView(&views[0]),
            },
            BindGroupEntry {
                binding: 1,
                resource: BindingResource::TextureView(&views[1]),
            },
            BindGroupEntry {
                binding: 2,
                resource: BindingResource::TextureView(&views[2]),
            },
            BindGroupEntry {
                binding: 3,
                resource: BindingResource::Sampler(&sampler),
            },
        ],
    ));
    gpu.sampler = Some(sampler);
}
