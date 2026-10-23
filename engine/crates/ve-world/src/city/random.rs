//! Small stateless hashing and random-value helpers for the city generator.

/// A seed wrapper that derives values from stable integer coordinates.
pub(super) struct Random {
    seed: u64,
}

impl Random {
    /// Create deterministic random values from a city seed.
    pub(super) const fn new(seed: u64) -> Self {
        Self { seed }
    }

    /// Return a stable mixed value for an index and purpose tag.
    pub(super) fn bits(&self, index: u64, tag: u64) -> u64 {
        let mut value = self.seed ^ index.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ tag;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    /// Return a deterministic value in the half-open range zero through one.
    pub(super) fn unit(&self, index: u64, tag: u64) -> f32 {
        let bits = self.bits(index, tag) >> 48;
        f32::from(u16::try_from(bits).unwrap_or_default()) / 65_536.0
    }

    /// Return a deterministic value in the range minus one through one.
    pub(super) fn signed(&self, index: u64, tag: u64) -> f32 {
        self.unit(index, tag) * 2.0 - 1.0
    }
}
