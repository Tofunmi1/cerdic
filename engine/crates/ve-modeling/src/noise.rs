//! Seeded smooth value noise shared by the noise deformer and the terrain generator.
//!
//! Integer hashing plus `+ - * /` only, so the same seed gives the same field on every platform.

/// Integer hash of a lattice cell to a value in -1 to 1.
fn corner(cell: [i64; 3], seed: u32) -> f64 {
    let h = corner_hash(cell, seed);
    // 24 high bits to 0..1, then -1..1: exact in f64.
    #[expect(
        clippy::cast_precision_loss,
        reason = "24 bits fit an f64 mantissa exactly"
    )]
    let unit = (h >> 40) as f64 / 16_777_216.0;
    unit * 2.0 - 1.0
}

/// Exact 64-bit seeded lattice hash. `sky.wgsl` emulates this with two u32 lanes.
fn corner_hash(cell: [i64; 3], seed: u32) -> u64 {
    let mut h = u64::from(seed).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    for c in cell {
        h ^= c.cast_unsigned();
        h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
        h ^= h >> 31;
    }
    h = h.wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^= h >> 29;
    h
}

/// Smooth value noise in -1 to 1: trilinear between hashed corners with a smoothstep fade.
pub(crate) fn value_noise(at: [f64; 3], seed: u32) -> f64 {
    let base = at.map(f64::floor);
    #[expect(
        clippy::cast_possible_truncation,
        reason = "noise coordinates are bounded by the node limits"
    )]
    let cell = base.map(|v| v as i64);
    let fade = [0, 1, 2].map(|axis| {
        let t = at[axis] - base[axis];
        t * t * (3.0 - 2.0 * t)
    });
    let mut total = 0.0;
    for dx in 0..2_i64 {
        for dy in 0..2_i64 {
            for dz in 0..2_i64 {
                let weight = (if dx == 1 { fade[0] } else { 1.0 - fade[0] })
                    * (if dy == 1 { fade[1] } else { 1.0 - fade[1] })
                    * (if dz == 1 { fade[2] } else { 1.0 - fade[2] });
                total += weight * corner([cell[0] + dx, cell[1] + dy, cell[2] + dz], seed);
            }
        }
    }
    total
}

/// Fractal noise in -1 to 1: `octaves` layers, each twice as fine and `persistence` times as strong as the last,
/// normalised so the sum stays within -1 to 1.
pub(crate) fn fractal(at: [f64; 3], seed: u32, octaves: u32, persistence: f64) -> f64 {
    let (mut total, mut amplitude, mut norm, mut frequency) = (0.0, 1.0, 0.0, 1.0);
    for octave in 0..octaves {
        let scaled = at.map(|v| v * frequency);
        total += amplitude * value_noise(scaled, seed.wrapping_add(octave.wrapping_mul(101)));
        norm += amplitude;
        amplitude *= persistence;
        frequency *= 2.0;
    }
    if norm > 0.0 { total / norm } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_stays_in_range_and_is_repeatable() {
        for step in 0..500 {
            let at = [
                f64::from(step) * 0.173,
                f64::from(step) * -0.091,
                f64::from(step) * 0.057,
            ];
            let (a, b) = (value_noise(at, 3), fractal(at, 3, 4, 0.5));
            assert!(
                (-1.0..=1.0).contains(&a) && (-1.0..=1.0).contains(&b),
                "{a} {b}"
            );
            assert_eq!(a.to_bits(), value_noise(at, 3).to_bits());
        }
    }

    #[test]
    fn sky_wgsl_64_bit_hash_vectors_match_the_shared_noise_hash() {
        assert_eq!(corner_hash([2, -7, 19], 41), 0xcfba_5ae7_64f4_97d3);
        assert_eq!(corner_hash([1, 2, 3], 9), 0x2320_939f_0a70_210a);
    }

    #[test]
    fn more_octaves_add_finer_detail_without_leaving_the_range() {
        let rough: f64 = (0..200)
            .map(|i| {
                (fractal([f64::from(i) * 0.01, 0.0, 0.3], 1, 5, 0.6)
                    - fractal([f64::from(i) * 0.01 + 0.01, 0.0, 0.3], 1, 5, 0.6))
                .abs()
            })
            .sum();
        let smooth: f64 = (0..200)
            .map(|i| {
                (fractal([f64::from(i) * 0.01, 0.0, 0.3], 1, 1, 0.6)
                    - fractal([f64::from(i) * 0.01 + 0.01, 0.0, 0.3], 1, 1, 0.6))
                .abs()
            })
            .sum();
        assert!(rough > smooth, "{rough} {smooth}");
    }
}
