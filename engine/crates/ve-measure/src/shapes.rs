//! Bounding shape fits: an axis-aligned box, an oriented box and a capsule.

use core::f32::consts::PI;

use glam::Vec3;

use crate::error::MeasureError;
use crate::mesh::Mesh;

/// Which kind of bounding shape this is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeKind {
    /// An axis-aligned box.
    Aabb,
    /// An oriented box from the point cloud's principal axes.
    Obb,
    /// A capsule along the principal axis.
    Capsule,
}

/// One fitted bounding shape and how well it fits.
///
/// The relevant fields depend on `kind`: a box uses `half_extents` and `axes`; a capsule uses
/// `radius`, `height` and `axes[0]`. `ratio` is `volume / mesh_volume`, so the best fit is the one
/// closest to `1`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoundingShape {
    /// Which shape this is.
    pub kind: ShapeKind,
    /// The shape's centre, in metres.
    pub centre: [f32; 3],
    /// Half the box side lengths, in metres (boxes only).
    pub half_extents: [f32; 3],
    /// The box's or capsule's axes; `axes[0]` is the capsule direction.
    pub axes: [[f32; 3]; 3],
    /// The capsule radius, in metres (capsules only).
    pub radius: f32,
    /// The capsule axis length, in metres (capsules only).
    pub height: f32,
    /// The shape's volume, in cubic metres.
    pub volume: f32,
    /// `volume / mesh_volume`.
    pub ratio: f32,
}

/// The three fitted shapes and which fits best.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoundingShapes {
    /// The axis-aligned box.
    pub aabb: BoundingShape,
    /// The oriented box.
    pub obb: BoundingShape,
    /// The capsule.
    pub capsule: BoundingShape,
    /// The kind with the smallest volume ratio.
    pub best: ShapeKind,
}

/// Fit an axis-aligned box, an oriented box (PCA) and a capsule, and say which fits best.
///
/// # Errors
/// Returns [`MeasureError`] when the mesh is malformed or empty.
pub fn bounding_shapes(mesh: &Mesh) -> Result<BoundingShapes, MeasureError> {
    mesh.validate()?;
    let points: Vec<Vec3> = mesh
        .positions
        .iter()
        .map(|p| Vec3::from_array(*p))
        .collect();
    let reference = mesh.signed_volume().abs();
    let aabb = aabb_shape(&points, reference);
    let obb = obb_shape(&points, reference);
    let capsule = capsule_shape(&points, reference);
    let best = [aabb, obb, capsule]
        .into_iter()
        .min_by(|a, b| a.ratio.total_cmp(&b.ratio))
        .map_or(ShapeKind::Aabb, |s| s.kind);
    Ok(BoundingShapes {
        aabb,
        obb,
        capsule,
        best,
    })
}

fn ratio_of(volume: f32, reference: f32) -> f32 {
    if reference > 1e-12 {
        volume / reference
    } else {
        f32::INFINITY
    }
}

fn aabb_shape(points: &[Vec3], reference: f32) -> BoundingShape {
    let mut min = points[0];
    let mut max = points[0];
    for p in points {
        min = min.min(*p);
        max = max.max(*p);
    }
    let half = (max - min) * 0.5;
    let volume = 8.0 * half.x * half.y * half.z;
    BoundingShape {
        kind: ShapeKind::Aabb,
        centre: ((min + max) * 0.5).to_array(),
        half_extents: half.to_array(),
        axes: identity_axes(),
        radius: 0.0,
        height: 0.0,
        volume,
        ratio: ratio_of(volume, reference),
    }
}

fn obb_shape(points: &[Vec3], reference: f32) -> BoundingShape {
    let (centre, axes) = principal_axes(points);
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for p in points {
        let d = *p - centre;
        for (k, axis) in axes.iter().enumerate() {
            let v = d.dot(*axis);
            min[k] = min[k].min(v);
            max[k] = max[k].max(v);
        }
    }
    let half = (max - min) * 0.5;
    let offset = (max + min) * 0.5;
    let mut centre = centre;
    for (k, axis) in axes.iter().enumerate() {
        centre += *axis * offset[k];
    }
    let volume = 8.0 * half.x * half.y * half.z;
    BoundingShape {
        kind: ShapeKind::Obb,
        centre: centre.to_array(),
        half_extents: half.to_array(),
        axes: axes.map(|a| a.to_array()),
        radius: 0.0,
        height: 0.0,
        volume,
        ratio: ratio_of(volume, reference),
    }
}

fn capsule_shape(points: &[Vec3], reference: f32) -> BoundingShape {
    let (centre, axes) = principal_axes(points);
    let direction = axes[0];
    let mut lo = f32::INFINITY;
    let mut hi = f32::NEG_INFINITY;
    for p in points {
        let t = (*p - centre).dot(direction);
        lo = lo.min(t);
        hi = hi.max(t);
    }
    let a = centre + direction * lo;
    let b = centre + direction * hi;
    let mut radius = 0.0f32;
    for p in points {
        radius = radius.max(point_segment_distance(*p, a, b));
    }
    let height = hi - lo;
    let volume = PI * radius * radius * height + 4.0 / 3.0 * PI * radius * radius * radius;
    BoundingShape {
        kind: ShapeKind::Capsule,
        centre: ((a + b) * 0.5).to_array(),
        half_extents: [0.0, height * 0.5, 0.0],
        axes: axes.map(|a| a.to_array()),
        radius,
        height,
        volume,
        ratio: ratio_of(volume, reference),
    }
}

fn point_segment_distance(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let ab = b - a;
    let length_squared = ab.length_squared();
    if length_squared <= 1e-20 {
        return (p - a).length();
    }
    let t = ((p - a).dot(ab) / length_squared).clamp(0.0, 1.0);
    (p - (a + ab * t)).length()
}

fn identity_axes() -> [[f32; 3]; 3] {
    [Vec3::X.to_array(), Vec3::Y.to_array(), Vec3::Z.to_array()]
}

/// The centroid and the three principal axes (descending eigenvalue) of a point cloud.
fn principal_axes(points: &[Vec3]) -> (Vec3, [Vec3; 3]) {
    let count = points.len() as f32;
    let mut mean = Vec3::ZERO;
    for p in points {
        mean += *p;
    }
    mean /= count;
    let mut cov = [[0.0f32; 3]; 3];
    for p in points {
        let d = (*p - mean).to_array();
        for i in 0..3 {
            for j in 0..3 {
                cov[i][j] += d[i] * d[j];
            }
        }
    }
    for row in &mut cov {
        for value in row {
            *value /= count;
        }
    }
    let vectors = jacobi(cov);
    let mut pairs: Vec<(f32, Vec3)> = (0..3)
        .map(|k| {
            (
                cov[k][k],
                Vec3::new(vectors[0][k], vectors[1][k], vectors[2][k]).normalize_or_zero(),
            )
        })
        .collect();
    pairs.sort_by(|a, b| b.0.total_cmp(&a.0));
    (mean, [pairs[0].1, pairs[1].1, pairs[2].1])
}

/// Jacobi eigen-decomposition of a symmetric 3×3 matrix; returns the eigenvector columns.
#[expect(
    clippy::needless_range_loop,
    reason = "the sweep updates row k and the pivot rows p and q at once, which iter_mut cannot borrow"
)]
fn jacobi(mut a: [[f32; 3]; 3]) -> [[f32; 3]; 3] {
    let mut v = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    for _ in 0..32 {
        let (mut p, mut q) = (0, 1);
        let mut max = a[0][1].abs();
        if a[0][2].abs() > max {
            max = a[0][2].abs();
            p = 0;
            q = 2;
        }
        if a[1][2].abs() > max {
            max = a[1][2].abs();
            p = 1;
            q = 2;
        }
        if max < 1e-12 {
            break;
        }
        let theta = 0.5 * (a[q][q] - a[p][p]) / a[p][q];
        let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
        let c = 1.0 / (t * t + 1.0).sqrt();
        let s = t * c;
        let (app, aqq, apq) = (a[p][p], a[q][q], a[p][q]);
        a[p][p] = c * c * app - 2.0 * s * c * apq + s * s * aqq;
        a[q][q] = s * s * app + 2.0 * s * c * apq + c * c * aqq;
        a[p][q] = 0.0;
        a[q][p] = 0.0;
        for k in 0..3 {
            if k != p && k != q {
                let (akp, akq) = (a[k][p], a[k][q]);
                a[k][p] = c * akp - s * akq;
                a[p][k] = a[k][p];
                a[k][q] = s * akp + c * akq;
                a[q][k] = a[k][q];
            }
        }
        for row in &mut v {
            let (vkp, vkq) = (row[p], row[q]);
            row[p] = c * vkp - s * vkq;
            row[q] = s * vkp + c * vkq;
        }
    }
    v
}
