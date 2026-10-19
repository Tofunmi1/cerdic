//! Deterministic ear clipping for simple faces within a bounded planar deviation.

use ve_kernel::ExecutionContext;

use crate::{GeometryError, PolyMesh};

const MAX_PLANAR_DEVIATION_RATIO: f64 = 1.0e-3;

/// Policy for deriving triangles from polygon faces.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TriangulationMode {
    /// Only faces within the planar-deviation tolerance may be triangulated.
    #[default]
    PlanarOnly,
    /// Allow mildly curved quads, splitting along their shorter diagonal. The two output
    /// triangle normals must have a positive dot product, which rejects folds over 90 degrees.
    CurvedQuads,
}

pub(crate) fn face(
    mesh: &PolyMesh,
    vertices: &[u32],
    mode: TriangulationMode,
    context: &ExecutionContext,
) -> Result<Vec<[u32; 3]>, GeometryError> {
    if vertices.len() < 3 {
        return Err(GeometryError::TriangulationUnsupported(
            "fewer than three corners",
        ));
    }
    let mut points = Vec::with_capacity(vertices.len());
    for (index, &vertex) in vertices.iter().enumerate() {
        if index % 256 == 0 && context.is_cancelled() {
            return Err(GeometryError::Cancelled);
        }
        points.push(point(mesh, vertex)?);
    }
    let normal = newell(&points);
    let magnitude = length(normal);
    let extent = bounds_extent(&points);
    let tolerance = (f64::from(f32::EPSILON) * extent.max(max_coordinate(&points)))
        .max(MAX_PLANAR_DEVIATION_RATIO * extent);
    let area_tolerance = tolerance * extent;
    if magnitude <= 2.0 * tolerance * extent {
        return Err(GeometryError::TriangulationUnsupported("zero-area polygon"));
    }
    let unit = normal.map(|value| value / magnitude);
    let planar = points
        .iter()
        .all(|point| dot(sub(*point, points[0]), unit).abs() <= tolerance);
    if !planar && mode == TriangulationMode::CurvedQuads && vertices.len() == 4 {
        return curved_quad(vertices, &points, normal);
    }
    if !planar {
        return Err(GeometryError::TriangulationUnsupported(
            "non-planar polygon",
        ));
    }
    let axis = dominant_axis(normal);
    let projected: Vec<[f64; 2]> = points.iter().map(|point| project(*point, axis)).collect();
    reject_self_intersection(&projected, area_tolerance, context)?;
    ear_clip(vertices, &projected, area_tolerance, context)
}

/// Whether a simple planar face bends consistently in its dominant-axis projection.
pub(crate) fn is_convex_face(
    mesh: &PolyMesh,
    vertices: &[u32],
    context: &ExecutionContext,
) -> Result<bool, GeometryError> {
    if vertices.len() <= 3 {
        return Ok(true);
    }
    let mut points = Vec::with_capacity(vertices.len());
    for (index, &vertex) in vertices.iter().enumerate() {
        if index % 256 == 0 && context.is_cancelled() {
            return Err(GeometryError::Cancelled);
        }
        points.push(point(mesh, vertex)?);
    }
    let axis = dominant_axis(newell(&points));
    let projected: Vec<[f64; 2]> = points.iter().map(|point| project(*point, axis)).collect();
    let extent = bounds_extent(&points).max(1.0);
    let tolerance = extent * extent * 1.0e-12;
    let mut turn = 0.0_f64;
    for index in 0..projected.len() {
        let value = cross(
            projected[index],
            projected[(index + 1) % projected.len()],
            projected[(index + 2) % projected.len()],
        );
        if value.abs() <= tolerance {
            continue;
        }
        if turn.abs() > tolerance && value.signum() != turn.signum() {
            return Ok(false);
        }
        turn = value;
    }
    Ok(true)
}

fn curved_quad(
    vertices: &[u32],
    points: &[[f64; 3]],
    reference_normal: [f64; 3],
) -> Result<Vec<[u32; 3]>, GeometryError> {
    let ac = length(sub(points[0], points[2]));
    let bd = length(sub(points[1], points[3]));
    let diagonal_ac = (vertices[0].min(vertices[2]), vertices[0].max(vertices[2]));
    let diagonal_bd = (vertices[1].min(vertices[3]), vertices[1].max(vertices[3]));
    let use_ac = match ac.total_cmp(&bd) {
        std::cmp::Ordering::Less => true,
        std::cmp::Ordering::Equal => diagonal_ac <= diagonal_bd,
        std::cmp::Ordering::Greater => false,
    };
    let local_triangles = if use_ac {
        [[0, 1, 2], [0, 2, 3]]
    } else {
        [[0, 1, 3], [1, 2, 3]]
    };
    let triangles = local_triangles.map(|triangle| triangle.map(|index| vertices[index]));
    let mut normals = Vec::with_capacity(2);
    for indices in local_triangles {
        let [a, b, c] = indices.map(|index| points[index]);
        let normal = cross3(sub(b, a), sub(c, a));
        if dot(normal, reference_normal) <= 0.0 {
            return Err(GeometryError::TriangulationUnsupported(
                "folded curved quad",
            ));
        }
        normals.push(normal);
    }
    if dot(normals[0], normals[1]) <= 0.0 {
        return Err(GeometryError::TriangulationUnsupported(
            "folded curved quad",
        ));
    }
    Ok(triangles.to_vec())
}

fn cross3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Checks whether a polygon loop has a simple projection along its dominant Newell axis.
/// Non-planarity is allowed; this is used by topology edits that do not triangulate yet.
pub(crate) fn validate_simple_loop(
    mesh: &PolyMesh,
    vertices: &[u32],
    context: &ExecutionContext,
) -> Result<(), GeometryError> {
    if vertices.len() < 3 {
        return Err(GeometryError::TriangulationUnsupported(
            "fewer than three corners",
        ));
    }
    let mut points = Vec::with_capacity(vertices.len());
    for (index, &vertex) in vertices.iter().enumerate() {
        if index % 256 == 0 && context.is_cancelled() {
            return Err(GeometryError::Cancelled);
        }
        points.push(point(mesh, vertex)?);
    }
    let normal = newell(&points);
    if length(normal) == 0.0 {
        return Err(GeometryError::TriangulationUnsupported("zero-area polygon"));
    }
    let axis = dominant_axis(normal);
    let projected: Vec<[f64; 2]> = points.iter().map(|point| project(*point, axis)).collect();
    let extent = bounds_extent(&points);
    reject_self_intersection(
        &projected,
        (f64::from(f32::EPSILON) * extent.max(max_coordinate(&points))).max(1.0e-12),
        context,
    )
}

fn point(mesh: &PolyMesh, index: u32) -> Result<[f64; 3], GeometryError> {
    let index = usize::try_from(index)
        .map_err(|_| GeometryError::InvalidMesh("vertex index overflows usize"))?;
    let point = mesh
        .positions()
        .get(index)
        .ok_or(GeometryError::InvalidMesh(
            "corner refers to missing vertex",
        ))?;
    Ok([
        f64::from(point[0]),
        f64::from(point[1]),
        f64::from(point[2]),
    ])
}

fn newell(points: &[[f64; 3]]) -> [f64; 3] {
    let mut normal = [0.0; 3];
    for index in 0..points.len() {
        let a = points[index];
        let b = points[(index + 1) % points.len()];
        normal[0] += (a[1] - b[1]) * (a[2] + b[2]);
        normal[1] += (a[2] - b[2]) * (a[0] + b[0]);
        normal[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    normal
}

fn dominant_axis(normal: [f64; 3]) -> usize {
    (0..3)
        .max_by(|&a, &b| normal[a].abs().total_cmp(&normal[b].abs()))
        .unwrap_or(2)
}

fn project(point: [f64; 3], axis: usize) -> [f64; 2] {
    match axis {
        0 => [point[1], point[2]],
        1 => [point[0], point[2]],
        _ => [point[0], point[1]],
    }
}

fn reject_self_intersection(
    points: &[[f64; 2]],
    tolerance: f64,
    context: &ExecutionContext,
) -> Result<(), GeometryError> {
    for first in 0..points.len() {
        if first % 64 == 0 && context.is_cancelled() {
            return Err(GeometryError::Cancelled);
        }
        let first_next = (first + 1) % points.len();
        for second in first + 1..points.len() {
            let second_next = (second + 1) % points.len();
            if first == second || first_next == second || second_next == first {
                continue;
            }
            if segments_intersect(
                points[first],
                points[first_next],
                points[second],
                points[second_next],
                tolerance,
            ) {
                return Err(GeometryError::TriangulationUnsupported(
                    "self-intersecting polygon",
                ));
            }
        }
    }
    Ok(())
}

fn ear_clip(
    vertices: &[u32],
    points: &[[f64; 2]],
    tolerance: f64,
    context: &ExecutionContext,
) -> Result<Vec<[u32; 3]>, GeometryError> {
    let signed_area = points
        .iter()
        .enumerate()
        .map(|(index, a)| {
            let b = points[(index + 1) % points.len()];
            a[0] * b[1] - b[0] * a[1]
        })
        .sum::<f64>();
    if signed_area.abs() <= tolerance * tolerance {
        return Err(GeometryError::TriangulationUnsupported(
            "zero-area projection",
        ));
    }
    let orientation = signed_area.signum();
    let mut remaining: Vec<usize> = (0..vertices.len()).collect();
    let mut result = Vec::with_capacity(vertices.len() - 2);
    while remaining.len() > 3 {
        if context.is_cancelled() {
            return Err(GeometryError::Cancelled);
        }
        let mut clipped = false;
        for cursor in 0..remaining.len() {
            let previous = remaining[(cursor + remaining.len() - 1) % remaining.len()];
            let current = remaining[cursor];
            let following = remaining[(cursor + 1) % remaining.len()];
            if cross(points[previous], points[current], points[following]) * orientation
                <= tolerance
            {
                continue;
            }
            if remaining
                .iter()
                .copied()
                .filter(|&candidate| {
                    candidate != previous && candidate != current && candidate != following
                })
                .any(|candidate| {
                    inside_triangle(
                        points[candidate],
                        points[previous],
                        points[current],
                        points[following],
                        orientation,
                        tolerance,
                    )
                })
            {
                continue;
            }
            result.push([vertices[previous], vertices[current], vertices[following]]);
            remaining.remove(cursor);
            clipped = true;
            break;
        }
        if !clipped {
            return Err(GeometryError::TriangulationUnsupported(
                "polygon cannot be ear-clipped",
            ));
        }
    }
    result.push([
        vertices[remaining[0]],
        vertices[remaining[1]],
        vertices[remaining[2]],
    ]);
    Ok(result)
}

fn inside_triangle(
    point: [f64; 2],
    a: [f64; 2],
    b: [f64; 2],
    c: [f64; 2],
    orientation: f64,
    tolerance: f64,
) -> bool {
    cross(a, b, point) * orientation >= -tolerance
        && cross(b, c, point) * orientation >= -tolerance
        && cross(c, a, point) * orientation >= -tolerance
}

fn segments_intersect(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2], tolerance: f64) -> bool {
    let ab_c = cross(a, b, c);
    let ab_d = cross(a, b, d);
    let cd_a = cross(c, d, a);
    let cd_b = cross(c, d, b);
    if ((ab_c > tolerance && ab_d < -tolerance) || (ab_c < -tolerance && ab_d > tolerance))
        && ((cd_a > tolerance && cd_b < -tolerance) || (cd_a < -tolerance && cd_b > tolerance))
    {
        return true;
    }
    (ab_c.abs() <= tolerance && on_segment(a, b, c, tolerance))
        || (ab_d.abs() <= tolerance && on_segment(a, b, d, tolerance))
        || (cd_a.abs() <= tolerance && on_segment(c, d, a, tolerance))
        || (cd_b.abs() <= tolerance && on_segment(c, d, b, tolerance))
}

fn on_segment(a: [f64; 2], b: [f64; 2], point: [f64; 2], tolerance: f64) -> bool {
    point[0] >= a[0].min(b[0]) - tolerance
        && point[0] <= a[0].max(b[0]) + tolerance
        && point[1] >= a[1].min(b[1]) - tolerance
        && point[1] <= a[1].max(b[1]) + tolerance
}

fn cross(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn length(vector: [f64; 3]) -> f64 {
    dot(vector, vector).sqrt()
}
fn bounds_extent(points: &[[f64; 3]]) -> f64 {
    (0..3)
        .map(|axis| {
            points
                .iter()
                .map(|point| point[axis])
                .fold(f64::NEG_INFINITY, f64::max)
                - points
                    .iter()
                    .map(|point| point[axis])
                    .fold(f64::INFINITY, f64::min)
        })
        .fold(0.0, f64::max)
}
fn max_coordinate(points: &[[f64; 3]]) -> f64 {
    points
        .iter()
        .flatten()
        .fold(0.0_f64, |maximum, value| maximum.max(value.abs()))
}
