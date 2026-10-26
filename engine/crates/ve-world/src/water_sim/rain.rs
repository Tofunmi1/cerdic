//! Deterministic, fixed-step rainfall forcing.
/// Seeded drop stream. Advance from a fixed simulation clock for repeatable rain.
#[derive(Clone, Debug)]
pub struct Rain {
    state: u32,
    budget: f64,
}
impl Rain {
    /// Starts a stream from an authored seed.
    pub fn new(seed: u32) -> Self {
        Self {
            state: seed ^ 0xa731_59c3,
            budget: 0.0,
        }
    }
    fn unit(&mut self) -> f32 {
        self.state = self
            .state
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        #[expect(clippy::cast_precision_loss, reason = "24-bit random mantissa")]
        {
            (self.state >> 8) as f32 / 16_777_216.0
        }
    }
    /// Returns drop locations in a rectangle; reject land at the caller to obtain the wet-area rate.
    pub fn drops(&mut self, dt: f32, rain: f32, size: [f32; 2]) -> Vec<[f32; 2]> {
        if ![dt, rain, size[0], size[1]]
            .iter()
            .all(|n| n.is_finite() && *n >= 0.0)
        {
            return Vec::new();
        }
        self.budget +=
            f64::from(dt) * f64::from(rain) * f64::from(size[0]) * f64::from(size[1]) / 100.0;
        let mut drops = Vec::new();
        while self.budget >= 1.0 {
            self.budget -= 1.0;
            drops.push([self.unit() * size[0], self.unit() * size[1]]);
        }
        drops
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rain_is_seeded_and_counts_area_and_elapsed_time() {
        let run = |seed| {
            let mut r = Rain::new(seed);
            (0..600)
                .flat_map(|_| r.drops(1.0 / 60.0, 1.0, [20.0, 10.0]))
                .collect::<Vec<_>>()
        };
        assert_eq!(run(42), run(42));
        assert_ne!(run(42), run(43));
        assert_eq!(run(42).len(), 20);
    }
}
