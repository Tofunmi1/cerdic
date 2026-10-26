//! Deterministic, geometry-only camera surveys of reachable surfaces.

mod geometry;
mod regions;
mod sample;
mod select;

use geometry::{Scene, add};

/// A world-space, Y-up triangle; winding determines its geometric normal.
pub type Triangle = [[f64; 3]; 3];

/// How to choose between standing views and views around the geometry.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SurveyMode {
    /// Walk on substantial low floor regions, otherwise orbit the geometry.
    #[default]
    Auto,
    /// Stand wherever clearance permits, orbiting only when no points remain.
    Walk,
    /// Always view the geometry from aerial rings.
    Orbit,
}

/// Sampling density, camera geometry, and the greedy coverage budget.
#[derive(Clone, Debug)]
pub struct SurveyOptions {
    /// Standing, aerial, or automatic camera placement.
    pub mode: SurveyMode,
    /// Height of the camera above a walkable floor, in metres.
    pub eye_height: f64,
    /// Minimum horizontal separation of standing points, in metres.
    pub spacing: f64,
    /// Horizontal and vertical camera field of view, in degrees.
    pub fov_deg: f64,
    /// Equally spaced headings at each standing point.
    pub yaws: usize,
    /// Upward pitch of standing cameras, in degrees.
    pub pitch_deg: f64,
    /// Maximum number of returned cameras.
    pub max_views: usize,
    /// Desired fraction of reachable surface samples covered well.
    pub target_coverage: f64,
    /// Maximum distance at which a surface sample can be seen.
    pub max_distance: f64,
    /// Surface samples per square metre; standing density is half this.
    pub samples_per_m2: f64,
    /// Seed for deterministic area sampling.
    pub seed: u64,
}

impl Default for SurveyOptions {
    fn default() -> Self {
        Self {
            mode: SurveyMode::Auto,
            eye_height: 1.65,
            spacing: 4.0,
            fov_deg: 90.0,
            yaws: 8,
            pitch_deg: 6.0,
            max_views: 12,
            target_coverage: 0.95,
            max_distance: 30.0,
            samples_per_m2: 2.0,
            seed: 7,
        }
    }
}

/// A camera suitable for passing directly to a snapshot renderer.
#[derive(Clone, Debug)]
pub struct SurveyCamera {
    /// World-space eye position.
    pub position: [f64; 3],
    /// World-space look target, one unit from the eye.
    pub target: [f64; 3],
    /// Stable, one-based label in selection order.
    pub label: String,
}

/// An ordered camera set and its predicted reachable-surface coverage.
#[derive(Clone, Debug)]
pub struct SurveyPlan {
    /// Placement actually used: Walk or Orbit (Orbit when geometry is absent).
    pub mode: SurveyMode,
    /// Cameras in greedy selection order (ring order for aerial fallback).
    pub cameras: Vec<SurveyCamera>,
    /// Final coverage, or zero when no cameras were selected.
    pub coverage: f64,
    /// Coverage after each returned camera.
    pub predicted: Vec<f64>,
    /// Number of floor points retained for standing candidates in the chosen mode.
    pub standable_points: usize,
    /// Number of area-weighted samples over all valid triangles.
    pub surface_samples: usize,
}

impl Default for SurveyPlan {
    fn default() -> Self {
        Self {
            mode: SurveyMode::Orbit,
            cameras: Vec::new(),
            coverage: 0.0,
            predicted: Vec::new(),
            standable_points: 0,
            surface_samples: 0,
        }
    }
}

/// Plans standing views or aerial rings according to the requested survey mode.
///
/// Degenerate/non-finite triangles are ignored. Invalid sampling or camera options
/// return an empty plan. Auto retains only substantial low floor regions;
/// Walk retains small disconnected walkable areas as well.
#[must_use]
pub fn plan_cameras(triangles: &[Triangle], options: &SurveyOptions) -> SurveyPlan {
    if !valid(options) {
        return SurveyPlan::default();
    }
    let scene = Scene::new(triangles);
    if scene.faces.is_empty() {
        return SurveyPlan::default();
    }
    let mut rng = sample::Random(options.seed);
    let points = sample::standing(&scene, options, &mut rng);
    let points = match options.mode {
        SurveyMode::Auto => regions::retain(&scene, points, options),
        SurveyMode::Walk => points,
        SurveyMode::Orbit => Vec::new(),
    };
    let samples = sample::surface(&scene, options.samples_per_m2, &mut rng);
    let aerial = points.is_empty();
    let candidates = if aerial {
        sample::aerial(&scene, options.max_views)
    } else {
        let mut cameras = Vec::new();
        let pitch = options.pitch_deg.to_radians();
        for position in &points {
            let offset = rng.unit() * std::f64::consts::TAU / geometry::number(options.yaws);
            for heading in 0..options.yaws {
                let yaw = offset
                    + std::f64::consts::TAU * geometry::number(heading)
                        / geometry::number(options.yaws);
                let direction = [
                    yaw.sin() * pitch.cos(),
                    pitch.sin(),
                    yaw.cos() * pitch.cos(),
                ];
                cameras.push(SurveyCamera {
                    position: *position,
                    target: add(*position, direction),
                    label: String::new(),
                });
            }
        }
        cameras
    };
    let mut plan = select::cover(&scene, &samples, &candidates, options, aerial);
    plan.mode = if aerial {
        SurveyMode::Orbit
    } else {
        SurveyMode::Walk
    };
    plan.standable_points = points.len();
    plan.surface_samples = samples.len();
    plan
}

fn valid(o: &SurveyOptions) -> bool {
    [
        o.eye_height,
        o.spacing,
        o.fov_deg,
        o.pitch_deg,
        o.target_coverage,
        o.max_distance,
        o.samples_per_m2,
    ]
    .iter()
    .all(|v| v.is_finite())
        && o.eye_height > 0.0
        && o.spacing > 0.0
        && o.fov_deg > 0.0
        && o.fov_deg < 180.0
        && o.max_distance > 0.0
        && o.samples_per_m2 > 0.0
        && o.yaws > 0
        && (0.0..=1.0).contains(&o.target_coverage)
}
