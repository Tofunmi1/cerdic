
#[cfg(test)]
mod tests {
    use super::*;

    const MIN: [f64; 3] = [-1.0, 0.0, -1.0];
    const MAX: [f64; 3] = [1.0, 2.0, 1.0];

    /// Bit-for-bit equality of two points.
    fn same(a: [f64; 3], b: [f64; 3]) -> bool {
        a.iter().zip(&b).all(|(x, y)| x.to_bits() == y.to_bits())
    }

    #[test]
    fn shear_moves_the_top_by_the_shift_and_the_bottom_not_at_all() {
        let map = Map::Shear([0.5, -0.25]);
        assert!(same(map.apply([0.3, 0.0, 0.1], MIN, MAX), [0.3, 0.0, 0.1]));
        let top = map.apply([0.3, 2.0, 0.1], MIN, MAX);
        assert!(
            (top[0] - 0.8).abs() < 1e-12 && (top[2] + 0.15).abs() < 1e-12,
            "{top:?}"
        );
    }

    #[test]
    fn wave_follows_the_sine_along_y() {
        let map = Map::Wave {
            amplitude: 0.2,
            wavelength: 1.0,
            phase: 0.0,
            along_z: false,
        };
        assert!(
            (map.apply([0.0, 0.25, 0.0], MIN, MAX)[0] - 0.2).abs() < 1e-12,
            "a quarter wavelength up is the crest"
        );
        assert!(
            map.apply([0.0, 0.5, 0.0], MIN, MAX)[0].abs() < 1e-12,
            "half a wavelength is back at zero"
        );
        let sideways = Map::Wave {
            amplitude: 0.2,
            wavelength: 1.0,
            phase: 0.0,
            along_z: true,
        }
        .apply([0.0, 0.25, 0.0], MIN, MAX);
        assert!((sideways[2] - 0.2).abs() < 1e-12 && sideways[0] == 0.0);
    }

    #[test]
    fn noise_is_bounded_deterministic_smooth_and_depends_on_the_seed() {
        let map = |seed| Map::Noise {
            amplitude: 0.1,
            scale: 0.5,
            seed,
        };
        let mut longest = 0.0_f64;
        for step in 0..200 {
            let p = [
                f64::from(step) * 0.037,
                f64::from(step) * 0.021,
                f64::from(step) * -0.013,
            ];
            let moved = map(7).apply(p, MIN, MAX);
            assert!(same(moved, map(7).apply(p, MIN, MAX)));
            for axis in 0..3 {
                longest = longest.max((moved[axis] - p[axis]).abs());
            }
            let nearby = map(7).apply([p[0] + 1e-6, p[1], p[2]], MIN, MAX);
            assert!((nearby[0] - moved[0]).abs() < 1e-4, "no jumps: {step}");
        }
        assert!(
            longest <= 0.1 + 1e-12 && longest > 0.02,
            "stays within the amplitude and does move: {longest}"
        );
        assert!(!same(
            map(7).apply([0.3, 0.4, 0.5], MIN, MAX),
            map(8).apply([0.3, 0.4, 0.5], MIN, MAX)
        ));
    }

    #[test]
    fn a_lattice_with_no_offsets_is_the_identity_and_one_corner_pulls_that_corner() {
        let zero = Lattice {
            divisions: 3,
            offsets: vec![[0.0; 3]; 27],
        };
        let p = [0.3, 1.1, -0.4];
        assert!(same(Map::Lattice(zero.clone()).apply(p, MIN, MAX), p));
        let mut pulled = zero;
        pulled.offsets[(2 * 3 + 2) * 3 + 2] = [0.0, 0.5, 0.0];
        let map = Map::Lattice(pulled);
        let corner = map.apply([1.0, 2.0, 1.0], MIN, MAX);
        assert!(
            (corner[1] - 2.5).abs() < 1e-12,
            "the far corner moves by the whole offset: {corner:?}"
        );
        assert!(
            same(map.apply([-1.0, 0.0, -1.0], MIN, MAX), [-1.0, 0.0, -1.0]),
            "the opposite corner stays"
        );
    }

    #[test]
    fn bernstein_weights_sum_to_one() {
        for n in 2..=4 {
            for step in 0..=10 {
                let t = f64::from(step) / 10.0;
                let sum: f64 = (0..n).map(|i| Lattice::bernstein(n, i, t)).sum();
                assert!((sum - 1.0).abs() < 1e-12, "{n} {t}");
            }
        }
    }
}
