//! Typed reads of the named byte blocks of a human pack.
use super::{Pack, bytes, f32s};

/// A block of `f32` triples (positions, bone heads).
pub(super) fn triples(p: &Pack, n: &str) -> Vec<[f32; 3]> {
    f32s(p, n)
        .chunks_exact(3)
        .map(|x| [x[0], x[1], x[2]])
        .collect()
}

/// A block of little-endian `u32` values.
pub(super) fn u32s(p: &Pack, n: &str) -> Vec<u32> {
    bytes(p, n)
        .chunks_exact(4)
        .map(|x| u32::from_le_bytes([x[0], x[1], x[2], x[3]]))
        .collect()
}

/// A block of little-endian `i32` values.
pub(super) fn i32s(p: &Pack, n: &str) -> Vec<i32> {
    bytes(p, n)
        .chunks_exact(4)
        .map(|x| i32::from_le_bytes([x[0], x[1], x[2], x[3]]))
        .collect()
}
