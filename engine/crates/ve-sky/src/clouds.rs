//! The cloud layer of the procedural sky, on the CPU: the same hash, value noise and cloud coverage as the engine's own sky shader
//! (`ve-render/src/shaders/sky.wgsl`), so the cubemap sky of the Bevy renderer can show clouds. Deterministic: the same direction, cover
//! and seed always give the same amount.
//!
//! One deliberate difference from the shader: the shader divides `hash_cell(..) >> 8` by 2^24 in `noise2` (the hash already has only 24
//! bits, so its noise never exceeds 0.004 and no cloud ever shows). Here a corner value is `hash_cell(..) / 2^24`, in `[0, 1)`.

/// The 24-bit hash of an integer cell and a seed, exactly the shader's `hash_cell` (a 64-bit multiply-xorshift mix over the three cell
/// coordinates, then its top 24 bits). Native `u64` arithmetic gives the same bits as the shader's pairs of `u32`.
#[must_use]
pub fn hash_cell(cell: [i32; 3], seed: u32) -> u32 {
    let mut hash = u64::from(seed).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    for coordinate in cell {
        let signed = i64::from(coordinate).cast_unsigned();
        hash ^= signed;
        hash = hash.wrapping_mul(0xbf58_476d_1ce4_e5b9);
        hash ^= hash >> 31;
    }
    hash = hash.wrapping_mul(0x94d0_49bb_1331_11eb);
    hash ^= hash >> 29;
    (hash >> 40) as u32
}

/// Smooth value noise in `[0, 1)`: the four corner values of the cell around `p` (each `hash_cell([x, y, 0], seed) / 2^24`), blended
/// with the weights `w = f * f * (3 - 2 f)` for the fractional part `f` of `p`, as the shader's `noise2` does.
#[must_use]
#[expect(
    clippy::cast_possible_truncation,
    reason = "WGSL converts floor(p) to i32 for its integer lattice coordinates"
)]
pub fn noise2(point: [f32; 2], seed: u32) -> f32 {
    let x_floor = point[0].floor();
    let y_floor = point[1].floor();
    let x = x_floor as i32;
    let y = y_floor as i32;
    let fx = point[0] - x_floor;
    let fy = point[1] - y_floor;
    let wx = fx * fx * (3.0 - 2.0 * fx);
    let wy = fy * fy * (3.0 - 2.0 * fy);
    let corner = |cell_x, cell_y| hash_as_noise(hash_cell([cell_x, cell_y, 0], seed));
    let lower_left = corner(x, y);
    let lower_right = corner(x.wrapping_add(1), y);
    let upper_left = corner(x, y.wrapping_add(1));
    let upper_right = corner(x.wrapping_add(1), y.wrapping_add(1));
    let lower = lower_left + (lower_right - lower_left) * wx;
    let upper = upper_left + (upper_right - upper_left) * wx;
    lower + (upper - lower) * wy
}

/// How much cloud covers the sky toward the unit `direction`, from 0 to 0.78. Zero for a direction at or below the horizon (`y <= 0`).
/// Above it: `coord = (x, z) / max(y, 0.08) * 2.2`, `n = 0.65 * noise2(coord, seed) + 0.35 * noise2(coord * 2.03, seed + 101)`, and the amount is
/// `0.78 * smoothstep(0.82 - 0.58 * cover, 0.92 - 0.45 * cover, n)`. `cover` is 0 to 1.
#[must_use]
pub fn cloud_amount(direction: [f32; 3], cover: f32, seed: u32) -> f32 {
    let [horizontal_x, vertical, horizontal_z] = direction;
    if vertical <= 0.0 {
        return 0.0;
    }
    let scale = 2.2 / vertical.max(0.08);
    let coord = [horizontal_x * scale, horizontal_z * scale];
    let combined_noise = 0.65 * noise2(coord, seed)
        + 0.35 * noise2([coord[0] * 2.03, coord[1] * 2.03], seed.wrapping_add(101));
    let low = 0.82 - 0.58 * cover;
    let high = 0.92 - 0.45 * cover;
    let interpolation = ((combined_noise - low) / (high - low)).clamp(0.0, 1.0);
    0.78 * interpolation * interpolation * (3.0 - 2.0 * interpolation)
}

#[expect(
    clippy::cast_precision_loss,
    reason = "the hash has only 24 significant bits, exactly representable by f32"
)]
fn hash_as_noise(hash: u32) -> f32 {
    hash as f32 / 16_777_216.0
}
