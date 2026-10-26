//! Built-in particle showcase and bounded billboard entity pool.
use bevy_asset::{Assets, RenderAssetUsages};
use bevy_camera::{Camera3d, visibility::Visibility};
use bevy_color::Color;
use bevy_ecs::{
    component::Component,
    resource::Resource,
    system::{Commands, Query, Res, ResMut},
};
use bevy_math::Vec3;
use bevy_mesh::{Indices, Mesh, Mesh3d, PrimitiveTopology};
use bevy_time::Time;
use bevy_transform::components::Transform;
use std::sync::atomic::{AtomicUsize, Ordering};
use ve_kernel::{KernelError, Solid};
use ve_particles::{Colour, ParticleSystem, Preset, Vec3 as ParticleVec3};
use ve_render::{VeAmbientLight, VeDirectionalLight, VeMaterial};

const POOL_SIZE: usize = 1500;
static LIVE: AtomicUsize = AtomicUsize::new(0);
static EXPLOSION: AtomicUsize = AtomicUsize::new(0);

#[derive(Resource, Default)]
pub(crate) struct FxSystems(Vec<(ParticleSystem, Vec3)>);
#[derive(Component)]
pub(crate) struct FxQuad;

/// Current live particle count, for browser smoke checks.
#[must_use]
pub(crate) fn live_count() -> usize {
    LIVE.load(Ordering::Relaxed)
}

/// Request another explosion burst from the scene.
pub(crate) fn request_explosion() {
    EXPLOSION.fetch_add(1, Ordering::Relaxed);
}

/// Create the outdoor night scene and a fixed pool of reusable quad entities.
pub(crate) fn spawn(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    ambient: &mut VeAmbientLight,
) -> Result<(), KernelError> {
    ambient.intensity = 0.06;
    let ground = meshes.add(crate::scene::render_mesh(
        &Solid::cube([24.0, 0.12, 18.0], true)?,
        false,
    )?);
    commands.spawn((
        Mesh3d(ground),
        VeMaterial {
            base_color: Color::srgb(0.045, 0.07, 0.09),
            roughness: 0.95,
            ..VeMaterial::default()
        },
        Transform::from_xyz(0.0, -0.08, 0.0),
    ));
    commands.spawn((
        VeDirectionalLight {
            color: Color::srgb(0.45, 0.55, 0.8),
            intensity: 1.3,
        },
        Transform::from_xyz(-2.0, 8.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 4.2, 12.0).looking_at(Vec3::new(0.0, 1.1, 0.0), Vec3::Y),
    ));
    let emitters = [
        (Preset::Fire, [-3.2, 0.2, 0.0], 1.0, [1.0, 0.32, 0.04]),
        (Preset::Smoke, [-3.2, 0.2, 0.0], 1.0, [0.32, 0.35, 0.38]),
        (Preset::Embers, [-3.2, 0.2, 0.0], 0.7, [1.0, 0.22, 0.015]),
        (Preset::Fountain, [0.0, 0.3, 0.0], 0.8, [0.15, 0.55, 1.0]),
        (Preset::Splash, [0.0, 0.3, 0.0], 0.7, [0.35, 0.75, 1.0]),
        (Preset::Rain, [3.6, 2.5, 0.0], 1.0, [0.42, 0.6, 0.85]),
        (Preset::Steam, [5.0, 0.25, 0.0], 0.7, [0.7, 0.78, 0.82]),
        (Preset::Magic, [6.8, 0.7, 0.0], 0.7, [0.55, 0.2, 1.0]),
        (
            Preset::Explosion,
            [2.0, 0.25, -3.2],
            0.85,
            [1.0, 0.28, 0.035],
        ),
        (Preset::Sparks, [2.0, 0.25, -3.2], 0.7, [1.0, 0.75, 0.18]),
        (Preset::Smoke, [2.0, 0.25, -3.2], 0.6, [0.34, 0.35, 0.4]),
    ];
    let systems = emitters
        .into_iter()
        .enumerate()
        .map(|(index, (preset, pos, scale, tint))| {
            let colour = Colour {
                r: tint[0],
                g: tint[1],
                b: tint[2],
                a: 1.0,
            };
            let mut spec = preset.spec(scale, colour);
            spec.seed = 0x4658_0000 + index as u64;
            spec.max_particles = 140;
            (ParticleSystem::new(spec), Vec3::new(pos[0], pos[1], pos[2]))
        })
        .collect();
    commands.insert_resource(FxSystems(systems));
    let quad = meshes.add(quad_mesh());
    for _ in 0..POOL_SIZE {
        commands.spawn((
            FxQuad,
            Mesh3d(quad.clone()),
            VeMaterial::default(),
            Transform::default(),
            Visibility::Hidden,
        ));
    }
    Ok(())
}

/// Step every deterministic system and refresh the fixed billboard pool.
pub(crate) fn step(
    time: Res<Time>,
    systems: Option<ResMut<FxSystems>>,
    mut quads: Query<
        (&mut Transform, &mut VeMaterial, &mut Visibility),
        bevy_ecs::query::With<FxQuad>,
    >,
    // Disjoint from the quads' mutable `Transform` access: a camera is never one of the pool's quads.
    cameras: Query<
        &Transform,
        (
            bevy_ecs::query::With<Camera3d>,
            bevy_ecs::query::Without<FxQuad>,
        ),
    >,
) {
    // Only the `fx` scene creates the systems; every other scene runs this system and has nothing to do.
    let Some(mut systems) = systems else {
        return;
    };
    let camera = cameras.iter().next().copied().unwrap_or_default();
    let camera_pos = camera.translation;
    let mut live = Vec::new();
    let mut trigger = EXPLOSION.swap(0, Ordering::Relaxed);
    for (index, (system, origin)) in systems.0.iter_mut().enumerate() {
        if trigger > 0 && (index == 8 || index == 9) {
            system.burst(if index == 8 { 180 } else { 80 });
            trigger -= 1;
        }
        system.step(
            time.delta_secs(),
            bevy_math::Mat4::from_translation(*origin),
        );
        live.extend(system.sorted_instances(ParticleVec3 {
            x: camera_pos.x,
            y: camera_pos.y,
            z: camera_pos.z,
        }));
    }
    live.truncate(POOL_SIZE);
    LIVE.store(live.len(), Ordering::Relaxed);
    for (index, (mut transform, mut material, mut visibility)) in quads.iter_mut().enumerate() {
        let Some(particle) = live.get(index) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        *visibility = Visibility::Visible;
        transform.translation = Vec3::new(
            particle.position.x,
            particle.position.y,
            particle.position.z,
        );
        transform.rotation = camera.rotation * bevy_math::Quat::from_rotation_z(particle.rotation);
        transform.scale = Vec3::splat(particle.size.max(0.001));
        let c = particle.colour;
        material.base_color = Color::linear_rgba(c.r, c.g, c.b, c.a);
        material.opacity = c.a;
        material.emissive = Color::linear_rgb(c.r * 4.0, c.g * 4.0, c.b * 4.0);
        material.roughness = 0.0;
    }
}

pub(crate) fn quad_mesh() -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    // A round billboard of radius 0.5 facing +Z: a centre vertex and a ring, as a triangle fan. A disc reads as a soft puff or a spark where
    // a square reads as a box.
    const RING: u16 = 20;
    let mut positions = vec![[0.0_f32, 0.0, 0.0]];
    for step in 0..RING {
        let angle = f32::from(step) / f32::from(RING) * std::f32::consts::TAU;
        positions.push([0.5 * angle.cos(), 0.5 * angle.sin(), 0.0]);
    }
    let mut indices = Vec::new();
    for step in 0..u32::from(RING) {
        indices.extend([0, 1 + step, 1 + (step + 1) % u32::from(RING)]);
    }
    let count = positions.len();
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; count]);
    // The forward pipeline reads baked occlusion at location 2 for every mesh; a billboard is fully open.
    mesh.insert_attribute(ve_render::ATTRIBUTE_OCCLUSION, vec![1.0_f32; count]);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}
