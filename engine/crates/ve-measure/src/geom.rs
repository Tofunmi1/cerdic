//! Ray and point primitives shared by the hierarchy's queries.

use glam::Vec3;

use crate::mesh::Mesh;

/// Where a ray met the mesh.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayHit {
    /// Distance along the (unit) ray direction, in metres.
    pub distance: f32,
    /// Which triangle was hit.
    pub triangle: u32,
    /// The point of intersection, in metres.
    pub point: [f32; 3],
    /// The hit triangle's normal (not normalised).
    pub normal: [f32; 3],
}

/// The nearest point on the mesh to a query point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClosestPoint {
    /// Distance from the query point, in metres.
    pub distance: f32,
    /// Which triangle the nearest point lies on.
    pub triangle: u32,
    /// The nearest point itself, in metres.
    pub point: [f32; 3],
}

/// True when the ray can still reach the node before distance `max_t`.
pub(crate) fn slab(origin: Vec3, inv: Vec3, min: Vec3, max: Vec3, max_t: f32) -> bool {
    let t0 = (min - origin) * inv;
    let t1 = (max - origin) * inv;
    let tmin = t0.min(t1).max_element();
    let tmax = t0.max(t1).min_element();
    tmax >= tmin.max(0.0) && tmin <= max_t
}

/// Möller–Trumbore intersection of the ray with triangle `tri`.
pub(crate) fn hit_triangle(mesh: &Mesh, tri: u32, origin: Vec3, dir: Vec3) -> Option<RayHit> {
    let (a, b, c) = mesh.corners(tri as usize);
    let edge1 = b - a;
    let edge2 = c - a;
    let pvec = dir.cross(edge2);
    let det = edge1.dot(pvec);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv_det = 1.0 / det;
    let tvec = origin - a;
    let u = tvec.dot(pvec) * inv_det;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let qvec = tvec.cross(edge1);
    let v = dir.dot(qvec) * inv_det;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = edge2.dot(qvec) * inv_det;
    if t <= 1e-6 {
        return None;
    }
    Some(RayHit {
        distance: t,
        triangle: tri,
        point: (origin + dir * t).to_array(),
        normal: edge1.cross(edge2).to_array(),
    })
}

/// Squared distance from `p` to the axis-aligned box `[min, max]`.
pub(crate) fn aabb_dist2(p: Vec3, min: Vec3, max: Vec3) -> f32 {
    let d = (min - p).max(p - max).max(Vec3::ZERO);
    d.length_squared()
}

/// The nearest point on triangle `tri` to `p`.
pub(crate) fn closest_on_triangle(mesh: &Mesh, tri: u32, p: Vec3) -> ClosestPoint {
    let (a, b, c) = mesh.corners(tri as usize);
    let q = closest_point_triangle(p, a, b, c);
    ClosestPoint {
        distance: (q - p).length(),
        triangle: tri,
        point: q.to_array(),
    }
}

/// The closest point on triangle `abc` to `p` (Ericson, *Real-Time Collision Detection*).
fn closest_point_triangle(p: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
    let ab = b - a;
    let ac = c - a;
    let ap = p - a;
    let d1 = ab.dot(ap);
    let d2 = ac.dot(ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = p - b;
    let d3 = ab.dot(bp);
    let d4 = ac.dot(bp);
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return a + ab * (d1 / (d1 - d3));
    }
    let cp = p - c;
    let d5 = ab.dot(cp);
    let d6 = ac.dot(cp);
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return a + ac * (d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        return b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    let denom = 1.0 / (va + vb + vc);
    a + ab * (vb * denom) + ac * (vc * denom)
}
