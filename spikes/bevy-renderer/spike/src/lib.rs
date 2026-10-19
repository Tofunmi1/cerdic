//! Spike: Bevy 0.20's renderer in a browser. Which effect is switched on comes from the page's query (`?fx=taa,bloom,tonemap=agx`), so
//! one build can be driven effect by effect from a test.
use bevy_internal::{
    anti_alias::{
        contrast_adaptive_sharpening::ContrastAdaptiveSharpening, fxaa::Fxaa, smaa::Smaa,
        taa::TemporalAntiAliasing,
    },
    camera::{Hdr, Viewport},
    core_pipeline::prepass::{DeferredPrepass, DepthPrepass, MotionVectorPrepass, NormalPrepass},
    light::{
        atmosphere::{Atmosphere, ScatteringMedium},
        AtmosphereEnvironmentMapLight, CascadeShadowConfigBuilder,
    },
    pbr::{AtmosphereSettings, ScreenSpaceAmbientOcclusion, ScreenSpaceReflections},
    post_process::{
        auto_exposure::AutoExposure, bloom::Bloom, dof::DepthOfField, motion_blur::MotionBlur,
    },
    prelude::*,
    render::view::{Msaa, Tonemapping},
};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(inline_js = "export function report(frames) { window.__spike = { frames }; }")]
extern "C" {
    fn report(frames: u32);
}

/// The effects asked for in the page's query.
#[derive(Resource, Clone, Default)]
struct Fx(Vec<String>);

impl Fx {
    fn from_page() -> Self {
        let search = web_sys::window()
            .and_then(|w| w.location().search().ok())
            .unwrap_or_default();
        let list = search
            .trim_start_matches('?')
            .split('&')
            .find_map(|pair| pair.strip_prefix("fx="))
            .unwrap_or("");
        Self(
            list.split(',')
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect(),
        )
    }
    fn on(&self, name: &str) -> bool {
        self.0.iter().any(|f| f == name)
    }
    fn value(&self, name: &str) -> Option<&str> {
        self.0
            .iter()
            .find_map(|f| f.strip_prefix(name).and_then(|r| r.strip_prefix('=')))
    }
}

#[wasm_bindgen(start)]
pub fn start() {
    let fx = Fx::from_page();
    App::new()
        .insert_resource(fx)
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                canvas: Some("#bevy".into()),
                fit_canvas_to_parent: true,
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, setup)
        .add_systems(Update, count_frames)
        .run();
}

fn count_frames(mut frames: Local<u32>) {
    *frames += 1;
    if *frames % 10 == 0 {
        report(*frames);
    }
}

fn camera_with_effects(commands: &mut Commands, fx: &Fx, viewport: Option<Viewport>, order: isize) -> Entity {
    let mut camera = commands.spawn((
        Camera3d::default(),
        Camera {
            viewport,
            order,
            ..default()
        },
        Transform::from_xyz(-7.0, 3.5, 9.0).looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y),
        if fx.on("taa") || fx.on("ssao") || fx.on("dof") || fx.on("mb") || fx.on("ssr") {
            Msaa::Off
        } else if let Some(samples) = fx.value("msaa") {
            match samples {
                "1" => Msaa::Off,
                "2" => Msaa::Sample2,
                _ => Msaa::Sample4,
            }
        } else {
            Msaa::Sample4
        },
    ));
    if fx.on("hdr") || fx.on("bloom") || fx.on("atmo") || fx.on("exposure") {
        camera.insert(Hdr);
    }
    if let Some(name) = fx.value("tonemap") {
        camera.insert(match name {
            "agx" => Tonemapping::AgX,
            "aces" => Tonemapping::AcesFitted,
            "reinhard" => Tonemapping::Reinhard,
            "neutral" => Tonemapping::KhronosPbrNeutral,
            "filmic" => Tonemapping::BlenderFilmic,
            "none" => Tonemapping::None,
            _ => Tonemapping::TonyMcMapface,
        });
    }
    if fx.on("bloom") {
        camera.insert(Bloom::NATURAL);
    }
    if fx.on("taa") {
        camera.insert(TemporalAntiAliasing::default());
    }
    if fx.on("smaa") {
        camera.insert(Smaa::default());
    }
    if fx.on("fxaa") {
        camera.insert(Fxaa::default());
    }
    if fx.on("cas") {
        camera.insert(ContrastAdaptiveSharpening::default());
    }
    if fx.on("ssao") {
        camera.insert(ScreenSpaceAmbientOcclusion::default());
    }
    if fx.on("ssr") {
        camera.insert((ScreenSpaceReflections::default(), DeferredPrepass, DepthPrepass, NormalPrepass));
    }
    if fx.on("dof") {
        camera.insert((DepthOfField::default(), DepthPrepass));
    }
    if fx.on("mb") {
        camera.insert((MotionBlur::default(), DepthPrepass, MotionVectorPrepass));
    }
    if fx.on("exposure") {
        camera.insert(AutoExposure::default());
    }
    camera.id()
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut media: ResMut<Assets<ScatteringMedium>>,
    fx: Res<Fx>,
) {
    // Cameras: one, or two side by side for the stereo case.
    let main = if fx.on("stereo") {
        let left = camera_with_effects(
            &mut commands,
            &fx,
            Some(Viewport {
                physical_position: UVec2::new(0, 0),
                physical_size: UVec2::new(400, 520),
                ..default()
            }),
            0,
        );
        camera_with_effects(
            &mut commands,
            &fx,
            Some(Viewport {
                physical_position: UVec2::new(400, 0),
                physical_size: UVec2::new(400, 520),
                ..default()
            }),
            1,
        );
        left
    } else {
        camera_with_effects(&mut commands, &fx, None, 0)
    };
    if fx.on("atmo") {
        let medium = media.add(ScatteringMedium::earth(256, 256));
        commands.entity(main).insert((
            Atmosphere::earth(medium),
            AtmosphereSettings::default(),
        ));
        if fx.on("atmoenv") {
            commands
                .entity(main)
                .insert(AtmosphereEnvironmentMapLight::default());
        }
    }

    // The scene: a floor, rows of spheres from matte to mirror, a cube, a glowing ball.
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(40.0, 40.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.45, 0.42, 0.38),
            perceptual_roughness: 0.9,
            ..default()
        })),
    ));
    let sphere = meshes.add(Sphere::new(0.7));
    for (i, roughness) in [0.05_f32, 0.25, 0.5, 0.8].into_iter().enumerate() {
        for (row, metallic) in [0.0_f32, 1.0].into_iter().enumerate() {
            commands.spawn((
                Mesh3d(sphere.clone()),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.8, 0.5, 0.3),
                    metallic,
                    perceptual_roughness: roughness,
                    ..default()
                })),
                Transform::from_xyz(i as f32 * 1.8 - 2.7, 0.7, row as f32 * 1.8 - 0.9),
            ));
        }
    }
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.2, 2.0, 1.2))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.3, 0.4, 0.7),
            perceptual_roughness: 0.6,
            ..default()
        })),
        Transform::from_xyz(-4.5, 1.0, -2.5),
    ));
    commands.spawn((
        Mesh3d(sphere.clone()),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::BLACK,
            emissive: LinearRgba::rgb(40.0, 18.0, 6.0),
            ..default()
        })),
        Transform::from_xyz(3.2, 2.2, -2.0),
    ));

    // Lights: a sun with cascades, plus a point and a spot light that cast shadows.
    commands.spawn((
        DirectionalLight {
            shadow_maps_enabled: true,
            illuminance: 9000.0,
            ..default()
        },
        CascadeShadowConfigBuilder {
            num_cascades: 3,
            maximum_distance: 40.0,
            ..default()
        }
        .build(),
        Transform::from_xyz(4.0, 8.0, 3.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    if fx.on("point") {
        commands.spawn((
            PointLight {
                shadow_maps_enabled: true,
                intensity: 400_000.0,
                range: 20.0,
                ..default()
            },
            Transform::from_xyz(-2.0, 3.0, 2.5),
        ));
    }
    if fx.on("spot") {
        commands.spawn((
            SpotLight {
                shadow_maps_enabled: true,
                intensity: 600_000.0,
                range: 25.0,
                ..default()
            },
            Transform::from_xyz(3.0, 5.0, 4.0).looking_at(Vec3::new(1.0, 0.0, 0.0), Vec3::Y),
        ));
    }
}
