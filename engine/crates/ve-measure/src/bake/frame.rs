//! The geometry a bake needs at one texel: smooth normals, the tangent frame from the UV layout, and the ray directions.

use std::collections::BTreeMap;

use glam::Vec3;

/// A tangent frame: `normal` points out of the surface, `tangent` toward increasing u, `bitangent` completes it.
#[derive(Clone, Copy, Debug)]
pub(super) struct Frame {
    pub(super) normal: Vec3,
    pub(super) tangent: Vec3,
    pub(super) bitangent: Vec3,
}

impl Frame {
    /// Orthonormalises the raw `tangent` against `normal`; the bitangent flips for a mirrored UV triangle
    /// (`bitangent_hint` is the raw direction of increasing v, which tells the handedness).
    pub(super) fn new(normal: Vec3, tangent: Vec3, bitangent_hint: Vec3) -> Self {
        let normal = normal.normalize_or_zero();
        let tangent = (tangent - normal * normal.dot(tangent)).normalize_or_zero();
        let mut bitangent = normal.cross(tangent);
        if bitangent.dot(bitangent_hint) < 0.0 {
            bitangent = -bitangent;
        }
        Self {
            normal,
            tangent,
            bitangent,
        }
    }

    /// A world direction in this frame's coordinates (x along the tangent, y along the bitangent, z along the normal).
    pub(super) fn to_local(self, v: Vec3) -> Vec3 {
        Vec3::new(
            v.dot(self.tangent),
            v.dot(self.bitangent),
            v.dot(self.normal),
        )
    }

    /// A direction given in this frame's coordinates, in world space.
    pub(super) fn to_world(self, v: Vec3) -> Vec3 {
        self.tangent * v.x + self.bitangent * v.y + self.normal * v.z
    }
}

/// Area-weighted vertex normals, welded by position so a UV seam does not split the shading.
pub(super) fn smooth_normals(positions: &[[f32; 3]], indices: &[u32]) -> Vec<Vec3> {
    let key = |p: &[f32; 3]| [p[0].to_bits(), p[1].to_bits(), p[2].to_bits()];
    let mut sums: BTreeMap<[u32; 3], Vec3> = BTreeMap::new();
    for tri in indices.chunks_exact(3) {
        let [a, b, c] = [tri[0], tri[1], tri[2]].map(|i| positions[i as usize]);
        let face = (Vec3::from_array(b) - Vec3::from_array(a))
            .cross(Vec3::from_array(c) - Vec3::from_array(a));
        for corner in [a, b, c] {
            *sums.entry(key(&corner)).or_insert(Vec3::ZERO) += face;
        }
    }
    positions
        .iter()
        .map(|p| {
            sums.get(&key(p))
                .copied()
                .unwrap_or(Vec3::ZERO)
                .normalize_or_zero()
        })
        .collect()
}

/// Directions of increasing u and v on a triangle, from its corner positions and UVs; `None` when the UV triangle has no area.
pub(super) fn uv_directions(p: [Vec3; 3], uv: [[f32; 2]; 3]) -> Option<(Vec3, Vec3)> {
    let (e1, e2) = (p[1] - p[0], p[2] - p[0]);
    let (du1, dv1) = (uv[1][0] - uv[0][0], uv[1][1] - uv[0][1]);
    let (du2, dv2) = (uv[2][0] - uv[0][0], uv[2][1] - uv[0][1]);
    let det = du1 * dv2 - du2 * dv1;
    if det.abs() < 1.0e-12 {
        return None;
    }
    let r = 1.0 / det;
    Some(((e1 * dv2 - e2 * dv1) * r, (e2 * du1 - e1 * du2) * r))
}

/// The base-2 radical inverse of `i`: a low-discrepancy sequence on 0..1.
fn radical_inverse(mut i: u32) -> f32 {
    i = i.rotate_left(16);
    i = ((i & 0x00ff_00ff) << 8) | ((i & 0xff00_ff00) >> 8);
    i = ((i & 0x0f0f_0f0f) << 4) | ((i & 0xf0f0_f0f0) >> 4);
    i = ((i & 0x3333_3333) << 2) | ((i & 0xcccc_cccc) >> 2);
    i = ((i & 0x5555_5555) << 1) | ((i & 0xaaaa_aaaa) >> 1);
    i as f32 * 2.328_306_4e-10
}

/// The `i`th of `count` cosine-weighted directions over the hemisphere around +z, turned about z by `turn` (0..1).
pub(super) fn hemisphere(i: u32, count: u32, turn: f32) -> Vec3 {
    let u1 = (i as f32 + 0.5) / count as f32;
    let phi = (radical_inverse(i) + turn).fract() * std::f32::consts::TAU;
    let r = u1.sqrt();
    Vec3::new(r * phi.cos(), r * phi.sin(), (1.0 - u1).sqrt())
}

/// A repeatable per-texel number in 0..1, so neighbouring texels do not share the same ray pattern.
pub(super) fn texel_turn(x: u32, y: u32) -> f32 {
    let h = x.wrapping_mul(73_856_093) ^ y.wrapping_mul(19_349_663);
    let h = (h ^ (h >> 13)).wrapping_mul(0x5bd1_e995);
    (h ^ (h >> 15)) as f32 / u32::MAX as f32
}
