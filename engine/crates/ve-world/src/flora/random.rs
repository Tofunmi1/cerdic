//! Small deterministic random stream used by the plant growers.

/// `SplitMix64` state.
pub(super) struct SplitMix64(u64);

impl SplitMix64 {
    /// Starts a stream from a 32-bit plant seed.
    pub(super) fn new(seed: u32) -> Self {
        Self(u64::from(seed).wrapping_add(0x9e37_79b9_7f4a_7c15))
    }

    /// Returns the next mixed integer.
    pub(super) fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    /// Returns a reproducible value in `[0, 1)`.
    pub(super) fn unit(&mut self) -> f32 {
        let value = u32::try_from(self.next() >> 40).unwrap_or(u32::MAX);
        let high = u8::try_from(value >> 16).unwrap_or(u8::MAX);
        let low = u16::try_from(value & 0xffff).unwrap_or(u16::MAX);
        (f32::from(high) * 65_536.0 + f32::from(low)) / 16_777_216.0
    }

    /// Returns a reproducible value in `[-1, 1)`.
    pub(super) fn signed(&mut self) -> f32 {
        self.unit() * 2.0 - 1.0
    }
}
