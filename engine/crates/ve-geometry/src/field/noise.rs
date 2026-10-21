//! Integer-hashed smooth value noise used by field displacement.

fn corner(cell: [i64; 3], seed: u32) -> f64 {
    let mut h = u64::from(seed).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    for coordinate in cell {
        h ^= coordinate.cast_unsigned();
        h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
        h ^= h >> 31;
    }
    h = h.wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^= h >> 29;
    #[expect(clippy::cast_precision_loss, reason = "24 hash bits fit f64 exactly")]
    let unit = (h >> 40) as f64 / 16_777_216.0;
    unit * 2.0 - 1.0
}

/// Smooth, normalised fractal value noise. Coordinates are divided by the caller's wavelength.
pub(crate) fn fractal(
    point: [f64; 3],
    seed: u32,
    octaves: u8,
    persistence: f64,
    ridged: bool,
) -> f64 {
    let mut sum = 0.0;
    let mut total = 0.0;
    let mut weight = 1.0;
    for octave in 0..octaves {
        let frequency = f64::from(1_u32 << octave);
        let at = point.map(|value| value * frequency);
        let base = at.map(f64::floor);
        #[expect(
            clippy::cast_possible_truncation,
            reason = "validated field coordinates are bounded"
        )]
        let cell = base.map(|value| value as i64);
        let fade = [0, 1, 2].map(|axis| {
            let t = at[axis] - base[axis];
            t * t * (3.0 - 2.0 * t)
        });
        let mut value = 0.0;
        for dx in 0..2_i64 {
            for dy in 0..2_i64 {
                for dz in 0..2_i64 {
                    let weight_corner = (if dx == 1 { fade[0] } else { 1.0 - fade[0] })
                        * (if dy == 1 { fade[1] } else { 1.0 - fade[1] })
                        * (if dz == 1 { fade[2] } else { 1.0 - fade[2] });
                    value += weight_corner
                        * corner(
                            [cell[0] + dx, cell[1] + dy, cell[2] + dz],
                            seed.wrapping_add(u32::from(octave)),
                        );
                }
            }
        }
        sum += weight
            * if ridged {
                1.0 - 2.0 * value.abs()
            } else {
                value
            };
        total += weight;
        weight *= persistence;
    }
    sum / total
}

#[cfg(test)]
mod tests {
    use super::fractal;

    #[test]
    fn bounded_deterministic_and_seeded() {
        for i in 0..1_000_000_u32 {
            let p = [
                f64::from(i) * 0.000_173,
                f64::from(i % 997) * 0.003,
                f64::from(i % 313) * -0.007,
            ];
            let a = fractal(p, 9, 5, 0.6, false);
            assert!((-1.0..=1.0).contains(&a));
            assert_eq!(a.to_bits(), fractal(p, 9, 5, 0.6, false).to_bits());
        }
        assert_ne!(
            fractal([0.21, 0.32, 0.43], 1, 3, 0.5, false).to_bits(),
            fractal([0.21, 0.32, 0.43], 2, 3, 0.5, false).to_bits()
        );
        let point = [0.21, 0.32, 0.43];
        let one = fractal(point, 7, 1, 0.5, false);
        let many = fractal(point, 7, 6, 0.5, false);
        assert!((-1.0..=1.0).contains(&many));
        assert!(one.abs() <= 1.0);
        let field = crate::Field::sphere([0.0; 3], 0.5)
            .and_then(|field| field.noise(0.05, 0.15, 3, 0.5, 7, false));
        let field = field.expect("valid test field");
        let tape = crate::field::compile(&field).expect("valid tape");
        for sample in [[0.21, 0.32, 0.43], [-0.17, 0.44, 0.03], [1.0, -0.5, 0.8]] {
            let rust = crate::field::evaluate(&field, sample);
            let cpp = ve_kernel::evaluate_tape(&tape, sample).expect("valid C++ tape sample");
            assert!((rust - cpp).abs() <= 1.0e-6, "Rust {rust} C++ {cpp}");
        }
    }
}
