//! `ve_world::water_sim`: a real wave simulation for a body of water (linear shallow-water equations on a grid). Where the procedural wave field gives the resting look,
//! this reacts: ripples spread from whatever disturbs the surface, travel at sqrt(g * depth) (slower over shallow water), reflect off the shore and die out by damping.
//! The tests check the physics, not a picture.
#![expect(
    clippy::cast_precision_loss,
    clippy::float_cmp,
    reason = "small grids; determinism is compared exactly"
)]
use ve_world::water_sim::{SimError, SimOptions, WaterSim};

const N: usize = 128;
const CELL: f32 = 0.25;
const G: f32 = 9.81;

fn opts() -> SimOptions {
    SimOptions {
        gravity: G,
        damping: 0.0,
        absorb_edges: false,
    }
}

/// Uniform water of `depth` metres over the whole grid.
fn uniform(depth: f32, options: SimOptions) -> WaterSim {
    WaterSim::new(&vec![depth; N * N], N, N, CELL, options).unwrap()
}

fn centre() -> (f32, f32) {
    (N as f32 * CELL / 2.0, N as f32 * CELL / 2.0)
}

/// Radius (from the centre) of the cell with the largest absolute displacement.
fn peak_radius(sim: &WaterSim) -> f32 {
    let (cx, cz) = centre();
    let mut best = (0.0_f32, 0.0_f32);
    for z in 0..N {
        for x in 0..N {
            let v = sim.heights()[z * N + x].abs();
            if v > best.0 {
                best = (
                    v,
                    ((x as f32 + 0.5) * CELL - cx).hypot((z as f32 + 0.5) * CELL - cz),
                );
            }
        }
    }
    best.1
}

fn run(sim: &mut WaterSim, seconds: f32, dt: f32) {
    let mut t = 0.0;
    while t < seconds {
        sim.step(dt);
        t += dt;
    }
}

#[test]
fn a_ripple_spreads_at_the_wave_speed_square_root_of_g_times_depth() {
    let depth = 1.0_f32;
    let mut sim = uniform(depth, opts());
    let (cx, cz) = centre();
    sim.disturb(cx, cz, 0.4, 0.05);
    run(&mut sim, 2.0, 1.0 / 60.0);
    let expected = (G * depth).sqrt() * 2.0; // 6.26 m
    let radius = peak_radius(&sim);
    assert!(
        (radius - expected).abs() < 0.8,
        "the ring is at {radius} m, expected about {expected} m"
    );
}

#[test]
fn shallow_water_is_slower_in_proportion_to_the_square_root_of_the_depth() {
    // Depth 1 m on the left half, 0.25 m on the right: the right runs at half the speed.
    let mut depths = vec![1.0; N * N];
    for z in 0..N {
        for x in N / 2..N {
            depths[z * N + x] = 0.25;
        }
    }
    let mut sim = WaterSim::new(&depths, N, N, CELL, opts()).unwrap();
    // Disturb on the seam, then measure how far the front has reached into each side after 1.5 s.
    let seam = (N / 2) as f32 * CELL;
    sim.disturb(seam, N as f32 * CELL / 2.0, 0.3, 0.05);
    run(&mut sim, 1.5, 1.0 / 60.0);
    let reach = |range: std::ops::Range<usize>| {
        let mut far = 0.0_f32;
        for z in 0..N {
            for x in range.clone() {
                if sim.heights()[z * N + x].abs() > 0.002 {
                    far = far.max(((x as f32 + 0.5) * CELL - seam).abs());
                }
            }
        }
        far
    };
    let deep = reach(0..N / 2);
    let shallow = reach(N / 2..N);
    assert!(deep > 0.0 && shallow > 0.0, "{deep} {shallow}");
    let ratio = shallow / deep;
    assert!(
        (0.35..0.75).contains(&ratio),
        "the shallow side should be about half as fast, ratio {ratio}"
    );
}

#[test]
fn it_stays_stable_for_any_time_step_the_caller_passes() {
    for dt in [1.0 / 240.0, 1.0 / 60.0, 1.0 / 20.0, 0.1, 0.5] {
        let mut sim = uniform(2.0, opts());
        let (cx, cz) = centre();
        sim.disturb(cx, cz, 0.5, 0.1);
        run(&mut sim, 6.0, dt);
        assert!(
            sim.heights().iter().all(|h| h.is_finite()),
            "dt {dt}: not finite"
        );
        let biggest = sim.heights().iter().fold(0.0_f32, |a, h| a.max(h.abs()));
        assert!(biggest <= 0.1 + 1e-3, "dt {dt}: grew to {biggest}");
    }
}

#[test]
fn without_damping_or_open_edges_the_water_volume_is_conserved() {
    let mut sim = uniform(1.0, opts());
    let (cx, cz) = centre();
    sim.disturb(cx, cz, 0.6, 0.08);
    let before = sim.total_volume();
    assert!(before > 0.0);
    run(&mut sim, 5.0, 1.0 / 60.0);
    assert!(
        (sim.total_volume() - before).abs() / before < 1e-3,
        "{before} -> {}",
        sim.total_volume()
    );
}

#[test]
fn damping_makes_the_ripples_die_out() {
    let energy = |damping: f32| {
        let mut sim = uniform(1.0, SimOptions { damping, ..opts() });
        let (cx, cz) = centre();
        sim.disturb(cx, cz, 0.5, 0.08);
        run(&mut sim, 4.0, 1.0 / 60.0);
        sim.heights()
            .iter()
            .map(|h| f64::from(*h).powi(2))
            .sum::<f64>()
    };
    assert!(
        energy(1.5) < energy(0.0) * 0.1,
        "{} against {}",
        energy(1.5),
        energy(0.0)
    );
}

#[test]
fn open_edges_let_the_waves_leave_and_closed_edges_keep_them_in() {
    let energy = |absorb: bool| {
        let mut sim = uniform(
            1.0,
            SimOptions {
                absorb_edges: absorb,
                ..opts()
            },
        );
        let (cx, cz) = centre();
        sim.disturb(cx, cz, 0.5, 0.08);
        run(&mut sim, 8.0, 1.0 / 60.0);
        sim.heights()
            .iter()
            .map(|h| f64::from(*h).powi(2))
            .sum::<f64>()
    };
    assert!(
        energy(true) < energy(false) * 0.25,
        "{} against {}",
        energy(true),
        energy(false)
    );
}

#[test]
fn a_centred_disturbance_spreads_symmetrically() {
    let mut sim = uniform(1.0, opts());
    let (cx, cz) = centre();
    sim.disturb(cx, cz, 0.5, 0.05);
    run(&mut sim, 1.2, 1.0 / 60.0);
    let h = |x: usize, z: usize| sim.heights()[z * N + x];
    for (dx, dz) in [(10, 0), (0, 10), (7, 7), (20, 3)] {
        let a = h(N / 2 + dx, N / 2 + dz);
        let b = h(N / 2 - 1 - dx, N / 2 - 1 - dz);
        assert!((a - b).abs() < 2e-4, "({dx},{dz}): {a} against {b}");
    }
}

#[test]
fn land_stays_dry_and_waves_reflect_off_it() {
    // A pool 64 x 64 cells in the middle of a grid of land (depth 0).
    let mut depths = vec![0.0; N * N];
    for z in 32..96 {
        for x in 32..96 {
            depths[z * N + x] = 1.0;
        }
    }
    let mut sim = WaterSim::new(&depths, N, N, CELL, opts()).unwrap();
    sim.disturb(N as f32 * CELL / 2.0, N as f32 * CELL / 2.0, 0.4, 0.05);
    run(&mut sim, 6.0, 1.0 / 60.0);
    for i in 0..N * N {
        if depths[i] == 0.0 {
            assert_eq!(sim.heights()[i], 0.0, "land cell {i} moved");
        }
    }
    let inside = (32..96)
        .map(|z| {
            (32..96)
                .map(|x| sim.heights()[z * N + x].abs())
                .fold(0.0_f32, f32::max)
        })
        .fold(0.0_f32, f32::max);
    assert!(inside > 0.003, "the pool keeps its waves: {inside}");
}

#[test]
fn the_surface_normal_leans_away_from_a_hump_and_is_flat_on_still_water() {
    let mut sim = uniform(1.0, opts());
    let (cx, cz) = centre();
    let flat = sim.normal(cx, cz);
    assert!(
        flat[0].abs() < 1e-6 && (flat[1] - 1.0).abs() < 1e-6 && flat[2].abs() < 1e-6,
        "{flat:?}"
    );
    sim.disturb(cx, cz, 1.0, 0.1);
    let left = sim.normal(cx - 0.8, cz);
    let right = sim.normal(cx + 0.8, cz);
    assert!(left[0] < -1e-3 && right[0] > 1e-3, "{left:?} {right:?}");
    for n in [left, right] {
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        assert!((len - 1.0).abs() < 1e-4);
    }
}

#[test]
fn height_between_cells_is_interpolated_and_outside_the_grid_is_zero() {
    let mut sim = uniform(1.0, opts());
    let (cx, cz) = centre();
    sim.disturb(cx, cz, 0.6, 0.1);
    let a = sim.height(cx, cz);
    let b = sim.height(cx + CELL * 0.5, cz);
    assert!(a > 0.05 && b > 0.0 && b <= a + 1e-6);
    assert_eq!(sim.height(-5.0, 3.0), 0.0);
    assert_eq!(sim.height(1.0e6, 1.0e6), 0.0);
    assert_eq!(sim.height(f32::NAN, 0.0), 0.0);
}

#[test]
fn bad_calls_change_nothing_and_do_not_panic() {
    let mut sim = uniform(1.0, opts());
    let before = sim.heights().to_vec();
    sim.disturb(f32::NAN, 1.0, 0.5, 0.1);
    sim.disturb(1.0, 1.0, -1.0, 0.1);
    sim.disturb(-100.0, -100.0, 0.5, 0.1);
    sim.step(0.0);
    sim.step(-1.0);
    sim.step(f32::NAN);
    assert_eq!(sim.heights(), &before[..]);
}

#[test]
fn a_disturbance_on_land_does_nothing() {
    let mut depths = vec![0.0; N * N];
    for z in 0..N {
        for x in 0..N / 2 {
            depths[z * N + x] = 1.0;
        }
    }
    let mut sim = WaterSim::new(&depths, N, N, CELL, opts()).unwrap();
    sim.disturb((N * 3 / 4) as f32 * CELL, N as f32 * CELL / 2.0, 0.5, 0.1);
    assert!(sim.heights().iter().all(|h| *h == 0.0));
}

#[test]
fn the_same_inputs_give_bit_identical_water() {
    let make = || {
        let mut sim = uniform(
            1.0,
            SimOptions {
                damping: 0.2,
                ..opts()
            },
        );
        let (cx, cz) = centre();
        sim.disturb(cx, cz, 0.5, 0.05);
        sim.disturb(cx + 3.0, cz - 2.0, 0.3, -0.03);
        run(&mut sim, 3.0, 1.0 / 60.0);
        sim.heights()
            .iter()
            .map(|h| h.to_bits())
            .collect::<Vec<_>>()
    };
    assert_eq!(make(), make());
}

#[test]
fn a_grid_that_does_not_match_its_depths_or_has_bad_numbers_is_refused() {
    assert!(matches!(
        WaterSim::new(&[1.0; 10], 4, 4, 0.25, opts()),
        Err(SimError::BadShape { .. })
    ));
    assert!(WaterSim::new(&vec![1.0; N * N], N, N, 0.0, opts()).is_err());
    assert!(WaterSim::new(&vec![1.0; N * N], N, N, f32::NAN, opts()).is_err());
    assert!(WaterSim::new(&vec![f32::NAN; N * N], N, N, 0.25, opts()).is_err());
    assert!(
        WaterSim::new(
            &vec![1.0; N * N],
            N,
            N,
            0.25,
            SimOptions {
                gravity: -1.0,
                ..opts()
            }
        )
        .is_err()
    );
    assert!(
        WaterSim::new(
            &vec![1.0; N * N],
            N,
            N,
            0.25,
            SimOptions {
                damping: -0.5,
                ..opts()
            }
        )
        .is_err()
    );
    assert!(WaterSim::new(&[], 0, 0, 0.25, opts()).is_err());
}
