//! Merge connected coplanar faces back into single faces (a solid's triangles into its flat faces).

use std::collections::{BTreeSet, VecDeque};

use ve_kernel::ExecutionContext;

use crate::topology::check_cancelled;
use crate::{GeometryError, PolyMesh, Topology, dissolve_faces};

/// The most patches one call merges; each merge re-validates the mesh.
pub const MAX_MERGES: usize = 1024;

/// Merges each connected patch of faces whose normals are within `angle` radians of the patch's first
/// face and whose planes coincide, using [`dissolve_faces`] so every merge is validated. A patch
/// that cannot become one simple loop (a hole, a pinch) refuses with that operator's code.
///
/// # Errors
/// A typed refusal, `poly.budget_exceeded` when more than [`MAX_MERGES`] patches would be merged,
/// a structural error or cancellation. The input is never changed.
pub fn merge_coplanar(
    mesh: &PolyMesh,
    angle: f64,
    context: &ExecutionContext,
) -> Result<PolyMesh, GeometryError> {
    if !angle.is_finite() || !(0.0..=0.5).contains(&angle) {
        return Err(GeometryError::PolyRefusal {
            code: "poly.angle_invalid",
            element: None,
        });
    }
    let mut current = mesh.clone();
    for _ in 0..=MAX_MERGES {
        check_cancelled(context)?;
        let Some(patch) = first_patch(&current, angle, context)? else {
            return Ok(current);
        };
        current = dissolve_faces(&current, &patch, context)?;
    }
    Err(GeometryError::PolyRefusal {
        code: "poly.budget_exceeded",
        element: None,
    })
}

/// Unit normal and centre of every face (a zero normal marks a degenerate face).
fn planes(mesh: &PolyMesh) -> Vec<([f64; 3], [f64; 3])> {
    let mut start = 0_usize;
    mesh.face_sizes()
        .iter()
        .map(|&size| {
            let corners = &mesh.corners()[start..start + size as usize];
            start += size as usize;
            let points: Vec<[f64; 3]> = corners
                .iter()
                .map(|&v| mesh.positions()[v as usize].map(f64::from))
                .collect();
            let mut sum = [0.0; 3];
            let mut centre = [0.0; 3];
            for (i, a) in points.iter().enumerate() {
                let b = points[(i + 1) % points.len()];
                sum[0] += (a[1] - b[1]) * (a[2] + b[2]);
                sum[1] += (a[2] - b[2]) * (a[0] + b[0]);
                sum[2] += (a[0] - b[0]) * (a[1] + b[1]);
                for (total, value) in centre.iter_mut().zip(a) {
                    *total += value;
                }
            }
            let length = dot(sum, sum).sqrt();
            let normal = if length > 1.0e-18 {
                sum.map(|v| v / length)
            } else {
                [0.0; 3]
            };
            let n = f64::from(u32::try_from(points.len().max(1)).unwrap_or(1));
            (normal, centre.map(|v| v / n))
        })
        .collect()
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The first connected patch of two or more coplanar faces, or `None` when there is none.
fn first_patch(
    mesh: &PolyMesh,
    angle: f64,
    context: &ExecutionContext,
) -> Result<Option<Vec<usize>>, GeometryError> {
    let topology = Topology::build(mesh, context)?;
    let planes = planes(mesh);
    let extent = mesh
        .positions()
        .iter()
        .flatten()
        .fold(0.0_f64, |m, v| m.max(f64::from(v.abs())));
    let distance = f64::from(f32::EPSILON) * 64.0 * extent.max(1.0e-3);
    let cosine = angle.cos();
    for seed in 0..planes.len() {
        check_cancelled(context)?;
        if dot(planes[seed].0, planes[seed].0) < 0.5 {
            continue;
        }
        let mut patch = BTreeSet::from([seed]);
        let mut queue = VecDeque::from([seed]);
        while let Some(face) = queue.pop_front() {
            for &next in &topology.face_neighbors()[face] {
                let coplanar = dot(planes[next].0, planes[seed].0) >= cosine
                    && dot(planes[seed].0, planes[next].0) > 0.5
                    && dot(
                        planes[seed].0,
                        [
                            planes[next].1[0] - planes[seed].1[0],
                            planes[next].1[1] - planes[seed].1[1],
                            planes[next].1[2] - planes[seed].1[2],
                        ],
                    )
                    .abs()
                        <= distance;
                if coplanar && patch.insert(next) {
                    queue.push_back(next);
                }
            }
        }
        if patch.len() > 1 {
            return Ok(Some(patch.into_iter().collect()));
        }
    }
    Ok(None)
}
