//! Billboard impostor views.
use super::{ImpostorSpec, ImpostorView, PlantMesh};

mod octahedral_plan;
pub use octahedral_plan::{
    OctahedralImpostorFrame, OctahedralImpostorPlan, octahedral_impostor_plan,
};

/// Builds evenly spaced capture views and tight bounds for an impostor atlas.
#[must_use]
pub fn impostor_spec(mesh: &PlantMesh, views_around: u32, with_top_view: bool) -> ImpostorSpec {
    let (minimum, maximum) = bounds(mesh);
    let centre = [
        (minimum[0] + maximum[0]) * 0.5,
        (minimum[1] + maximum[1]) * 0.5,
        (minimum[2] + maximum[2]) * 0.5,
    ];
    let radius = mesh
        .vertices
        .iter()
        .map(|vertex| distance(vertex.position, centre))
        .fold(0.0_f32, f32::max);
    let mut views = Vec::new();
    let views_around = views_around.clamp(1, 16);
    for index in 0..views_around {
        let angle = core::f32::consts::TAU * f32::from(u16::try_from(index).unwrap_or(0))
            / f32::from(u16::try_from(views_around).unwrap_or(1));
        let (sine, cosine) = sin_cos(angle);
        views.push(ImpostorView {
            direction: [cosine, 0.0, sine],
            up: [0.0, 1.0, 0.0],
            half_width: ((maximum[0] - minimum[0]).powi(2) + (maximum[2] - minimum[2]).powi(2))
                .sqrt()
                * 0.5,
            half_height: (maximum[1] - minimum[1]) * 0.5,
        });
    }
    if with_top_view {
        views.push(ImpostorView {
            direction: [0.0, -1.0, 0.0],
            up: [0.0, 0.0, 1.0],
            half_width: (maximum[0] - minimum[0]).max(maximum[2] - minimum[2]) * 0.5,
            half_height: (maximum[0] - minimum[0]).max(maximum[2] - minimum[2]) * 0.5,
        });
    }
    let view_count = u32::try_from(views.len()).unwrap_or(u32::MAX);
    let mut columns = 1_u32;
    while columns * columns < view_count {
        columns += 1;
    }
    let rows = view_count.div_ceil(columns);
    ImpostorSpec {
        views,
        bounds_centre: centre,
        bounds_radius: radius,
        atlas_grid: [columns, rows],
    }
}

fn bounds(mesh: &PlantMesh) -> ([f32; 3], [f32; 3]) {
    let mut minimum = [f32::MAX; 3];
    let mut maximum = [f32::MIN; 3];
    for vertex in &mesh.vertices {
        for axis in 0..3 {
            minimum[axis] = minimum[axis].min(vertex.position[axis]);
            maximum[axis] = maximum[axis].max(vertex.position[axis]);
        }
    }
    if mesh.vertices.is_empty() {
        ([0.0; 3], [0.0; 3])
    } else {
        (minimum, maximum)
    }
}

fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

fn sin_cos(angle: f32) -> (f32, f32) {
    let tau = core::f32::consts::TAU;
    let mut x = angle - (angle / tau).floor() * tau;
    if x > core::f32::consts::PI {
        x -= tau;
    }
    let sign = if x.abs() > core::f32::consts::FRAC_PI_2 {
        -1.0
    } else {
        1.0
    };
    if x > core::f32::consts::FRAC_PI_2 {
        x = core::f32::consts::PI - x;
    } else if x < -core::f32::consts::FRAC_PI_2 {
        x = -core::f32::consts::PI - x;
    }
    let x2 = x * x;
    let sine = x
        * (1.0
            + x2 * (-0.166_666_67
                + x2 * (0.008_333_331 + x2 * (-0.000_198_409 + x2 * 0.000_002_752_6))));
    let cosine = sign
        * (1.0
            + x2 * (-0.5 + x2 * (0.041_666_638 + x2 * (-0.001_388_837_8 + x2 * 0.000_024_760_9))));
    (sine, cosine)
}
