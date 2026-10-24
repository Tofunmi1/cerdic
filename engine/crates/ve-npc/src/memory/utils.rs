pub(super) fn finite_clamp(v: f32, lo: f32, hi: f32) -> f32 {
    if v.is_nan() { 0.0 } else { v.clamp(lo, hi) }
}

pub(super) fn decay(age: f64, half: f64) -> f32 {
    if half <= 0.0 {
        return if age <= 0.0 { 1.0 } else { 0.0 };
    }
    let value = 0.5_f64.powf(age / half).clamp(0.0, 1.0);
    // The value is bounded to [0, 1], so conversion cannot overflow or become non-finite.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "bounded normalized score is represented as f32"
    )]
    {
        value as f32
    }
}

pub(super) fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let na = a.iter().map(|v| v * v).sum::<f32>();
    let nb = b.iter().map(|v| v * v).sum::<f32>();
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    (a.iter().zip(b).map(|(x, y)| x * y).sum::<f32>() / (na * nb).sqrt()).clamp(0.0, 1.0)
}
