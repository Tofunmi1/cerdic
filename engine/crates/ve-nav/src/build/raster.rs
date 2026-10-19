//! Triangle-versus-cell overlap and plane sampling for the heightfield.

use glam::Vec2;

/// A triangle projected onto the ground plane, as `(x, z)` pairs.
pub(crate) type Tri2 = [Vec2; 3];

/// A tiny tolerance keeps touching (which counts as overlap) from being lost to rounding in
/// `centre +/- half`, especially for thin walls that lie exactly on a cell edge.
const TOUCH: f32 = 1.0e-4;

/// Do `tri` and the axis-aligned cell `min..max` overlap in the ground plane?
///
/// Separating axis test using the two cell axes and the three edge normals. Touching counts as an
/// overlap, so a wall that lies exactly on a cell edge still blocks that cell.
pub(crate) fn overlaps(tri: &Tri2, min: Vec2, max: Vec2) -> bool {
    let centre = (min + max) * 0.5;
    let half = (max - min) * 0.5;
    let mut axes = [Vec2::X, Vec2::Y, Vec2::ZERO, Vec2::ZERO, Vec2::ZERO];
    for i in 0..3 {
        let a = tri[i];
        let b = tri[(i + 1) % 3];
        axes[2 + i] = Vec2::new(b.y - a.y, a.x - b.x);
    }
    for axis in axes {
        if axis.length_squared() <= 0.0 {
            continue;
        }
        let mut lo = f32::INFINITY;
        let mut hi = f32::NEG_INFINITY;
        for vertex in tri {
            let p = vertex.dot(axis);
            lo = lo.min(p);
            hi = hi.max(p);
        }
        let mid = centre.dot(axis);
        let radius = half.x * axis.x.abs() + half.y * axis.y.abs();
        if hi < mid - radius - TOUCH || lo > mid + radius + TOUCH {
            return false;
        }
    }
    true
}

/// Height of the plane through `tri` at the ground-plane point `p`, if the plane is not vertical.
pub(crate) fn plane_height(tri: &[[f32; 3]; 3], normal: [f32; 3], p: Vec2) -> Option<f32> {
    let ny = normal[1];
    if ny.abs() <= 1.0e-6 {
        return None;
    }
    let dx = p.x - tri[0][0];
    let dz = p.y - tri[0][2];
    Some(tri[0][1] - (normal[0] * dx + normal[2] * dz) / ny)
}
