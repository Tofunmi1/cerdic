//! `inst.scatter_on`: seeded placements over the surface of a solid, oriented to its normals.
//!
//! Points are chosen uniformly by area: a triangle is picked with probability proportional to its
//! area, then a uniform barycentric point is drawn inside it. `min_distance` is kept by dart
//! throwing, so when the requested count cannot be placed the node refuses instead of returning
//! fewer. All randomness comes from the same `SplitMix64` stream `inst.scatter` uses, so one seed
//! gives the same placements on every platform and on re-evaluation.
use super::scatter::{Stream, seed};
use crate::{
    Fail, fail, fail_suggest,
    profile_ops::{number, whole},
};
use glam::{DMat4, DQuat, DVec3};
use ve_graph::{Node, Value as Param};
use ve_kernel::Mesh;

/// The most placements one `inst.scatter_on` may produce.
pub(crate) const MAX_SCATTER_ON: u32 = 5_000;
/// Random draws allowed per requested placement before the spacing is declared unsatisfiable.
const ATTEMPTS_PER_POINT: u32 = 32;
/// How far from `+/- 0.5` a face normal's Y must be to count as up or down.
const FLAT: f64 = 0.5;

/// A surface triangle with its outward normal and area.
struct Triangle {
    corners: [DVec3; 3],
    normal: DVec3,
    area: f64,
}

impl Triangle {
    fn new(mesh: &Mesh, corners: [u32; 3]) -> Self {
        let corners = corners.map(|k| DVec3::from_array(mesh.positions[k as usize].map(f64::from)));
        let cross = (corners[1] - corners[0]).cross(corners[2] - corners[0]);
        Self {
            corners,
            normal: cross.normalize_or_zero(),
            area: 0.5 * cross.length(),
        }
    }
}

/// The triangles of a surface mesh, ready for area-weighted sampling.
struct Surface {
    triangles: Vec<Triangle>,
    total: f64,
}

impl Surface {
    fn new(mesh: &Mesh) -> Self {
        let triangles: Vec<Triangle> = mesh
            .indices
            .chunks_exact(3)
            .map(|t| Triangle::new(mesh, [t[0], t[1], t[2]]))
            .filter(|triangle| triangle.area > 0.0)
            .collect();
        let total = triangles.iter().map(|triangle| triangle.area).sum();
        Self { triangles, total }
    }

    /// A point and outward normal drawn uniformly by area, or `None` for an empty surface.
    fn sample(&self, stream: &mut Stream) -> Option<(DVec3, DVec3)> {
        if self.total <= 0.0 {
            return None;
        }
        let target = stream.unit() * self.total;
        let mut accumulated = 0.0;
        let mut chosen = self.triangles.last()?;
        for triangle in &self.triangles {
            accumulated += triangle.area;
            if accumulated >= target {
                chosen = triangle;
                break;
            }
        }
        // Uniform barycentric coordinates: `sqrt(u)` spreads the second weight over the triangle.
        let spread = stream.unit().sqrt();
        let last = stream.unit();
        let weights = [1.0 - spread, spread * (1.0 - last), spread * last];
        let point = chosen.corners[0] * weights[0]
            + chosen.corners[1] * weights[1]
            + chosen.corners[2] * weights[2];
        Some((point, chosen.normal))
    }
}

fn in_region(region: &str, normal: DVec3) -> bool {
    match region {
        "up" => normal.y > FLAT,
        "down" => normal.y < -FLAT,
        "side" => normal.y.abs() <= FLAT,
        _ => true,
    }
}

/// The local orientation for one placement: align to the normal, spin about it and lean away.
fn orientation(stream: &mut Stream, normal: DVec3, align: bool, spin: bool, tilt: f64) -> DMat4 {
    let turn = if spin {
        DMat4::from_rotation_y(stream.unit() * std::f64::consts::TAU)
    } else {
        DMat4::IDENTITY
    };
    let lean = if tilt > 0.0 {
        let angle = stream.unit() * tilt;
        let direction = stream.unit() * std::f64::consts::TAU;
        let axis = DVec3::new(direction.cos(), 0.0, direction.sin());
        DMat4::from_quat(DQuat::from_axis_angle(axis, angle))
    } else {
        DMat4::IDENTITY
    };
    let local = turn * lean;
    if align {
        DMat4::from_quat(DQuat::from_rotation_arc(DVec3::Y, normal)) * local
    } else {
        local
    }
}

fn capped(count: u32) -> Fail {
    fail_suggest(
        "inst.unsatisfiable",
        format!("scatter_on count must be 1..={MAX_SCATTER_ON}, got {count}"),
        "lower `count`",
    )
}

fn unsatisfied(placed: usize, count: u32, min_distance: f64) -> Fail {
    fail_suggest(
        "inst.unsatisfiable",
        format!("placed {placed} of {count} points with min_distance {min_distance}"),
        "lower `count` or `min_distance`, or enlarge the surface",
    )
}

/// Seeded placements over the triangles of `surface`, oriented to it, one matrix per placement.
pub(crate) fn scatter_on(node: &Node, surface: &Mesh) -> Result<Vec<DMat4>, Fail> {
    let count = whole(node, "count").map_err(|_| fail("graph.invalid_param", "count"))?;
    if count == 0 || count > MAX_SCATTER_ON {
        return Err(capped(count));
    }
    let measure = |name| number(node, name).map_err(|_| fail("graph.invalid_param", name));
    let min_distance = measure("min_distance")?;
    let tilt = measure("tilt")?;
    let scale_jitter = measure("scale_jitter")?;
    let align = matches!(node.params.get("align"), Some(Param::Bool(true)));
    let spin = matches!(node.params.get("spin"), Some(Param::Bool(true)));
    let region = match node.params.get("region") {
        Some(Param::Str(name)) => name.as_str(),
        _ => "all",
    };
    let surface = Surface::new(surface);
    if surface.triangles.is_empty() {
        return Err(fail(
            "graph.missing_input",
            "inst.scatter_on needs a surface that is not empty",
        ));
    }
    let mut stream = Stream::new(seed(node)?, 7);
    let mut points: Vec<DVec3> = Vec::new();
    let mut transforms = Vec::new();
    let budget = count.saturating_mul(ATTEMPTS_PER_POINT).saturating_add(64);
    let mut attempts = 0;
    while points.len() < count as usize {
        attempts += 1;
        if attempts > budget {
            return Err(unsatisfied(points.len(), count, min_distance));
        }
        let Some((point, normal)) = surface.sample(&mut stream) else {
            return Err(fail(
                "graph.missing_input",
                "inst.scatter_on needs a surface that is not empty",
            ));
        };
        if !in_region(region, normal) {
            continue;
        }
        if min_distance > 0.0
            && points
                .iter()
                .any(|placed| placed.distance(point) < min_distance)
        {
            continue;
        }
        let factor = 1.0 + stream.signed() * scale_jitter;
        let matrix = DMat4::from_translation(point)
            * orientation(&mut stream, normal, align, spin, tilt)
            * DMat4::from_scale(DVec3::splat(factor));
        points.push(point);
        transforms.push(matrix);
    }
    Ok(transforms)
}
