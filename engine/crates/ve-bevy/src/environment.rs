//! The sky where the atmosphere passes cannot run (WebGL2): a skybox and an environment light from procedural cubemaps built from the
//! scene's sky colours (`ve-sky`). The environment light has no sun in it, because the directional light already lights the scene.

use bevy_app::{App, Plugin, Update};
use bevy_asset::{Assets, RenderAssetUsages};
use bevy_camera::Camera3d;
use bevy_color::{Color, ColorToComponents};
use bevy_ecs::prelude::*;
use bevy_image::{Image, ImageSampler};
use bevy_light::{EnvironmentMapLight, Skybox};
use bevy_math::Quat;
use bevy_render::render_resource::{
    Extent3d, TextureDimension, TextureFormat, TextureViewDescriptor, TextureViewDimension,
};
use half::f16;
use ve_render::SkyBackground;
use ve_sky::{CubeFaces, cloud_amount, irradiance_cubemap, prefiltered_mips, texel_direction};

use crate::units;

/// Face size of the sky the viewer sees, and of the sources the lighting maps are made from.
const SKYBOX_SIZE: u32 = 128;
const SOURCE_SIZE: u32 = 32;
const IRRADIANCE_SIZE: u32 = 16;
const SPECULAR_MIPS: u32 = 6;

/// Registers the cubemap sky systems.
#[derive(Debug, Default, Clone, Copy)]
pub struct VeBevyEnvironmentPlugin;

impl Plugin for VeBevyEnvironmentPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, map_environment.after(crate::camera::map_cameras));
    }
}

fn smoothstep(low: f32, high: f32, x: f32) -> f32 {
    let t = ((x - low) / (high - low)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The sky's linear radiance toward `direction`, the way the engine's own sky shader (`ve-render/src/shaders/sky.wgsl`) paints it: the
/// three colours mixed in sRGB along smooth steps, then the sun's glow and disc when `with_sun`, then the clouds when the sky has them.
#[must_use]
pub fn sky_radiance(sky: &SkyBackground, direction: [f32; 3], with_sun: bool) -> [f32; 3] {
    let up = direction[1].clamp(-1.0, 1.0);
    let mix = |a: [f32; 3], b: [f32; 3], t: f32| -> [f32; 3] {
        std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t)
    };
    let mut color = if up >= 0.0 {
        mix(sky.horizon, sky.zenith, smoothstep(0.0, 0.85, up))
    } else {
        mix(sky.horizon, sky.ground, 1.0 - smoothstep(-0.35, 0.0, up))
    };
    if with_sun {
        let to_sun = [
            -sky.sun_direction[0],
            -sky.sun_direction[1],
            -sky.sun_direction[2],
        ];
        let length = to_sun.iter().map(|v| v * v).sum::<f32>().sqrt().max(1e-6);
        let sun_dot = (0..3)
            .map(|i| direction[i] * to_sun[i] / length)
            .sum::<f32>();
        let disc = smoothstep(0.012_f32.cos(), 0.007_f32.cos(), sun_dot);
        let glow = (-(1.0 - sun_dot).max(0.0) * 180.0).exp();
        let strength = disc * 4.0 + glow * sky.sun_intensity * 0.18;
        for (c, tint) in color.iter_mut().zip([1.0, 0.75, 0.43]) {
            *c += tint * strength;
        }
    }
    if sky.clouds {
        // `cloud_amount` already includes the layer's 0.78 opacity, as in the shader's `mix(color, cloud, cloud * 0.78)`.
        let amount = cloud_amount(direction, sky.cloud_cover, sky.seed);
        color = mix(color, [0.90, 0.92, 0.96], amount);
    }
    Color::srgb(color[0], color[1], color[2])
        .to_linear()
        .to_f32_array_no_alpha()
}

/// The sky as a cube of `size` texels per face, sampled at every texel centre.
#[must_use]
pub fn sky_cubemap(sky: &SkyBackground, size: u32, with_sun: bool) -> CubeFaces {
    CubeFaces {
        size,
        faces: std::array::from_fn(|face| {
            (0..size)
                .flat_map(|y| (0..size).map(move |x| (x, y)))
                .map(|(x, y)| {
                    let rgb = sky_radiance(sky, texel_direction(face, size, x, y), with_sun);
                    [rgb[0], rgb[1], rgb[2], 1.0]
                })
                .collect()
        }),
    }
}

/// A cube texture of `RGBA16F` texels from a chain of mips, in the layout wgpu expects: every mip of a face, then the next face.
#[must_use]
pub fn cube_image(mips: &[CubeFaces]) -> Image {
    let Some(base) = mips.first() else {
        return Image::default();
    };
    let texels = |faces: &CubeFaces, face: usize| -> Vec<u8> {
        faces.faces[face]
            .iter()
            .flat_map(|t| t.iter().flat_map(|c| f16::from_f32(*c).to_le_bytes()))
            .collect()
    };
    let size = Extent3d {
        width: base.size,
        height: base.size,
        depth_or_array_layers: 6,
    };
    let first_mip: Vec<u8> = (0..6).flat_map(|face| texels(base, face)).collect();
    let mut image = Image::new(
        size,
        TextureDimension::D2,
        first_mip,
        TextureFormat::Rgba16Float,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.data = Some(
        (0..6)
            .flat_map(|face| mips.iter().flat_map(move |mip| texels(mip, face)))
            .collect(),
    );
    image.texture_descriptor.mip_level_count = u32::try_from(mips.len()).unwrap_or(1);
    image.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::Cube),
        ..TextureViewDescriptor::default()
    });
    image.sampler = ImageSampler::linear();
    image
}

fn map_environment(
    mut commands: Commands,
    sky: Res<SkyBackground>,
    mut images: ResMut<Assets<Image>>,
    cameras: Query<(Entity, Has<EnvironmentMapLight>), With<Camera3d>>,
    added: Query<Entity, Added<Camera3d>>,
) {
    if !sky.is_changed() && added.is_empty() {
        return;
    }
    if !sky.enabled {
        for (entity, has_light) in &cameras {
            if has_light {
                commands
                    .entity(entity)
                    .remove::<(Skybox, EnvironmentMapLight)>();
            }
        }
        return;
    }
    let cd = units::scene_unit_cd();
    // With the atmosphere (WebGPU) the sky is drawn by it, so only the environment light comes from the cubemap: Bevy's own atmosphere
    // light is generated per camera with compute and went dark on the eye cameras of a split view that render into the lens image.
    let skybox = images.add(cube_image(&[sky_cubemap(&sky, SKYBOX_SIZE, true)]));
    let source = sky_cubemap(&sky, SOURCE_SIZE, false);
    let diffuse = images.add(cube_image(&[irradiance_cubemap(&source, IRRADIANCE_SIZE)]));
    let specular = images.add(cube_image(&prefiltered_mips(&source, SPECULAR_MIPS)));
    for (entity, _) in &cameras {
        let mut camera = commands.entity(entity);
        camera.insert(EnvironmentMapLight {
            diffuse_map: diffuse.clone(),
            specular_map: specular.clone(),
            intensity: cd,
            rotation: Quat::IDENTITY,
            affects_lightmapped_mesh_diffuse: true,
        });
        if !cfg!(feature = "atmosphere") {
            camera.insert(Skybox {
                image: Some(skybox.clone()),
                brightness: cd,
                rotation: Quat::IDENTITY,
            });
        }
    }
}
