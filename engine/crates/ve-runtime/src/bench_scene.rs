//! Benchmark scenes for measuring frame time on a headset: a room with props, sized by draw calls
//! and triangles so runs are comparable (S1 light, S2 dense, S3 stress).
//!
//! Every prop gets its own mesh asset, so batching cannot merge them and the draw count is exactly
//! the number of props. All geometry comes from the kernel; placement is a fixed pseudo-random
//! sequence, so every run draws the same scene.

use std::f32::consts::TAU;

pub(crate) mod showcase;

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
    /// S4: one million triangles, beyond the original Quest targets (a desktop and future-device probe).
    pub(crate) const MILLION: Self = Self {
        draws: 600,
        triangles: 1_000_000,
    };
    /// S5: two million triangles, the evaluator's Final budget.
    pub(crate) const TWO_MILLION: Self = Self {
        draws: 1_000,
        triangles: 2_000_000,
    };
}

/// What the scene is made of, for the status report. These describe the authored scene; what the
/// renderer submits (per eye, per pass) is reported separately from the render counters.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct SceneStats {
    /// Mesh instances in the scene (props plus the room).
    pub(crate) instances: u32,
    /// Separate mesh assets behind them. The benchmark gives every instance its own asset, so
    /// nothing is batched; a shared-mesh workload would show fewer assets than instances.
    pub(crate) mesh_assets: u32,
    /// Triangles in the scene, counting each instance once.
    pub(crate) triangles: u64,
}

impl SceneStats {
    /// Counts one more instance with its own mesh asset.
    fn count(&mut self, mesh: &Mesh) {
        self.instances += 1;
        self.mesh_assets += 1;
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
    wall_height: f32,
) -> Result<(), KernelError> {
    let (h, mid) = (wall_height, wall_height / 2.0);
    // The floor's top sits 5 mm below y = 0 so a model's own base slab (top at y = 0, as authored
    // models are) is not coplanar with it; coplanar faces tie in the depth test and flicker as the
    // camera moves.
    let room = [
        ([14.0, 0.1, 14.0], Vec3::new(0.0, -0.055, 0.0)),
        ([14.0, h, 0.1], Vec3::new(0.0, mid, -7.0)),
        ([14.0, h, 0.1], Vec3::new(0.0, mid, 7.0)),
        ([0.1, h, 14.0], Vec3::new(-7.0, mid, 0.0)),
        ([0.1, h, 14.0], Vec3::new(7.0, mid, 0.0)),
    ];
    for (size, position) in room {
        let size = size.map(f64::from);
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
            crate::studio::StudioRoom {
                base_position: position,
            },
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

/// A sun and four warm point lights.
fn spawn_lights(commands: &mut Commands) {
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
}

/// The lights and a headset camera standing at the room's edge.
fn spawn_lights_and_camera(commands: &mut Commands, rig: &mut XrRig) {
    spawn_lights(commands);
    rig.0 = Mat4::from_translation(Vec3::new(0.0, 0.0, 4.5));
    commands.spawn((
        Camera3d::default(),
        XrCamera,
        Transform::from_xyz(0.0, 1.6, 4.5).looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y),
    ));
}

/// An empty lit room for looking at authored models: the benchmark's floor, walls, sun (which casts
/// shadows onto the floor) and warm lights, with an ordinary camera a little way back.
///
/// # Errors
/// Fails if the kernel cannot build the room.
pub(crate) fn stage_scene(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    ambient: &mut VeAmbientLight,
) -> Result<(), KernelError> {
    ambient.intensity = 0.25;
    ambient.reflection = 1.0;
    // Taller walls than the benchmark's, so little sky shows above the model.
    spawn_room(commands, meshes, &mut SceneStats::default(), 9.0)?;
    spawn_lights(commands);
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 1.0, 2.2).looking_at(Vec3::new(0.0, 0.3, 0.0), Vec3::Y),
    ));
    Ok(())
}

/// A camera and nothing else: no room, no floor, no lights. The document being viewed brings its own.
pub(crate) fn open_scene(commands: &mut Commands) -> Result<(), KernelError> {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 1.6, 4.0).looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y),
    ));
    Ok(())
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
    spawn_room(commands, meshes, &mut stats, 3.0)?;
    spawn_props(commands, meshes, &mut stats, spec)?;
    spawn_lights_and_camera(commands, rig);
    commands.insert_resource(stats);
    Ok(())
}
