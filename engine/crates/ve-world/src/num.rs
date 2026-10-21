//! Small, bounded conversions shared by grid algorithms.

/// Converts a grid coordinate whose side is bounded by `MAX_SIDE` to metres.
#[expect(
    clippy::cast_precision_loss,
    reason = "grid coordinates are bounded to 2048"
)]
pub(crate) fn cell_f32(index: usize) -> f32 {
    index as f32
}

/// Converts a signed neighbour displacement after checking grid bounds.
pub(crate) fn offset(index: usize, delta: i32, limit: usize) -> Option<usize> {
    let value = i32::try_from(index).ok()?.checked_add(delta)?;
    let value = usize::try_from(value).ok()?;
    (value < limit).then_some(value)
}

/// Floors a previously clamped, non-negative grid coordinate.
#[expect(
    clippy::cast_possible_truncation,
    reason = "the value is bounded by a grid side"
)]
#[expect(clippy::cast_sign_loss, reason = "the value is clamped non-negative")]
pub(crate) fn floor_index(value: f32) -> usize {
    value.floor() as usize
}

/// Converts a finite noise coordinate to the saturating lattice index used by the noise hash.
#[expect(
    clippy::cast_possible_truncation,
    reason = "Rust's float-to-int conversion preserves the previous saturating lattice behavior"
)]
pub(crate) fn lattice_index(value: f32) -> i32 {
    value.floor() as i32
}

/// Converts a bounded grid cell count to a floating-point divisor.
#[expect(clippy::cast_precision_loss, reason = "grid cell counts are bounded")]
pub(crate) fn count_f32(count: usize) -> f32 {
    count as f32
}

/// Converts a bounded signed grid offset to floating-point coordinates.
#[expect(
    clippy::cast_precision_loss,
    reason = "signed noise coordinates are finite lattice values"
)]
pub(crate) fn lattice_f32(index: i32) -> f32 {
    index as f32
}

/// Converts a rounded, aligned world displacement to its saturating signed cell offset.
#[expect(
    clippy::cast_possible_truncation,
    reason = "the value is rounded and Rust's float-to-int conversion preserves saturation"
)]
pub(crate) fn rounded_offset(value: f32) -> i64 {
    value as i64
}

/// Converts a grid index bounded by `MAX_SIDE` to a signed cell coordinate.
#[expect(
    clippy::cast_possible_wrap,
    reason = "height grid sides are bounded by 2048"
)]
pub(crate) fn signed_cell(index: usize) -> i64 {
    index as i64
}

/// Converts an already clamped unsigned value to a 16-bit texture channel.
#[expect(
    clippy::cast_possible_truncation,
    reason = "the caller clamps the value to the 16-bit unsigned range"
)]
#[expect(
    clippy::cast_sign_loss,
    reason = "the caller clamps the value non-negative"
)]
pub(crate) fn texture_u16(value: f32) -> u16 {
    value as u16
}

/// Converts an already clamped unsigned value to an 8-bit splat channel.
#[expect(
    clippy::cast_possible_truncation,
    reason = "the caller clamps the value to the 8-bit unsigned range"
)]
#[expect(
    clippy::cast_sign_loss,
    reason = "the caller clamps the value non-negative"
)]
pub(crate) fn weight_u8(value: f32) -> u8 {
    value as u8
}

/// Converts a bounded cell index to the flow receiver representation.
pub(crate) fn cell_u32(index: usize) -> u32 {
    u32::try_from(index).unwrap_or(u32::MAX)
}
