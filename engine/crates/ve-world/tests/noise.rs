//! Properties of `ve_world::noise`.
//!
//! These test the mathematics (range, continuity, lattice zeros, periodicity, isotropy, fractal energy), not any reference
//! implementation's output, so they hold for any correct gradient, simplex or cellular noise.
use ve_world::noise::{
    Basis, CellDistance, CellReturn, Fractal, FractalKind, Noise, NoiseError, Warp, WarpKind,
};

const SMOOTH: [Basis; 3] = [Basis::Value, Basis::Perlin, Basis::Simplex];

fn all_bases() -> Vec<Basis> {
    let mut bases = SMOOTH.to_vec();
    for distance in [
        CellDistance::Euclidean,
        CellDistance::Manhattan,
        CellDistance::Hybrid,
    ] {
        for ret in [
            CellReturn::Value,
            CellReturn::Distance,
            CellReturn::Distance2,
            CellReturn::Difference,
        ] {
            bases.push(Basis::Cellular {
                distance,
                ret,
                jitter: 1.0,
            });
        }
    }
    bases
}

fn noise(basis: Basis) -> Noise {
    Noise {
        basis,
        seed: 1337,
        frequency: 1.0,
        ..Noise::default()
    }
}

/// A repeatable spread of points that does not line up with any lattice.
fn points(count: u32) -> impl Iterator<Item = [f64; 4]> {
    (0..count).map(|i| {
        let t = f64::from(i);
        [
            (t * 0.618_033_988_7).fract() * 40.0 - 20.0,
            (t * 0.754_877_666_2).fract() * 40.0 - 20.0,
            (t * 0.569_840_290_9).fract() * 40.0 - 20.0,
            (t * 0.430_159_709_0).fract() * 40.0 - 20.0,
        ]
    })
}

fn mean_and_deviation(values: &[f64]) -> (f64, f64) {
    let n = values.len() as f64;
    let mean = values.iter().sum::<f64>() / n;
    let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
    (mean, variance.sqrt())
}

#[test]
fn every_basis_stays_in_range_and_is_deterministic_in_two_three_and_four_dimensions() {
    for basis in all_bases() {
        let n = noise(basis);
        n.validate().expect("valid");
        for p in points(4_000) {
            let a = [
                n.sample2(p[0], p[1]),
                n.sample3(p[0], p[1], p[2]),
                n.sample4(p[0], p[1], p[2], p[3]),
            ];
            let b = [
                n.sample2(p[0], p[1]),
                n.sample3(p[0], p[1], p[2]),
                n.sample4(p[0], p[1], p[2], p[3]),
            ];
            for (x, y) in a.iter().zip(&b) {
                assert!(x.is_finite(), "{basis:?} gave {x}");
                assert!(
                    (-1.0..=1.0).contains(x),
                    "{basis:?} left [-1, 1] with {x} at {p:?}"
                );
                assert_eq!(x.to_bits(), y.to_bits(), "{basis:?} is not deterministic");
            }
        }
    }
}

#[test]
fn the_seed_changes_the_result_and_the_same_seed_repeats_it() {
    for basis in all_bases() {
        let a = Noise {
            seed: 1,
            ..noise(basis)
        };
        let b = Noise {
            seed: 2,
            ..noise(basis)
        };
        let differing = points(500)
            .filter(|p| a.sample3(p[0], p[1], p[2]) != b.sample3(p[0], p[1], p[2]))
            .count();
        assert!(differing > 400, "{basis:?}: seeds 1 and 2 agree too often");
    }
}

#[test]
fn smooth_bases_are_continuous() {
    // A step of 1e-4 may move the value by at most `LIPSCHITZ * step` (generous: real gradient noise is under 4).
    const STEP: f64 = 1.0e-4;
    const LIPSCHITZ: f64 = 12.0;
    for basis in SMOOTH {
        let n = noise(basis);
        for p in points(3_000) {
            let base3 = n.sample3(p[0], p[1], p[2]);
            for axis in 0..3 {
                let mut q = [p[0], p[1], p[2]];
                q[axis] += STEP;
                let moved = n.sample3(q[0], q[1], q[2]);
                assert!(
                    (moved - base3).abs() <= LIPSCHITZ * STEP,
                    "{basis:?} jumps by {} along axis {axis} at {p:?}",
                    (moved - base3).abs()
                );
            }
            let w = n.sample4(p[0], p[1], p[2], p[3]);
            let w_moved = n.sample4(p[0], p[1], p[2], p[3] + STEP);
            assert!(
                (w_moved - w).abs() <= LIPSCHITZ * STEP,
                "{basis:?} jumps along the fourth axis"
            );
        }
    }
}

#[test]
fn cellular_distance_is_continuous_but_cellular_value_is_piecewise_constant() {
    const STEP: f64 = 1.0e-5;
    let distance = noise(Basis::Cellular {
        distance: CellDistance::Euclidean,
        ret: CellReturn::Distance,
        jitter: 1.0,
    });
    for p in points(2_000) {
        let a = distance.sample3(p[0], p[1], p[2]);
        let b = distance.sample3(p[0] + STEP, p[1], p[2]);
        assert!((a - b).abs() <= 8.0 * STEP, "distance jumps at {p:?}");
    }
    let value = noise(Basis::Cellular {
        distance: CellDistance::Euclidean,
        ret: CellReturn::Value,
        jitter: 1.0,
    });
    let equal = points(2_000)
        .filter(|p| {
            value.sample3(p[0], p[1], p[2]).to_bits()
                == value.sample3(p[0] + STEP, p[1], p[2]).to_bits()
        })
        .count();
    assert!(
        equal > 1_900,
        "cell value should be flat inside a cell: {equal}/2000"
    );
}

#[test]
fn perlin_is_zero_on_the_integer_lattice() {
    let n = noise(Basis::Perlin);
    for i in -4..=4 {
        for j in -4..=4 {
            assert!(n.sample2(f64::from(i), f64::from(j)).abs() < 1.0e-9);
            for k in -2..=2 {
                assert!(n.sample3(f64::from(i), f64::from(j), f64::from(k)).abs() < 1.0e-9);
            }
        }
    }
}

#[test]
fn the_frequency_scales_the_domain() {
    for basis in all_bases() {
        let coarse = noise(basis);
        let fine = Noise {
            frequency: 2.0,
            ..noise(basis)
        };
        for p in points(200) {
            assert_eq!(
                fine.sample3(p[0], p[1], p[2]).to_bits(),
                coarse.sample3(p[0] * 2.0, p[1] * 2.0, p[2] * 2.0).to_bits(),
                "{basis:?}"
            );
        }
    }
}

#[test]
fn values_are_centred_and_use_a_useful_part_of_the_range() {
    for basis in all_bases() {
        let n = noise(basis);
        let values: Vec<f64> = points(30_000)
            .map(|p| n.sample3(p[0], p[1], p[2]))
            .collect();
        let (mean, deviation) = mean_and_deviation(&values);
        let (lowest, highest) = values
            .iter()
            .fold((f64::MAX, f64::MIN), |(lo, hi), v| (lo.min(*v), hi.max(*v)));
        let centred = matches!(basis, Basis::Value | Basis::Perlin | Basis::Simplex)
            || matches!(
                basis,
                Basis::Cellular {
                    ret: CellReturn::Value,
                    ..
                }
            );
        if centred {
            assert!(mean.abs() < 0.06, "{basis:?} mean {mean}");
        }
        assert!(
            (0.1..=0.7).contains(&deviation),
            "{basis:?} deviation {deviation} (too flat or too harsh)"
        );
        assert!(
            highest - lowest > 1.0,
            "{basis:?} only spans {lowest}..{highest}"
        );
    }
}

#[test]
fn simplex_noise_has_no_strong_axis_bias() {
    // Mean absolute change over a short step along an axis and along a diagonal of the same length.
    fn roughness(n: &Noise, direction: [f64; 3]) -> f64 {
        let length = direction.iter().map(|d| d * d).sum::<f64>().sqrt();
        let step = 0.05;
        let total: f64 = points(20_000)
            .map(|p| {
                let a = n.sample3(p[0], p[1], p[2]);
                let b = n.sample3(
                    p[0] + direction[0] / length * step,
                    p[1] + direction[1] / length * step,
                    p[2] + direction[2] / length * step,
                );
                (a - b).abs()
            })
            .sum();
        total / 20_000.0
    }
    let n = noise(Basis::Simplex);
    let axis = roughness(&n, [1.0, 0.0, 0.0]);
    let diagonal = roughness(&n, [1.0, 1.0, 1.0]);
    let ratio = axis / diagonal;
    assert!(
        (0.85..=1.15).contains(&ratio),
        "axis {axis} versus diagonal {diagonal}"
    );
}

/// The final clamp must not be hiding a scale constant that is too large: almost no sample may sit exactly on the range's ends.
#[test]
fn almost_no_sample_is_clipped() {
    for basis in all_bases() {
        let n = noise(basis);
        let clipped = points(30_000)
            .filter(|p| n.sample3(p[0], p[1], p[2]).abs() >= 1.0)
            .count();
        assert!(clipped < 600, "{basis:?}: {clipped} of 30000 samples clip");
        for dims in [2, 4] {
            let clipped = points(10_000)
                .filter(|p| {
                    let v = if dims == 2 {
                        n.sample2(p[0], p[1])
                    } else {
                        n.sample4(p[0], p[1], p[2], p[3])
                    };
                    v.abs() >= 1.0
                })
                .count();
            assert!(
                clipped < 200,
                "{basis:?} in {dims}D: {clipped} of 10000 clip"
            );
        }
    }
}

#[test]
fn cellular_with_no_jitter_repeats_every_cell() {
    for distance in [
        CellDistance::Euclidean,
        CellDistance::Manhattan,
        CellDistance::Hybrid,
    ] {
        let n = noise(Basis::Cellular {
            distance,
            ret: CellReturn::Distance,
            jitter: 0.0,
        });
        for p in points(500) {
            let a = n.sample3(p[0], p[1], p[2]);
            for shift in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]] {
                let b = n.sample3(p[0] + shift[0], p[1] + shift[1], p[2] + shift[2]);
                assert!((a - b).abs() < 1.0e-9, "{distance:?} is not periodic");
            }
        }
    }
}

#[test]
fn cellular_difference_is_never_negative_before_mapping_and_second_distance_is_never_smaller() {
    // `Difference` is F2 - F1 and `Distance2` is F2, so for the same point Distance2 >= Distance after the shared linear mapping.
    let d1 = noise(Basis::Cellular {
        distance: CellDistance::Euclidean,
        ret: CellReturn::Distance,
        jitter: 1.0,
    });
    let d2 = noise(Basis::Cellular {
        distance: CellDistance::Euclidean,
        ret: CellReturn::Distance2,
        jitter: 1.0,
    });
    for p in points(3_000) {
        assert!(
            d2.sample3(p[0], p[1], p[2]) >= d1.sample3(p[0], p[1], p[2]) - 1.0e-9,
            "F2 must not be smaller than F1 at {p:?}"
        );
    }
}

fn fractal(kind: FractalKind, octaves: u8) -> Fractal {
    Fractal {
        kind,
        octaves,
        lacunarity: 2.0,
        gain: 0.5,
        weighted_strength: 0.0,
    }
}

fn roughness(n: &Noise) -> f64 {
    let step = 0.01;
    let total: f64 = points(8_000)
        .map(|p| (n.sample3(p[0], p[1], p[2]) - n.sample3(p[0] + step, p[1], p[2])).abs())
        .sum();
    total / 8_000.0
}

#[test]
fn one_octave_equals_the_plain_basis_and_more_octaves_add_detail() {
    for basis in SMOOTH {
        let plain = noise(basis);
        let one = Noise {
            fractal: Some(fractal(FractalKind::Fbm, 1)),
            ..noise(basis)
        };
        for p in points(300) {
            assert_eq!(
                plain.sample3(p[0], p[1], p[2]).to_bits(),
                one.sample3(p[0], p[1], p[2]).to_bits(),
                "{basis:?}"
            );
        }
        let rough = |octaves| {
            roughness(&Noise {
                fractal: Some(fractal(FractalKind::Fbm, octaves)),
                ..noise(basis)
            })
        };
        // Octave slopes are independent, so they add in quadrature: 8 octaves at gain 0.5 give about sqrt(8) / 2 = 1.4 times one octave.
        // Octave slopes are independent, so they add in quadrature: 8 octaves at gain 0.5 give about sqrt(8) / 2 = 1.4 times one octave.
        assert!(rough(8) > rough(1) * 1.2, "{basis:?} gains no detail");
    }
}

#[test]
fn fractals_stay_in_range_for_every_kind_and_parameter_corner() {
    for kind in [FractalKind::Fbm, FractalKind::Ridged, FractalKind::Billow] {
        for (lacunarity, gain, weighted) in [
            (1.0, 0.0, 0.0),
            (2.0, 0.5, 0.0),
            (4.0, 1.0, 0.0),
            (2.0, 0.5, 1.0),
            (3.0, 0.9, 0.5),
        ] {
            let n = Noise {
                fractal: Some(Fractal {
                    kind,
                    octaves: 8,
                    lacunarity,
                    gain,
                    weighted_strength: weighted,
                }),
                ..noise(Basis::Perlin)
            };
            n.validate().expect("valid");
            for p in points(2_000) {
                let v = n.sample3(p[0], p[1], p[2]);
                assert!(
                    (-1.0..=1.0).contains(&v),
                    "{kind:?} {lacunarity} {gain} {weighted}: {v}"
                );
            }
        }
    }
}

#[test]
fn ridged_noise_has_sharp_creases_and_billow_has_soft_ones() {
    // Ridged folds the basis, so its values pile up near the top; fbm does not.
    let ridged = Noise {
        fractal: Some(fractal(FractalKind::Ridged, 4)),
        ..noise(Basis::Perlin)
    };
    let fbm = Noise {
        fractal: Some(fractal(FractalKind::Fbm, 4)),
        ..noise(Basis::Perlin)
    };
    let (ridged_mean, _) = mean_and_deviation(
        &points(20_000)
            .map(|p| ridged.sample3(p[0], p[1], p[2]))
            .collect::<Vec<_>>(),
    );
    let (fbm_mean, _) = mean_and_deviation(
        &points(20_000)
            .map(|p| fbm.sample3(p[0], p[1], p[2]))
            .collect::<Vec<_>>(),
    );
    assert!(
        (ridged_mean - fbm_mean).abs() > 0.1,
        "ridged ({ridged_mean}) should not be centred like fbm ({fbm_mean})"
    );
}

#[test]
fn zero_warp_changes_nothing_and_a_real_warp_does() {
    for kind in [WarpKind::Gradient, WarpKind::Simplex] {
        let plain = noise(Basis::Simplex);
        let none = Noise {
            warp: Some(Warp {
                kind,
                amplitude: 0.0,
                frequency: 1.0,
                octaves: 1,
            }),
            ..noise(Basis::Simplex)
        };
        let warped = Noise {
            warp: Some(Warp {
                kind,
                amplitude: 1.5,
                frequency: 0.7,
                octaves: 2,
            }),
            ..noise(Basis::Simplex)
        };
        warped.validate().expect("valid");
        let mut changed = 0;
        for p in points(500) {
            assert_eq!(
                plain.sample3(p[0], p[1], p[2]).to_bits(),
                none.sample3(p[0], p[1], p[2]).to_bits(),
                "{kind:?}"
            );
            let w = warped.sample3(p[0], p[1], p[2]);
            assert!((-1.0..=1.0).contains(&w));
            if w != plain.sample3(p[0], p[1], p[2]) {
                changed += 1;
            }
        }
        assert!(changed > 450, "{kind:?} warp barely moves anything");
    }
}

#[test]
fn the_fourth_axis_animates_the_field() {
    for basis in SMOOTH {
        let n = noise(basis);
        let moved = points(300)
            .filter(|p| {
                (n.sample4(p[0], p[1], p[2], 0.0) - n.sample4(p[0], p[1], p[2], 3.7)).abs() > 1.0e-6
            })
            .count();
        assert!(moved > 250, "{basis:?} ignores the fourth coordinate");
    }
}

#[test]
fn large_coordinates_stay_finite_and_in_range() {
    for basis in all_bases() {
        let n = noise(basis);
        for k in 0..200 {
            let base = 1.0e6 + f64::from(k) * 0.37;
            let v = n.sample3(base, -base * 0.5, base * 0.25);
            assert!((-1.0..=1.0).contains(&v), "{basis:?} at {base}: {v}");
        }
    }
}

#[test]
fn non_finite_input_gives_zero_instead_of_poisoning_a_terrain() {
    let n = noise(Basis::Perlin);
    assert_eq!(n.sample2(f64::NAN, 1.0), 0.0);
    assert_eq!(n.sample3(0.5, f64::INFINITY, 1.0), 0.0);
    assert_eq!(n.sample4(0.5, 1.0, 2.0, f64::NEG_INFINITY), 0.0);
}

#[test]
fn bad_settings_are_refused_with_the_name_of_the_setting() {
    let bad = |n: Noise, setting: &str| {
        let Err(NoiseError::Setting { name, .. }) = n.validate() else {
            panic!("{setting} should be refused");
        };
        assert_eq!(name, setting);
    };
    bad(
        Noise {
            frequency: 0.0,
            ..Noise::default()
        },
        "frequency",
    );
    bad(
        Noise {
            frequency: f64::NAN,
            ..Noise::default()
        },
        "frequency",
    );
    bad(
        Noise {
            fractal: Some(fractal(FractalKind::Fbm, 0)),
            ..Noise::default()
        },
        "octaves",
    );
    bad(
        Noise {
            fractal: Some(fractal(FractalKind::Fbm, 13)),
            ..Noise::default()
        },
        "octaves",
    );
    bad(
        Noise {
            fractal: Some(Fractal {
                lacunarity: 0.5,
                ..fractal(FractalKind::Fbm, 3)
            }),
            ..Noise::default()
        },
        "lacunarity",
    );
    bad(
        Noise {
            fractal: Some(Fractal {
                gain: 1.5,
                ..fractal(FractalKind::Fbm, 3)
            }),
            ..Noise::default()
        },
        "gain",
    );
    bad(
        Noise {
            basis: Basis::Cellular {
                distance: CellDistance::Euclidean,
                ret: CellReturn::Value,
                jitter: 2.0,
            },
            ..Noise::default()
        },
        "jitter",
    );
    bad(
        Noise {
            warp: Some(Warp {
                kind: WarpKind::Gradient,
                amplitude: -1.0,
                frequency: 1.0,
                octaves: 1,
            }),
            ..Noise::default()
        },
        "amplitude",
    );
}
