//! Benchmark scenes for measuring frame time on a headset: a room with props, sized by draw calls
//! and triangles so runs are comparable (S1 light, S2 dense, S3 stress).
//!
//! Every prop gets its own mesh asset, so batching cannot merge them and the draw count is exactly
//! the number of props. All geometry comes from the kernel; placement is a fixed pseudo-random
//! sequence, so every run draws the same scene.

use std::f32::consts::TAU;

use bevy_asset::Assets;
use bevy_camera::Camera3d;
use bevy_color::Color;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::Commands;
use bevy_math::{Mat4, Quat, Vec3};
use bevy_mesh::{Indices, Mesh, Mesh3d};
use bevy_transform::components::Transform;
use ve_kernel::{KernelError, Solid};
use ve_render::{VeAmbientLight, VeDirectionalLight, VeMaterial, VePointLight};
use ve_xr_web::{XrCamera, XrRig};

use crate::scene::render_mesh;

/// How big a benchmark scene is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BenchSpec {
    /// Props, and so draw calls.
    pub(crate) draws: u32,
    /// Triangles over all props.
    pub(crate) triangles: u32,
}

impl BenchSpec {
    /// S1: an empty room and a few dozen props.
    pub(crate) const LIGHT: Self = Self {
        draws: 40,
        triangles: 50_000,
    };
    /// S2: a dense room.
    pub(crate) const DENSE: Self = Self {
        draws: 150,
        triangles: 200_000,
    };
    /// S3: a stress test.
    pub(crate) const STRESS: Self = Self {
        draws: 300,
        triangles: 400_000,
    };
}

/// What the scene actually contains, for the status report.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct SceneStats {
    /// Mesh draw calls per frame (props plus the room).
    pub(crate) draws: u32,
    /// Triangles drawn per frame.
    pub(crate) triangles: u64,
}

impl SceneStats {
    /// Counts one more mesh drawn every frame.
    fn count(&mut self, mesh: &Mesh) {
        self.draws += 1;
        // Flat-shaded meshes have unshared vertices and no index buffer: three vertices a triangle.
        let corners = mesh
            .indices()
            .map_or_else(|| mesh.count_vertices(), Indices::len);
        self.triangles += u64::try_from(corners / 3).unwrap_or(0);
    }
}

/// A small deterministic pseudo-random sequence (a linear congruential generator).
struct Lcg(u32);

impl Lcg {
    fn next_unit(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        // The top 24 bits fit an f32 mantissa exactly.
        #[expect(clippy::cast_precision_loss, reason = "24 bits fit an f32 exactly")]
        let value = (self.0 >> 8) as f32;
        value / 16_777_216.0
    }

    fn range(&mut self, low: f32, high: f32) -> f32 {
        low + (high - low) * self.next_unit()
    }
}

/// The shape `build(segments)` whose triangle count is closest to `target`. Triangle counts grow
/// with the segment count, so the search stops once it has passed the target.
fn shape_near(
    target: usize,
    build: impl Fn(u32) -> Result<Solid, KernelError>,
) -> Result<Solid, KernelError> {
    let mut best = build(8)?;
    let mut best_gap = best.triangle_count().abs_diff(target);
    for segments in (12_u32..=240).step_by(4) {
        let candidate = build(segments)?;
        let gap = candidate.triangle_count().abs_diff(target);
        if gap < best_gap {
            best = candidate;
            best_gap = gap;
        } else {
            break;
        }
    }
    Ok(best)
}

/// A cube with a sphere bitten out of it (the sphere is wider than the cube, so it cuts through
/// every face).
fn bitten_cube(segments: u32) -> Result<Solid, KernelError> {
    Solid::cube([1.0, 1.0, 1.0], true)?.difference(&Solid::sphere(0.62, segments)?)
}

/// A floor and four walls, one draw each.
fn spawn_room(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    stats: &mut SceneStats,
) -> Result<(), KernelError> {
    let room = [
        ([14.0, 0.1, 14.0], Vec3::new(0.0, -0.05, 0.0)),
        ([14.0, 3.0, 0.1], Vec3::new(0.0, 1.5, -7.0)),
        ([14.0, 3.0, 0.1], Vec3::new(0.0, 1.5, 7.0)),
        ([0.1, 3.0, 14.0], Vec3::new(-7.0, 1.5, 0.0)),
        ([0.1, 3.0, 14.0], Vec3::new(7.0, 1.5, 0.0)),
    ];
    for (size, position) in room {
        let mesh = render_mesh(&Solid::cube(size, true)?, false)?;
        stats.count(&mesh);
        commands.spawn((
            Mesh3d(meshes.add(mesh)),
            VeMaterial {
                base_color: Color::srgb(0.62, 0.64, 0.68),
                roughness: 0.9,
                ..VeMaterial::default()
            },
            Transform::from_translation(position),
        ));
    }
    Ok(())
}

/// `spec.draws` props, every one a separate mesh asset (a clone of one of two kernel shapes).
fn spawn_props(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    stats: &mut SceneStats,
    spec: BenchSpec,
) -> Result<(), KernelError> {
    let per_prop = (spec.triangles as usize / spec.draws.max(1) as usize).max(12);
    let ball = render_mesh(
        &shape_near(per_prop, |segments| Solid::sphere(0.5, segments))?,
        true,
    )?;
    let bitten = render_mesh(&shape_near(per_prop, bitten_cube)?, false)?;
    let mut random = Lcg(0x00C0_FFEE);
    for index in 0..spec.draws {
        let (mesh, size) = if index % 2 == 0 {
            (ball.clone(), random.range(0.25, 0.5))
        } else {
            (bitten.clone(), random.range(0.3, 0.7))
        };
        stats.count(&mesh);
        let hue = random.next_unit();
        commands.spawn((
            Mesh3d(meshes.add(mesh)),
            VeMaterial {
                base_color: Color::hsl(hue * 360.0, 0.55, 0.6),
                roughness: random.range(0.25, 0.9),
                metallic: if index % 5 == 0 { 0.9 } else { 0.0 },
                ..VeMaterial::default()
            },
            Transform::from_xyz(random.range(-5.5, 5.5), size, random.range(-5.5, 5.5))
                .with_scale(Vec3::splat(size))
                .with_rotation(Quat::from_rotation_y(random.range(0.0, TAU))),
        ));
    }
    Ok(())
}

/// A sun, four warm point lights and a headset camera standing at the room's edge.
fn spawn_lights_and_camera(commands: &mut Commands, rig: &mut XrRig) {
    commands.spawn((
        VeDirectionalLight {
            color: Color::WHITE,
            intensity: 2.5,
        },
        Transform::IDENTITY.looking_to(Vec3::new(-0.4, -0.8, -0.45), Vec3::Y),
    ));
    for (x, z) in [(-3.0_f32, -3.0_f32), (3.0, -3.0), (-3.0, 3.0), (3.0, 3.0)] {
        commands.spawn((
            VePointLight {
                color: Color::srgb(1.0, 0.85, 0.7),
                intensity: 8.0,
                range: 7.0,
            },
            Transform::from_xyz(x, 2.4, z),
        ));
    }
    rig.0 = Mat4::from_translation(Vec3::new(0.0, 0.0, 4.5));
    commands.spawn((
        Camera3d::default(),
        XrCamera,
        Transform::from_xyz(0.0, 1.6, 4.5).looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y),
    ));
}

/// Builds the room, the props, the lights and a headset camera, and records what the scene contains.
///
/// # Errors
/// Fails if the kernel cannot build a shape.
pub(crate) fn bench_scene(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    ambient: &mut VeAmbientLight,
    rig: &mut XrRig,
    spec: BenchSpec,
) -> Result<(), KernelError> {
    ambient.intensity = 0.25;
    let mut stats = SceneStats::default();
    spawn_room(commands, meshes, &mut stats)?;
    spawn_props(commands, meshes, &mut stats, spec)?;
    spawn_lights_and_camera(commands, rig);
    commands.insert_resource(stats);
    Ok(())
}
