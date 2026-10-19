//! Geometric self-intersection check for the faces an operator has just made.
//!
//! Topology validation cannot see a face pushed through another part of the mesh. This check tests
//! each new face against every face it shares no vertex with. Contact is not a crossing: only an edge
//! that passes through the interior of another triangle, or coplanar triangles whose outlines cross,
//! counts, so touching surfaces are never refused. Cost is bounded by a face-pair budget.

use std::collections::{BTreeMap, BTreeSet};

use ve_kernel::ExecutionContext;

use crate::topology::check_cancelled;
use crate::{Element, GeometryError, PolyMesh};

/// Face pairs one check may test before it refuses the work as too large.
pub(crate) const PAIR_BUDGET: usize = 1_000_000;

type Triangle = [[f64; 3]; 3];

/// Refuses with `poly.self_intersection` (naming the new face) when any face in `new_faces` crosses a
/// face it shares no vertex with.
///
/// # Errors
/// A typed refusal for a crossing or an exhausted budget, or a cancellation error.
pub(crate) fn refuse_crossings(
    mesh: &PolyMesh,
    new_faces: &[usize],
    context: &ExecutionContext,
) -> Result<(), GeometryError> {
    let ranges = face_ranges(mesh)?;
    let boxes = face_boxes(mesh, &ranges, context)?;
    let cell = cell_size(&boxes);
    let grid = build_grid(&boxes, cell, context)?;
    let tolerance = f64::from(f32::EPSILON) * 8.0 * extent(&boxes).max(f64::MIN_POSITIVE);
    let mut triangles: BTreeMap<usize, Vec<Triangle>> = BTreeMap::new();
    let mut tested = 0_usize;
    for &face in new_faces {
        check_cancelled(context)?;
        let own: BTreeSet<u32> = mesh.corners()[ranges[face].clone()]
            .iter()
            .copied()
            .collect();
        for other in candidates(&grid, &boxes[face], cell) {
            if other == face || !boxes_overlap(&boxes[face], &boxes[other], tolerance) {
                continue;
            }
            if mesh.corners()[ranges[other].clone()]
                .iter()
                .any(|vertex| own.contains(vertex))
            {
                continue;
            }
            tested += 1;
            if tested > PAIR_BUDGET {
                return Err(refusal("poly.budget_exceeded", face));
            }
            if tested.is_multiple_of(256) {
                check_cancelled(context)?;
            }
            fill(&mut triangles, mesh, &ranges, face);
            fill(&mut triangles, mesh, &ranges, other);
            if triangles[&face].iter().any(|a| {
                triangles[&other]
                    .iter()
                    .any(|b| triangles_cross(a, b, tolerance))
            }) {
                return Err(refusal("poly.self_intersection", face));
            }
        }
    }
    Ok(())
}

fn refusal(code: &'static str, face: usize) -> GeometryError {
    GeometryError::PolyRefusal {
        code,
        element: Some(Element::Face(face)),
    }
}

fn face_ranges(mesh: &PolyMesh) -> Result<Vec<std::ops::Range<usize>>, GeometryError> {
    let mut start = 0_usize;
    mesh.face_sizes()
        .iter()
        .map(|&size| {
            let end = start
                .checked_add(size as usize)
                .ok_or(GeometryError::InvalidMesh("face range overflow"))?;
            let range = start..end;
            start = end;
            Ok(range)
        })
        .collect()
}

fn point(mesh: &PolyMesh, vertex: u32) -> [f64; 3] {
    mesh.positions()[vertex as usize].map(f64::from)
}

fn face_boxes(
    mesh: &PolyMesh,
    ranges: &[std::ops::Range<usize>],
    context: &ExecutionContext,
) -> Result<Vec<([f64; 3], [f64; 3])>, GeometryError> {
    let mut boxes = Vec::with_capacity(ranges.len());
    for (index, range) in ranges.iter().enumerate() {
        if index % 1024 == 0 {
            check_cancelled(context)?;
        }
        let mut low = [f64::INFINITY; 3];
        let mut high = [f64::NEG_INFINITY; 3];
        for &vertex in &mesh.corners()[range.clone()] {
            let p = point(mesh, vertex);
            for axis in 0..3 {
                low[axis] = low[axis].min(p[axis]);
                high[axis] = high[axis].max(p[axis]);
            }
        }
        boxes.push((low, high));
    }
    Ok(boxes)
}

fn extent(boxes: &[([f64; 3], [f64; 3])]) -> f64 {
    let mut low = [f64::INFINITY; 3];
    let mut high = [f64::NEG_INFINITY; 3];
    for (l, h) in boxes {
        for axis in 0..3 {
            low[axis] = low[axis].min(l[axis]);
            high[axis] = high[axis].max(h[axis]);
        }
    }
    (0..3)
        .map(|axis| high[axis] - low[axis])
        .fold(0.0, f64::max)
}

/// Mean longest side of a face box, so a face touches only a few cells.
fn cell_size(boxes: &[([f64; 3], [f64; 3])]) -> f64 {
    let total: f64 = boxes
        .iter()
        .map(|(l, h)| (0..3).map(|axis| h[axis] - l[axis]).fold(0.0, f64::max))
        .sum();
    let count = f64::from(u32::try_from(boxes.len().max(1)).unwrap_or(u32::MAX));
    (total / count)
        .max(extent(boxes) * 1.0e-6)
        .max(f64::MIN_POSITIVE)
}

type Grid = BTreeMap<[i64; 3], Vec<usize>>;

#[expect(
    clippy::cast_possible_truncation,
    reason = "cell indices of f32-range coordinates fit an i64"
)]
fn cell_of(value: f64, size: f64) -> i64 {
    (value / size).floor() as i64
}

fn build_grid(
    boxes: &[([f64; 3], [f64; 3])],
    size: f64,
    context: &ExecutionContext,
) -> Result<Grid, GeometryError> {
    let mut grid = Grid::new();
    let mut references = 0_usize;
    for (face, (low, high)) in boxes.iter().enumerate() {
        if face % 1024 == 0 {
            check_cancelled(context)?;
        }
        let from = low.map(|v| cell_of(v, size));
        let to = high.map(|v| cell_of(v, size));
        for x in from[0]..=to[0] {
            for y in from[1]..=to[1] {
                for z in from[2]..=to[2] {
                    references += 1;
                    if references > PAIR_BUDGET * 4 {
                        return Err(refusal("poly.budget_exceeded", face));
                    }
                    grid.entry([x, y, z]).or_default().push(face);
                }
            }
        }
    }
    Ok(grid)
}

fn candidates(grid: &Grid, boxed: &([f64; 3], [f64; 3]), size: f64) -> BTreeSet<usize> {
    let from = boxed.0.map(|v| cell_of(v, size));
    let to = boxed.1.map(|v| cell_of(v, size));
    let mut found = BTreeSet::new();
    for x in from[0]..=to[0] {
        for y in from[1]..=to[1] {
            for z in from[2]..=to[2] {
                if let Some(faces) = grid.get(&[x, y, z]) {
                    found.extend(faces.iter().copied());
                }
            }
        }
    }
    found
}

fn boxes_overlap(a: &([f64; 3], [f64; 3]), b: &([f64; 3], [f64; 3]), tolerance: f64) -> bool {
    (0..3).all(|axis| a.1[axis] >= b.0[axis] - tolerance && b.1[axis] >= a.0[axis] - tolerance)
}

/// Stores the triangles of a face (a fan; only used to detect crossings) if not already there.
fn fill(
    cache: &mut BTreeMap<usize, Vec<Triangle>>,
    mesh: &PolyMesh,
    ranges: &[std::ops::Range<usize>],
    face: usize,
) {
    cache.entry(face).or_insert_with(|| {
        let corners = &mesh.corners()[ranges[face].clone()];
        (1..corners.len().saturating_sub(1))
            .map(|i| {
                [
                    point(mesh, corners[0]),
                    point(mesh, corners[i]),
                    point(mesh, corners[i + 1]),
                ]
            })
            .collect()
    });
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Whether the interiors of two triangles cross (contact and shared edges do not count).
fn triangles_cross(a: &Triangle, b: &Triangle, tolerance: f64) -> bool {
    let normal_a = cross(sub(a[1], a[0]), sub(a[2], a[0]));
    let normal_b = cross(sub(b[1], b[0]), sub(b[2], b[0]));
    let (length_a, length_b) = (
        dot(normal_a, normal_a).sqrt(),
        dot(normal_b, normal_b).sqrt(),
    );
    if length_a <= tolerance * tolerance || length_b <= tolerance * tolerance {
        return false;
    }
    let side_b: [f64; 3] = std::array::from_fn(|i| dot(sub(b[i], a[0]), normal_a) / length_a);
    let side_a: [f64; 3] = std::array::from_fn(|i| dot(sub(a[i], b[0]), normal_b) / length_b);
    if side_b.iter().all(|d| d.abs() <= tolerance) {
        return coplanar_cross(a, b, normal_a, tolerance);
    }
    edge_pierces(a, &normal_a, length_a, b, &side_b, tolerance)
        || edge_pierces(b, &normal_b, length_b, a, &side_a, tolerance)
}

/// Whether an edge of `edges` passes strictly through the interior of `face`.
fn edge_pierces(
    face: &Triangle,
    normal: &[f64; 3],
    length: f64,
    edges: &Triangle,
    side: &[f64; 3],
    tolerance: f64,
) -> bool {
    for i in 0..3 {
        let j = (i + 1) % 3;
        if !((side[i] > tolerance && side[j] < -tolerance)
            || (side[i] < -tolerance && side[j] > tolerance))
        {
            continue;
        }
        let t = side[i] / (side[i] - side[j]);
        let hit: [f64; 3] =
            std::array::from_fn(|axis| edges[i][axis] + t * (edges[j][axis] - edges[i][axis]));
        if strictly_inside(face, normal, length, hit, tolerance) {
            return true;
        }
    }
    false
}

fn strictly_inside(face: &Triangle, normal: &[f64; 3], length: f64, p: [f64; 3], tol: f64) -> bool {
    (0..3).all(|i| {
        let edge = sub(face[(i + 1) % 3], face[i]);
        dot(cross(edge, sub(p, face[i])), *normal) / length > tol * length.sqrt().max(1.0e-12)
    })
}

fn coplanar_cross(a: &Triangle, b: &Triangle, normal: [f64; 3], tolerance: f64) -> bool {
    let axis = (0..3)
        .max_by(|&x, &y| normal[x].abs().total_cmp(&normal[y].abs()))
        .unwrap_or(2);
    let flat = |t: &Triangle| -> [[f64; 2]; 3] {
        t.map(|p| match axis {
            0 => [p[1], p[2]],
            1 => [p[0], p[2]],
            _ => [p[0], p[1]],
        })
    };
    let (pa, pb) = (flat(a), flat(b));
    for i in 0..3 {
        for j in 0..3 {
            if segments_cross(pa[i], pa[(i + 1) % 3], pb[j], pb[(j + 1) % 3], tolerance) {
                return true;
            }
        }
    }
    contains(&pa, centroid(&pb), tolerance) || contains(&pb, centroid(&pa), tolerance)
}

fn centroid(t: &[[f64; 2]; 3]) -> [f64; 2] {
    [
        (t[0][0] + t[1][0] + t[2][0]) / 3.0,
        (t[0][1] + t[1][1] + t[2][1]) / 3.0,
    ]
}

fn orient(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

fn contains(t: &[[f64; 2]; 3], p: [f64; 2], tol: f64) -> bool {
    let signs = [
        orient(t[0], t[1], p),
        orient(t[1], t[2], p),
        orient(t[2], t[0], p),
    ];
    signs.iter().all(|s| *s > tol) || signs.iter().all(|s| *s < -tol)
}

fn segments_cross(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2], tol: f64) -> bool {
    let (o1, o2) = (orient(a, b, c), orient(a, b, d));
    let (o3, o4) = (orient(c, d, a), orient(c, d, b));
    ((o1 > tol && o2 < -tol) || (o1 < -tol && o2 > tol))
        && ((o3 > tol && o4 < -tol) || (o3 < -tol && o4 > tol))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tri(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> Triangle {
        [a, b, c]
    }

    #[test]
    fn a_piercing_edge_crosses_and_a_touching_corner_does_not() {
        let floor = tri([0.0, 0.0, 0.0], [4.0, 0.0, 0.0], [0.0, 0.0, 4.0]);
        let through = tri([1.0, -1.0, 1.0], [1.0, 1.0, 1.0], [1.5, 1.0, 1.5]);
        assert!(triangles_cross(&floor, &through, 1.0e-9));
        let touching = tri([1.0, 0.0, 1.0], [1.0, 1.0, 1.0], [1.5, 1.0, 1.5]);
        assert!(!triangles_cross(&floor, &touching, 1.0e-9));
        let above = tri([1.0, 0.5, 1.0], [1.0, 1.0, 1.0], [1.5, 1.0, 1.5]);
        assert!(!triangles_cross(&floor, &above, 1.0e-9));
    }

    #[test]
    fn coplanar_overlap_crosses_and_disjoint_does_not() {
        let a = tri([0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 0.0, 2.0]);
        let overlapping = tri([0.5, 0.0, 0.5], [3.0, 0.0, 0.5], [0.5, 0.0, 3.0]);
        assert!(triangles_cross(&a, &overlapping, 1.0e-9));
        let apart = tri([5.0, 0.0, 5.0], [6.0, 0.0, 5.0], [5.0, 0.0, 6.0]);
        assert!(!triangles_cross(&a, &apart, 1.0e-9));
    }
}
