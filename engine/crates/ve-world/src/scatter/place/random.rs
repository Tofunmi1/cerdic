//! Deterministic random helpers shared by scatter placement and species selection.

pub(in crate::scatter) fn mix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

pub(in crate::scatter) fn cell_hash(seed: u32, x: i32, z: i32, salt: u32) -> u64 {
    let x_bits = u32::from_ne_bytes(x.to_ne_bytes());
    let z_bits = u32::from_ne_bytes(z.to_ne_bytes());
    let coordinates = (u64::from(x_bits) << 32) | u64::from(z_bits);
    mix64(
        coordinates ^ (u64::from(seed) << 1) ^ u64::from(salt).wrapping_mul(0xd6e8_feb8_6659_fd93),
    )
}

pub(in crate::scatter) fn unit(value: u64) -> f32 {
    top_bits_f32(value >> 40) / 16_777_216.0
}

#[expect(
    clippy::cast_precision_loss,
    reason = "24 random bits are exactly representable as f32"
)]
fn top_bits_f32(value: u64) -> f32 {
    value as f32
}

#[expect(
    clippy::cast_precision_loss,
    reason = "species identifiers are carried in the documented f32 channel"
)]
pub(in crate::scatter) fn id_value(value: u32) -> f32 {
    value as f32
}

pub(in crate::scatter) fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
