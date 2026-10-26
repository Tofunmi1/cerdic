//! CPU reference for triangular-grid non-repeating texture sampling (ADR 0070).

/// One shifted lookup in a stochastic blend.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StochasticSample {
    /// Texture position, in the same units as the input UV.
    pub uv: [f32; 2],
    /// Linear barycentric weight; the three weights sum to one.
    pub weight: f32,
}

fn hash(mut value: u32) -> u32 {
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb_352d);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846c_a68b);
    value ^ (value >> 16)
}

/// Deterministic vertex displacement in `[0, 1)` on each axis.
///
/// Signed coordinates are reinterpreted as unsigned bits. All arithmetic wraps at 32 bits,
/// and only the upper 24 hash bits are converted to float, exactly as in the GPU shader.
#[must_use]
pub fn vertex_offset(cell: [i32; 2], seed: u32) -> [f32; 2] {
    let x = u32::from_ne_bytes(cell[0].to_ne_bytes());
    let y = u32::from_ne_bytes(cell[1].to_ne_bytes());
    let key = hash(x ^ hash(y ^ seed) ^ 0x9e37_79b9);
    #[expect(
        clippy::cast_precision_loss,
        reason = "24-bit integers fit exactly in f32"
    )]
    [key, hash(key ^ 0x68bc_21eb)].map(|h| (h >> 8) as f32 * (1.0 / 16_777_216.0))
}

/// Three shifted positions and linear weights in the containing equilateral grid triangle.
///
/// `scale` is grid cells per UV unit, clamped to `0.01..=1000`; non-finite scale
/// becomes one and non-finite UV components become zero. Grid coordinates saturate
/// before conversion to integers, keeping extreme finite coordinates safe as well.
#[must_use]
pub fn stochastic_uvs(uv: [f32; 2], scale: f32, seed: u32) -> [StochasticSample; 3] {
    let uv = uv.map(|v| if v.is_finite() { v } else { 0.0 });
    let scale = if scale.is_finite() {
        scale.clamp(0.01, 1000.0)
    } else {
        1.0
    };
    // Bound before multiplication to avoid overflow even for f32::MAX. The GPU uses the same bound.
    let p = uv.map(|v| v.clamp(-1_000_000.0, 1_000_000.0) * scale);
    let skew = [p[0] - p[1] * 0.577_350_26, p[1] * 1.154_700_5];
    let floor = skew.map(f32::floor);
    #[expect(
        clippy::cast_possible_truncation,
        reason = "bounded skew is inside i32 range"
    )]
    let cell = floor.map(|v| v as i32);
    let f = [skew[0] - floor[0], skew[1] - floor[1]];
    let w = 1.0 - f[0] - f[1];
    let (vertices, weights) = if w >= 0.0 {
        (
            [cell, [cell[0] + 1, cell[1]], [cell[0], cell[1] + 1]],
            [w, f[0], f[1]],
        )
    } else {
        (
            [
                [cell[0] + 1, cell[1] + 1],
                [cell[0] + 1, cell[1]],
                [cell[0], cell[1] + 1],
            ],
            [-w, 1.0 - f[1], 1.0 - f[0]],
        )
    };
    std::array::from_fn(|i| {
        let offset = vertex_offset(vertices[i], seed);
        StochasticSample {
            uv: [uv[0] + offset[0], uv[1] + offset[1]],
            weight: weights[i],
        }
    })
}
