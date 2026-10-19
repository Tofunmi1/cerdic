//! Light probes on the GPU: global illumination for diffuse surfaces.
//!
//! A [`ProbeVolume`] is a 3D grid of probes, each holding first-order spherical-harmonic irradiance over pi (see the `ve-gi`
//! crate that bakes them). It is uploaded as four 3D `Rgba16Float` textures, one per coefficient (the constant term and the x,
//! y and z terms, each as rgb), and the forward shader looks a surface up with one hardware trilinear sample per texture, so
//! walls pick up bounce light and sheltered places go dark. Without a volume a zero 1 by 1 by 1 stand-in is bound and the
//! shader keeps its constant sky ambient.

use bevy_ecs::prelude::*;
use bevy_math::Vec4;
use bevy_render::RenderApp;
use bevy_render::extract_resource::ExtractResource;
use bevy_render::render_resource::{
    AddressMode, Extent3d, FilterMode, Origin3d, Sampler, SamplerDescriptor, TexelCopyBufferLayout,
    TexelCopyTextureInfo, Texture, TextureAspect, TextureDescriptor, TextureDimension,
    TextureFormat, TextureUsages, TextureView, TextureViewDescriptor,
};
use bevy_render::renderer::{RenderDevice, RenderQueue};

/// Format of the probe textures: four half floats, filterable on every WebGL2 device.
pub(crate) const FORMAT: TextureFormat = TextureFormat::Rgba16Float;

/// Largest grid the renderer accepts, in probes along any axis and in all.
pub const MAX_PROBES_PER_AXIS: u32 = 64;
/// See [`MAX_PROBES_PER_AXIS`].
pub const MAX_PROBES: usize = 65_536;

/// A grid of light probes: probe `(i, j, k)` is at `origin + spacing * (i, j, k)` and is number `i + dims[0] * (j + dims[1] * k)`.
#[derive(Resource, ExtractResource, Clone, Debug, Default, PartialEq)]
#[extract_app(RenderApp)]
pub struct ProbeVolume {
    /// World position of probe zero.
    pub origin: [f32; 3],
    /// Distance between neighbouring probes in metres.
    pub spacing: f32,
    /// Probes along x, y and z.
    pub dims: [u32; 3],
    /// Twelve numbers per probe: the constant term's rgb, then the x, y and z terms' rgb. A surface facing the unit normal
    /// `n` receives `max(dc + x * n.x + y * n.y + z * n.z, 0)`, irradiance over pi.
    pub coefficients: Vec<[f32; 12]>,
}

impl ProbeVolume {
    /// Whether the volume is a usable grid: positive spacing, sizes within the limits, one entry per probe, all finite.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        let count = self.dims.iter().map(|&d| d as usize).product::<usize>();
        self.spacing.is_finite()
            && self.spacing > 0.0
            && self.origin.iter().all(|v| v.is_finite())
            && self
                .dims
                .iter()
                .all(|&d| (1..=MAX_PROBES_PER_AXIS).contains(&d))
            && count <= MAX_PROBES
            && self.coefficients.len() == count
            && self.coefficients.iter().flatten().all(|v| v.is_finite())
    }
}

/// The nearest half float (round to nearest even) to `value`; overflow becomes infinity, tiny values denormals or zero.
#[must_use]
pub(crate) fn f32_to_f16(value: f32) -> u16 {
    let bits = value.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exponent = ((bits >> 23) & 0xff) as i32;
    let mantissa = bits & 0x007f_ffff;
    if exponent == 0xff {
        // Infinity, or a NaN (kept a NaN by setting a mantissa bit).
        return sign | 0x7c00 | if mantissa != 0 { 0x0200 } else { 0 };
    }
    let unbiased = exponent - 127;
    if unbiased > 15 {
        return sign | 0x7c00;
    }
    if unbiased >= -14 {
        // A normal half: 10 mantissa bits, round to nearest even on the 13 dropped ones.
        let half_exponent = ((unbiased + 15) as u32) << 10;
        let mut half = half_exponent | (mantissa >> 13);
        let dropped = mantissa & 0x1fff;
        if dropped > 0x1000 || (dropped == 0x1000 && half & 1 == 1) {
            half += 1;
        }
        // A carry out of the mantissa bumps the exponent, which is right (and reaches infinity at the top).
        return sign | half as u16;
    }
    if unbiased >= -25 {
        // A denormal half: the implicit leading bit becomes explicit and the value is shifted right.
        let full = mantissa | 0x0080_0000;
        let shift = (-unbiased - 1) as u32;
        let mut half = full >> shift;
        let dropped = full & ((1 << shift) - 1);
        let halfway = 1 << (shift - 1);
        if dropped > halfway || (dropped == halfway && half & 1 == 1) {
            half += 1;
        }
        return sign | half as u16;
    }
    sign
}

/// The four textures' bytes for a volume: for coefficient `k` (0 constant, 1 x, 2 y, 3 z), each probe's rgb as half floats and a
/// one in alpha, in probe order.
#[must_use]
pub(crate) fn texture_bytes(volume: &ProbeVolume) -> [Vec<u8>; 4] {
    std::array::from_fn(|k| {
        let mut bytes = Vec::with_capacity(volume.coefficients.len() * 8);
        for probe in &volume.coefficients {
            for channel in 0..3 {
                bytes.extend_from_slice(&f32_to_f16(probe[3 * k + channel]).to_le_bytes());
            }
            bytes.extend_from_slice(&f32_to_f16(1.0).to_le_bytes());
        }
        bytes
    })
}

/// The probe textures in use, their sampler, and what the shader needs to find a position in them.
#[derive(Resource, Default)]
pub(crate) struct ProbeGpu {
    // Held so the textures outlive their views.
    _textures: Vec<Texture>,
    views: Vec<TextureView>,
    sampler: Option<Sampler>,
    /// xyz: world position of probe zero, w: spacing.
    pub(crate) origin_spacing: Vec4,
    /// xyz: probes along each axis, w: 1 when a volume is bound and 0 for the stand-in.
    pub(crate) dims_enabled: Vec4,
}

impl ProbeGpu {
    /// The four coefficient views, once created.
    pub(crate) fn views(&self) -> Option<&[TextureView]> {
        (self.views.len() == 4).then_some(self.views.as_slice())
    }

    /// The linear clamping sampler, once created.
    pub(crate) fn sampler(&self) -> Option<&Sampler> {
        self.sampler.as_ref()
    }
}

/// Creates the sampler, and (re)builds the textures whenever the volume changes: the volume itself when it is valid, a zero
/// 1 by 1 by 1 stand-in otherwise.
#[expect(
    clippy::cast_precision_loss,
    reason = "probe counts along an axis are at most 64"
)]
pub(crate) fn prepare_probes(
    volume: Res<ProbeVolume>,
    mut gpu: ResMut<ProbeGpu>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    if gpu.sampler.is_none() {
        gpu.sampler = Some(device.create_sampler(&SamplerDescriptor {
            label: Some("ve_probe_sampler"),
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            ..Default::default()
        }));
    }
    if !gpu.views.is_empty() && !volume.is_changed() {
        return;
    }
    let active = volume.is_valid();
    let (dims, bytes) = if active {
        (volume.dims, texture_bytes(&volume))
    } else {
        ([1, 1, 1], std::array::from_fn(|_| vec![0; 8]))
    };
    let size = Extent3d {
        width: dims[0],
        height: dims[1],
        depth_or_array_layers: dims[2],
    };
    let mut textures = Vec::with_capacity(4);
    let mut views = Vec::with_capacity(4);
    for data in &bytes {
        let texture = device.create_texture(&TextureDescriptor {
            label: Some("ve_probes"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D3,
            format: FORMAT,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            data,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(dims[0] * 8),
                rows_per_image: Some(dims[1]),
            },
            size,
        );
        views.push(texture.create_view(&TextureViewDescriptor::default()));
        textures.push(texture);
    }
    gpu._textures = textures;
    gpu.views = views;
    if active {
        let origin = volume.origin;
        gpu.origin_spacing = Vec4::new(origin[0], origin[1], origin[2], volume.spacing);
        gpu.dims_enabled = Vec4::new(dims[0] as f32, dims[1] as f32, dims[2] as f32, 1.0);
    } else {
        gpu.origin_spacing = Vec4::new(0.0, 0.0, 0.0, 1.0);
        gpu.dims_enabled = Vec4::new(1.0, 1.0, 1.0, 0.0);
    }
}

#[cfg(test)]
#[path = "probes_tests.rs"]
mod tests;
