//! The baked light probes as a Bevy irradiance volume. The runtime bakes first-order spherical harmonics on a grid (`ve-gi`) and keeps them
//! in `ProbeVolume`; `ve_gi::volume_texels` turns them into the ambient cubes Bevy's volume reads, and the volume's cube is placed so each
//! voxel centre sits on its probe.

use bevy_app::{App, Plugin, Update};
use bevy_asset::{Assets, RenderAssetUsages};
use bevy_ecs::prelude::*;
use bevy_image::{Image, ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy_light::{IrradianceVolume, LightProbe};
use bevy_math::Vec3;
use bevy_render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy_transform::components::Transform;
use half::f16;
use ve_gi::{GridSpec, ProbeGrid, ProbeSh, volume_placement, volume_texels};
use ve_render::ProbeVolume;

use crate::units;

/// Registers the probe volume system.
#[derive(Debug, Default, Clone, Copy)]
pub struct VeBevyProbesPlugin;

impl Plugin for VeBevyProbesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, map_probes);
    }
}

/// The baked grid of a [`ProbeVolume`], or `None` when it is not a usable grid.
#[must_use]
pub fn grid_of(volume: &ProbeVolume) -> Option<ProbeGrid> {
    if !volume.is_valid() {
        return None;
    }
    let spec = GridSpec {
        origin: volume.origin,
        spacing: volume.spacing,
        dims: volume.dims,
    };
    let probes: Vec<ProbeSh> = volume
        .coefficients
        .iter()
        .map(|c| ProbeSh {
            dc: [c[0], c[1], c[2]],
            x: [c[3], c[4], c[5]],
            y: [c[6], c[7], c[8]],
            z: [c[9], c[10], c[11]],
        })
        .collect();
    let valid = vec![true; probes.len()];
    Some(ProbeGrid::from_parts(spec, probes, valid, [0.0; 3]))
}

/// The 3D image of a grid's ambient cubes (`RGBA16F`, trilinear, clamped at the edges).
#[must_use]
pub fn volume_image(grid: &ProbeGrid) -> Image {
    let volume = volume_texels(grid);
    let data: Vec<u8> = volume
        .texels
        .iter()
        .flat_map(|texel| texel.iter().flat_map(|c| f16::from_f32(*c).to_le_bytes()))
        .collect();
    let [width, height, depth] = volume.size;
    let mut image = Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: depth,
        },
        TextureDimension::D3,
        data,
        TextureFormat::Rgba16Float,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        address_mode_w: ImageAddressMode::ClampToEdge,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Nearest,
        ..ImageSamplerDescriptor::default()
    });
    image
}

/// Replaces the irradiance volume whenever the baked probes change.
fn map_probes(
    mut commands: Commands,
    volume: Res<ProbeVolume>,
    mut images: ResMut<Assets<Image>>,
    mut current: Local<Option<Entity>>,
) {
    if !volume.is_changed() {
        return;
    }
    if let Some(entity) = current.take() {
        commands.entity(entity).try_despawn();
    }
    let Some(grid) = grid_of(&volume) else {
        return;
    };
    let (translation, scale) = volume_placement(&grid.spec);
    let voxels = images.add(volume_image(&grid));
    *current = Some(
        commands
            .spawn((
                LightProbe::default(),
                IrradianceVolume {
                    voxels,
                    intensity: units::scene_unit_cd(),
                    affects_lightmapped_meshes: true,
                },
                Transform {
                    translation: Vec3::from_array(translation),
                    scale: Vec3::from_array(scale),
                    ..Transform::IDENTITY
                },
            ))
            .id(),
    );
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "a test fixture that is not a valid grid should fail the test loudly"
)]
mod tests {
    use super::*;

    fn volume(dims: [u32; 3]) -> ProbeVolume {
        let count = (dims[0] * dims[1] * dims[2]) as usize;
        ProbeVolume {
            origin: [1.0, 0.0, -1.0],
            spacing: 0.5,
            dims,
            coefficients: vec![
                [
                    0.5, 0.25, 0.125, 0.1, 0.0, 0.0, 0.0, 0.1, 0.0, 0.0, 0.0, 0.1
                ];
                count
            ],
        }
    }

    #[test]
    fn a_valid_volume_becomes_a_grid_with_its_coefficients_in_place() {
        let grid = grid_of(&volume([2, 2, 2])).expect("valid volume");
        assert_eq!(grid.spec.dims, [2, 2, 2]);
        assert_eq!(grid.probes.len(), 8);
        assert_eq!(grid.probes[0].dc, [0.5, 0.25, 0.125]);
        assert_eq!(grid.probes[0].y, [0.0, 0.1, 0.0]);
    }

    #[test]
    fn an_empty_or_inconsistent_volume_is_no_grid() {
        assert!(grid_of(&ProbeVolume::default()).is_none());
        let mut broken = volume([2, 2, 2]);
        broken.coefficients.pop();
        assert!(grid_of(&broken).is_none());
    }

    #[test]
    fn the_image_is_a_3d_texture_of_rx_by_2ry_by_3rz() {
        let grid = grid_of(&volume([2, 3, 4])).expect("valid volume");
        let image = volume_image(&grid);
        assert_eq!(image.texture_descriptor.dimension, TextureDimension::D3);
        let size = image.texture_descriptor.size;
        assert_eq!(
            (size.width, size.height, size.depth_or_array_layers),
            (2, 6, 12)
        );
        assert_eq!(
            image.data.as_ref().map(Vec::len),
            Some(2 * 6 * 12 * 8),
            "four half floats per texel"
        );
    }
}
