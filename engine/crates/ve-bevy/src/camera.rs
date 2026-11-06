//! What every 3D camera gets for Bevy's pipeline: an HDR target with tone mapping, and the effects the person's quality settings and the
//! backend allow (bloom, anti aliasing, ambient occlusion, exposure, sun shadows), plus the atmosphere when the scene asks for a sky.

use bevy_anti_alias::smaa::Smaa;
use bevy_app::{App, Plugin, Update};
use bevy_asset::{Assets, Handle};
use bevy_camera::{Camera3d, Hdr};
use bevy_color::Color;
use bevy_core_pipeline::prepass::NormalPrepass;
use bevy_ecs::prelude::*;
use bevy_light::atmosphere::{Atmosphere, ScatteringMedium};
use bevy_light::{DirectionalLight, DirectionalLightShadowMap, FogVolume, VolumetricFog, VolumetricLight};
use bevy_math::Vec3;
use bevy_pbr::{AtmosphereSettings, ContactShadows, DistanceFog, FogFalloff};
use bevy_post_process::bloom::Bloom;
use bevy_post_process::dof::{DepthOfField, DepthOfFieldMode};
use bevy_post_process::motion_blur::MotionBlur;
use bevy_render::render_resource::TextureUsages;
use bevy_render::view::{ColorGrading, ColorGradingGlobal, ColorGradingSection, Msaa, Tonemapping};
use bevy_transform::components::Transform;
use ve_render::{Grading, RenderQuality, SkyBackground};

use crate::stereo::EyeCamera;

/// Registers the camera systems.
#[derive(Debug, Default, Clone, Copy)]
pub struct VeBevyCameraPlugin;

impl Plugin for VeBevyCameraPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(DirectionalLightShadowMap { size: 2048 })
            .add_systems(
                Update,
                (
                    map_cameras,
                    map_sky.after(map_cameras),
                    map_grading,
                    map_haze,
                    map_volumetric,
                    map_camera_fx,
                    drift_clouds,
                    crate::ground::map_ground,
                ),
            );
    }
}

/// Aerial perspective: distant things fade toward the horizon colour and the air glows warm toward the sun, the cue that tells a photograph from a
/// render. It is on whenever the scene has a sky; a hazier sky (higher turbidity) gets thicker air. Exponential-squared, so what is near stays clear.
fn map_haze(
    mut commands: Commands,
    sky: Res<SkyBackground>,
    cameras: Query<(Entity, Has<DistanceFog>), With<Camera3d>>,
    added: Query<Entity, Added<Camera3d>>,
) {
    if !sky.is_changed() && added.is_empty() {
        return;
    }
    for (entity, has_fog) in &cameras {
        if !sky.enabled {
            if has_fog {
                commands.entity(entity).remove::<DistanceFog>();
            }
            continue;
        }
        // the horizon colour lightened toward a pale haze: the sky's own horizon is bright, and a deep blue band would read as sea
        let [r, g, b] = sky.horizon.map(|c| c * 0.4 + 0.6 * 0.82);
        // The document's `haze` (0 clear to 1 thick) thickens the air on top of what the turbidity gives, and `haze_color` tints it:
        // the more haze, the more of the authored colour. (Before this, the document's haze never reached the renderer.)
        let haze = sky.haze.clamp(0.0, 1.0);
        let tint = (haze * 3.0).min(1.0);
        let [hr, hg, hb] = sky.haze_color;
        let (r, g, b) = (r + (hr - r) * tint, g + (hg - g) * tint, b + (hb - b) * tint);
        commands.entity(entity).insert(DistanceFog {
            color: Color::srgb(r, g, b),
            directional_light_color: Color::srgba(1.0, 0.78, 0.55, 0.55),
            directional_light_exponent: 28.0,
            falloff: FogFalloff::ExponentialSquared {
                density: 0.00035 * sky.turbidity.clamp(1.0, 10.0) + 0.006 * haze,
            },
        });
    }
}

/// The document's camera effects as Bevy's own post-processing components: depth of field (`focus_distance` above zero), motion blur
/// (`motion_blur` above zero) and screen-space contact shadows (`contact_shadows`). Each is added when asked for and removed again when
/// not, so a document that does not mention them pays nothing.
fn map_camera_fx(
    mut commands: Commands,
    sky: Res<SkyBackground>,
    cameras: Query<(Entity, Has<DepthOfField>, Has<MotionBlur>, Has<ContactShadows>), With<Camera3d>>,
    added: Query<Entity, Added<Camera3d>>,
) {
    if !sky.is_changed() && added.is_empty() {
        return;
    }
    for (entity, has_dof, has_blur, has_contact) in &cameras {
        let mut camera = commands.entity(entity);
        if sky.focus_distance > 0.0 {
            camera.insert(DepthOfField {
                mode: DepthOfFieldMode::Gaussian,
                focal_distance: sky.focus_distance,
                aperture_f_stops: sky.aperture.clamp(0.5, 22.0),
                ..DepthOfField::default()
            });
        } else if has_dof {
            camera.remove::<DepthOfField>();
        }
        if sky.motion_blur > 0.0 {
            camera.insert(MotionBlur {
                shutter_angle: sky.motion_blur.clamp(0.0, 1.0),
                samples: 4,
            });
        } else if has_blur {
            camera.remove::<MotionBlur>();
        }
        if sky.contact_shadows {
            camera.insert(ContactShadows::default());
        } else if has_contact {
            camera.remove::<ContactShadows>();
        }
    }
}

/// Marks the single fog volume this module spawns for the document's ground haze and light shafts.
#[derive(Component)]
pub(crate) struct HazeVolume;

/// Sunlight shafts and ground haze, drawn by Bevy's volumetric fog. They are on when the document's sky asks for either: `light_shafts`
/// above zero makes the sun scatter through the air (visible as rays past trees and walls), and `haze_height` above zero lays a layer of
/// ground haze up to that height, as thick as `haze` says. Both off removes the fog volume and the camera component again.
fn map_volumetric(
    mut commands: Commands,
    sky: Res<SkyBackground>,
    mut cameras: Query<(Entity, &mut Camera3d, Has<VolumetricFog>)>,
    suns: Query<(Entity, Has<VolumetricLight>), With<DirectionalLight>>,
    volume: Query<Entity, With<HazeVolume>>,
    new_suns: Query<Entity, Added<DirectionalLight>>,
) {
    // Reading `is_added` does not mark the camera changed (only writing through the reference does), and a second `Added` query on
    // the same component would conflict with this system's mutable one.
    let camera_added = cameras.iter_mut().any(|(_, camera, _)| camera.is_added());
    if !sky.is_changed() && !camera_added && new_suns.is_empty() {
        return;
    }
    let wanted = sky.shafts > 0.0 || sky.haze_height > 0.0;
    for (entity, mut camera_3d, has) in &mut cameras {
        if wanted {
            // The fog reads the scene depth as a texture, which Bevy only allows on a camera made with that usage.
            let binding = TextureUsages::TEXTURE_BINDING.bits();
            if camera_3d.depth_texture_usages.0 & binding == 0 {
                camera_3d.depth_texture_usages.0 |= binding;
            }
            if !has {
                commands.entity(entity).insert(VolumetricFog {
                    ambient_intensity: 0.0,
                    step_count: 48,
                    jitter: 0.0,
                    ..VolumetricFog::default()
                });
            }
        } else if has {
            commands.entity(entity).remove::<VolumetricFog>();
        }
    }
    for (entity, has) in &suns {
        match (wanted && sky.shafts > 0.0, has) {
            (true, false) => {
                commands.entity(entity).insert(VolumetricLight);
            }
            (false, true) => {
                commands.entity(entity).remove::<VolumetricLight>();
            }
            _ => {}
        }
    }
    for entity in &volume {
        commands.entity(entity).despawn();
    }
    if wanted {
        // A wide box hugging the ground: as tall as the haze (or tall enough to carry the rays), as dense as the document says.
        let height = if sky.haze_height > 0.0 { sky.haze_height } else { 200.0 };
        let density = sky.shafts.clamp(0.0, 1.0) * 0.03
            + if sky.haze_height > 0.0 { sky.haze.clamp(0.0, 1.0) * 0.08 } else { 0.0 };
        let [r, g, b] = sky.haze_color;
        commands.spawn((
            HazeVolume,
            FogVolume {
                fog_color: Color::srgb(r, g, b),
                density_factor: density,
                ..FogVolume::default()
            },
            Transform::from_xyz(0.0, height * 0.5, 0.0).with_scale(Vec3::new(1600.0, height, 1600.0)),
        ));
    }
}

/// The viewer's grading (exposure, contrast, saturation, tint) as Bevy's colour grading. Bevy applies it before tone mapping, ours came
/// after (contrast, saturation), so the looks differ a little at the extremes. The tint becomes a white balance: red minus blue is the temperature (warm is more red than blue; scaled by 0.45 because Bevy's white
/// balance is strong, a raw 0.3 turns the picture solid orange or cyan) and green above 1 is Bevy's magenta tint, which keeps a cool grade blue
/// where cooling alone drifts to green.
fn map_grading(
    mut commands: Commands,
    grading: Res<Grading>,
    cameras: Query<Entity, With<Camera3d>>,
    added: Query<Entity, Added<Camera3d>>,
) {
    if !grading.is_changed() && added.is_empty() {
        return;
    }
    let section = ColorGradingSection {
        contrast: grading.contrast,
        ..ColorGradingSection::default()
    };
    let color_grading = ColorGrading {
        global: ColorGradingGlobal {
            exposure: grading.exposure.max(f32::MIN_POSITIVE).log2(),
            temperature: (grading.tint[0] - grading.tint[2]) * 0.45,
            tint: (grading.tint[1] - 1.0) * 1.0,
            post_saturation: grading.saturation,
            ..ColorGradingGlobal::default()
        },
        shadows: section,
        midtones: section,
        highlights: section,
    };
    for entity in &cameras {
        commands.entity(entity).insert(color_grading.clone());
    }
}

/// Applies [`RenderQuality`] to the cameras and the sun when it changes or a camera appears.
pub(crate) fn map_cameras(
    mut commands: Commands,
    quality: Res<RenderQuality>,
    mut cameras: Query<(Entity, &mut Camera3d, Has<EyeCamera>)>,
    mut suns: Query<&mut DirectionalLight>,
    mut shadow_map: ResMut<DirectionalLightShadowMap>,
) {
    // Reading `is_added` does not mark the camera changed; only writing through the reference does.
    if !quality.is_changed() && !cameras.iter_mut().any(|(_, camera, _)| camera.is_added()) {
        return;
    }
    for (entity, mut camera_3d, is_eye) in &mut cameras {
        // The atmosphere reads the scene depth as a texture. Bevy adds that usage only on the frame a camera changes while it already
        // carries the atmosphere, so a camera that gets it later (the eye cameras of a split view) would draw with an invalid bind group.
        let binding = TextureUsages::TEXTURE_BINDING.bits();
        if cfg!(feature = "atmosphere") && camera_3d.depth_texture_usages.0 & binding == 0 {
            camera_3d.depth_texture_usages.0 |= binding;
        }
        let mut camera = commands.entity(entity);
        // Close to the identity in the middle of the range, so authored colours stay as authored (Tony McMapface turns a bright saturated
        // colour white; Khronos Neutral boosts the saturation of dark colours).
        camera.insert((Hdr, Tonemapping::SomewhatBoringDisplayTransform));
        if quality.bloom_enabled {
            camera.insert(Bloom::NATURAL);
        } else {
            camera.remove::<Bloom>();
        }
        // Occlusion and exposure are the engine's own passes on every backend (`ao`, `exposure`; Bevy's versions need compute shaders and look
        // different from one backend to the next). The occlusion pass reads the prepass's normal texture (world normal, fragment depth in
        // alpha) and the exposure pass reads the lit colour, so both need single sampled targets.
        if quality.ssao_enabled {
            camera.insert(NormalPrepass);
        } else {
            camera.remove::<NormalPrepass>();
        }
        // No temporal anti aliasing: it needs motion vectors, and parts that move without their entity moving (CPU skinned meshes, bone
        // driven parts, the outline hulls) have none, so TAA left a dark ghost of every moving thing (a swimming shark dragged a black
        // copy of itself). It also squeezes a split view's shared target. SMAA has no history and costs nothing like that.
        #[cfg(feature = "webgpu")]
        {
            let _ = is_eye;
            camera.insert(Msaa::Off);
            // SMAA is the one anti-aliasing here; the person can turn it off to see the raw edges or to save the pass.
            if quality.smaa_enabled {
                camera.insert(Smaa::default());
            } else {
                camera.remove::<Smaa>();
            }
            camera.remove::<bevy_anti_alias::taa::TemporalAntiAliasing>();
        }
        #[cfg(not(feature = "webgpu"))]
        if is_eye || quality.ssao_enabled || quality.auto_exposure_enabled {
            camera.insert((Msaa::Off, Smaa::default()));
        } else {
            camera.insert(Msaa::from_samples(quality.msaa_samples));
            camera.remove::<Smaa>();
        }
    }
    for mut sun in &mut suns {
        sun.shadow_maps_enabled = quality.shadows_enabled();
    }
    if quality.shadow_resolution > 0 {
        shadow_map.size = quality.shadow_resolution as usize;
    }
}

/// The wind carries the clouds: a few metres a second along x and a little along z, so a still view shows them moving.
fn drift_clouds(mut frames: Local<u64>, mut settings: Query<&mut AtmosphereSettings>) {
    *frames += 1;
    #[expect(
        clippy::cast_precision_loss,
        reason = "a frame count in seconds, to a metre of drift"
    )]
    let t = *frames as f32 / 60.0;
    for mut s in &mut settings {
        s.cloud_drift = bevy_math::Vec4::new(t * 7.0, t * 2.5, 0.0, 0.0);
    }
}

/// Bevy's physically based atmosphere, behind the `atmosphere` feature (WebGPU compute). The atmosphere is its own entity whose position
/// is the planet's centre, so that sea level is the plane `y = 0`; each camera only carries the settings. (Put on the camera itself, the
/// viewer would sit at the planet's centre and the sky draws black.) Without the feature the sky is the procedural cubemap of `environment`.
fn map_sky(
    mut commands: Commands,
    sky: Res<SkyBackground>,
    mut media: ResMut<Assets<ScatteringMedium>>,
    mut medium: Local<Option<Handle<ScatteringMedium>>>,
    mut planet: Local<Option<Entity>>,
    cameras: Query<(Entity, Has<AtmosphereSettings>), With<Camera3d>>,
    added: Query<Entity, Added<Camera3d>>,
) {
    if !cfg!(feature = "atmosphere") || (!sky.is_changed() && added.is_empty()) {
        return;
    }
    if sky.enabled && planet.is_none() {
        let handle = medium
            .get_or_insert_with(|| media.add(ScatteringMedium::earth(256, 256)))
            .clone();
        let mut atmosphere = Atmosphere::earth(handle);
        // a grey floor beyond the scene's own ground, not the desert tan of the default
        atmosphere.ground_albedo = bevy_math::Vec3::splat(0.2);
        let centre = Transform::from_xyz(0.0, -atmosphere.inner_radius, 0.0);
        *planet = Some(commands.spawn((atmosphere, centre)).id());
    } else if !sky.enabled
        && let Some(entity) = planet.take()
    {
        commands.entity(entity).despawn();
    }
    for (entity, has_settings) in &cameras {
        let mut camera = commands.entity(entity);
        if sky.enabled {
            // Procedural volumetric clouds (VE patch 5 of bevy_pbr's atmosphere): a layer from 4.2 km up, 2.2 km thick; cover 0 draws none.
            camera.insert(AtmosphereSettings {
                clouds: bevy_math::Vec4::new(
                    if sky.clouds {
                        sky.cloud_cover.clamp(0.05, 1.0)
                    } else {
                        0.0
                    },
                    sky.seed as f32,
                    4200.0,
                    2200.0,
                ),
                ..AtmosphereSettings::default()
            });
        } else if has_settings {
            camera.remove::<AtmosphereSettings>();
        }
    }
}
