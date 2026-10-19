use super::Waveform;

pub(super) const PI2: f32 = core::f32::consts::TAU;
pub(super) fn sin(x: f32) -> f32 {
    let mut v = x - (x / PI2).floor() * PI2;
    if v > core::f32::consts::PI {
        v -= PI2;
    }
    let z = v * v;
    v * (1.0 - z / 6.0 + z * z / 120.0 - z * z * z / 5_040.0 + z * z * z * z / 362_880.0)
}
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "clamped semitone count is nonnegative and bounded before conversion"
)]
pub(super) fn semitone(s: f32) -> f32 {
    // Repeated twelfth-root approximation avoids pow/libm.
    let steps = s.clamp(-48.0, 48.0);
    let root = 1.059_463_1_f32;
    let mut result = 1.0;
    let mut n = steps.abs() as u32;
    while n > 0 {
        result *= root;
        n -= 1;
    }
    if steps < 0.0 { 1.0 / result } else { result }
}
pub(super) fn wave(w: Waveform, p: f32, rng: &mut Rng) -> f32 {
    match w {
        Waveform::Sine => sin(p * PI2),
        Waveform::Saw => 2.0 * p - 1.0,
        Waveform::Square => {
            if p < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
        Waveform::Triangle => 1.0 - 4.0 * (p - 0.5).abs(),
        Waveform::WhiteNoise => rng.noise(),
    }
}
pub(super) fn normalise(mut values: Vec<f32>) -> Vec<f32> {
    let peak = values.iter().fold(0.0_f32, |p, v| p.max(v.abs()));
    let scale = if peak > 1.0 { 1.0 / peak } else { 1.0 };
    for v in &mut values {
        if v.is_finite() {
            *v = (*v * scale).clamp(-1.0, 1.0);
        } else {
            *v = 0.0;
        }
    }
    values
}
pub(super) struct Rng(pub(super) u64);
impl Rng {
    pub(super) fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    #[expect(
        clippy::cast_precision_loss,
        reason = "top 24 random bits are intentionally mapped to an f32 noise sample"
    )]
    pub(super) fn noise(&mut self) -> f32 {
        ((self.next() >> 40) as f32 / 8_388_607.5) - 1.0
    }
}
